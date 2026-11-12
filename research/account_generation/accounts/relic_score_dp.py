"""在整数得分网格上合并单件、角色和账号的全中档可达分数。"""

import random
from collections.abc import Iterator
from fractions import Fraction
from math import ceil, floor, isfinite, lcm

from ..relics.models import Relic
from ..relics.rules import SUBSTAT_ROLLS
from ..relics.single import SCORE_TOLERANCE, _PieceSearch

# 自定义权重过细或过大时，使用通用搜索，限制密集位集的计算与存储。
_MAX_SCALE = 1_000
_MAX_SCORE_TICKS = 100_000
# 保留数值余量，浮点比较边缘由通用搜索判断。
_GRID_ROUNDOFF = 1e-9


def _integer_window(bounds: tuple[float, float], scale: int) -> tuple[int, int]:
    """将得分范围和比较容差换算成包含端点的整数区间。"""
    lo, hi = bounds
    tolerance = SCORE_TOLERANCE - _GRID_ROUNDOFF
    return ceil((lo - tolerance) * scale), floor((hi + tolerance) * scale)


def _suffix_scores(choices: list[tuple[int, ...]]) -> list[tuple[int, int]]:
    """合并每组选一项的可达总分，返回各后缀的最小分及以其为起点的位集。"""
    suffix = [(0, 1)] * (len(choices) + 1)
    for i in range(len(choices) - 1, -1, -1):
        base, previous = suffix[i + 1]
        minimum = choices[i][0]
        bits = 0
        for score in choices[i]:
            bits |= previous << (score - minimum)
        suffix[i] = (base + minimum, bits)
    return suffix


def _scores_in_window(
    reachable: tuple[int, int], bounds: tuple[float, float], scale: int
) -> Iterator[int]:
    """按升序给出分数窗口内已置位的整数分数。"""
    base, bits = reachable
    lo, hi = _integer_window(bounds, scale)
    first = max(0, lo - base)
    last = min(bits.bit_length() - 1, hi - base)
    if last < first:
        return
    bits = (bits >> first) & ((1 << (last - first + 1)) - 1)
    while bits:
        bit = bits & -bits
        yield base + first + bit.bit_length() - 1
        bits ^= bit


def _pick_scores(
    choices: list[tuple[int, ...]],
    suffix: list[tuple[int, int]],
    total: int,
    desired: list[float],
) -> tuple[int, ...]:
    """选出可凑齐总分的各组分数，依次优先接近各组期望值。"""
    selected = []
    for i, options in enumerate(choices):
        base, bits = suffix[i + 1]
        score = min(
            (
                score
                for score in options
                if (offset := total - score - base) >= 0 and (bits >> offset) & 1
            ),
            key=lambda score: abs(score - desired[i]),
        )
        selected.append(score)
        total -= score
    return tuple(selected)


def _try_middle_relics(
    searches: dict[str, list[_PieceSearch]],
    group_bounds: dict[str, tuple[float, float]],
    preferred_bounds: dict[str, tuple[float, float]],
    total_bounds: tuple[float, float],
    desired: dict[str, float],
    rng: random.Random,
) -> dict[str, tuple[tuple[Relic, float], ...]] | None:
    """尝试用全中档成品满足硬性分数范围，优先接近角色投入目标。

    Returns:
        按角色和部位排列的遗器及得分。网格无法给出可验证结果时返回 None。
    """
    if any(piece.tier_weights[1] == 0 for pieces in searches.values() for piece in pieces):
        return None
    scale = 1
    weights = {}
    for cid, pieces in searches.items():
        values = [pieces[0].weights.get(key, 0) for key in SUBSTAT_ROLLS]
        if not all(isfinite(value) for value in values):
            return None
        weights[cid] = [Fraction(str(value)) for value in values]
        scale = lcm(scale, *(weight.denominator for weight in weights[cid]))
        if scale > _MAX_SCALE:
            return None
    units = {
        cid: dict(zip(SUBSTAT_ROLLS, (int(weight * scale) for weight in values), strict=True))
        for cid, values in weights.items()
    }
    absolute_limit = sum(
        max(map(abs, units[cid].values())) * max(piece.roll_bounds)
        for cid, pieces in searches.items()
        for piece in pieces
    )
    if absolute_limit > _MAX_SCORE_TICKS:
        return None

    piece_choices: dict[str, list[tuple[int, ...]]] = {}
    piece_suffix = {}
    full_choices = []
    preferred_choices = []
    for cid, pieces in searches.items():
        choices = []
        for piece in pieces:
            lo, hi = _integer_window(piece.interval, scale)
            scores = tuple(
                sorted(score for score in piece.middle_scores(units[cid]) if lo <= score <= hi)
            )
            if not scores:
                return None
            choices.append(scores)
        piece_choices[cid] = choices
        piece_suffix[cid] = _suffix_scores(choices)
        reachable = piece_suffix[cid][0]
        full = tuple(_scores_in_window(reachable, group_bounds[cid], scale))
        if not full:
            return None
        full_choices.append(full)
        preferred_choices.append(tuple(_scores_in_window(reachable, preferred_bounds[cid], scale)))

    desired_ticks = [desired[cid] * scale for cid in searches]
    desired_total = sum(desired_ticks)
    for choices in (preferred_choices, full_choices):
        if not all(choices):
            continue
        suffix = _suffix_scores(choices)
        total = min(
            _scores_in_window(suffix[0], total_bounds, scale),
            key=lambda score: abs(score - desired_total),
            default=None,
        )
        if total is not None:
            role_scores = _pick_scores(choices, suffix, total, desired_ticks)
            break
    else:
        return None

    state = rng.getstate()
    results = {}
    for (cid, pieces), role_score in zip(searches.items(), role_scores, strict=True):
        scores = _pick_scores(
            piece_choices[cid],
            piece_suffix[cid],
            role_score,
            [role_score / len(pieces)] * len(pieces),
        )
        relics = []
        for piece, score in zip(pieces, scores, strict=True):
            target = score / scale
            relic = next(
                piece.candidates((target, target), middle_only=True, distinct_scores=True), None
            )
            if relic is None:
                rng.setstate(state)
                return None
            relics.append(relic)
        lo, hi = group_bounds[cid]
        if not lo - SCORE_TOLERANCE <= sum(score for _, score in relics) <= hi + SCORE_TOLERANCE:
            rng.setstate(state)
            return None
        results[cid] = tuple(relics)
    actual = sum(score for relics in results.values() for _, score in relics)
    if not total_bounds[0] - SCORE_TOLERANCE <= actual <= total_bounds[1] + SCORE_TOLERANCE:
        rng.setstate(state)
        return None
    return results
