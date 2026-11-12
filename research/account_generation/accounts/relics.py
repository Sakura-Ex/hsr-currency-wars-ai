"""联合搜索账号内全部遗器，满足平均词条目标与角色投入分布。"""

import random
from fractions import Fraction
from math import gcd, lcm

from ..characters.generator import _QUALITY_RATIOS, _prepare_character_relics
from ..characters.models import CharacterSpec
from ..relics.models import Relic
from ..relics.sets import _complete_piece_specs
from ..relics.single import SCORE_TOLERANCE, GenerationTargetNotReached, _PieceSearch, _SearchBudget
from .allocation import _concentration_weights, _distribute
from .models import AccountReference, AccountRequest

# 连续投入目标转换到离散遗器时，每个角色允许半条的分配偏差。
_INVESTMENT_SCORE_SPREAD = 0.5


class _RosterRelicTargetNotReached(GenerationTargetNotReached):
    """当前名单的遗器无法满足目标，可继续检查其他名单。"""


def _generate_account_relics(
    ids: tuple[str, ...],
    cores: tuple[str, ...],
    specs: dict[str, CharacterSpec],
    request: AccountRequest,
    reference: AccountReference,
    rng: random.Random,
    budget: _SearchBudget,
) -> tuple[dict[str, tuple[tuple[Relic, float], ...]], dict[str, float]]:
    """联合生成账号遗器，按角色理论容量分配有效词条投入。

    Args:
        ids: 持有角色 ID，输出沿用此顺序。
        cores: 当前核心角色。
        specs: 已确定角色练度与光锥的生成条件。
        request: 账号平均词条区间、五档条件和集中程度。
        reference: 当前版本的角色推荐与规则数据。
        rng: 套装选择与全部离散候选共用的随机源。
        budget: 与金数分配共享的搜索预算。

    Returns:
        各角色按部位排列的遗器与得分，以及施加分数条件前的理论上限。

    Raises:
        _RosterRelicTargetNotReached: 当前名单的分数区间冲突或离散目标未找到解。
        GenerationTargetNotReached: 搜索预算耗尽或固定遗器条件无法满足。
        ValueError: 角色或档位条件不符合要求。
    """
    if not ids:
        minimum = request.min_mean_effective_rolls
        maximum = request.max_mean_effective_rolls
        if (
            (minimum is not None and minimum > 0)
            or (maximum is not None and maximum < 0)
            or request.relic_quality_tier is not None
        ):
            raise _RosterRelicTargetNotReached("空账号的遗器平均词条为零")
        return {}, {}

    searches: dict[str, list[_PieceSearch]] = {}
    group_bounds: dict[str, tuple[float, float]] = {}
    uppers = {}
    for cid in ids:
        relic_request, uppers[cid] = _prepare_character_relics(
            specs[cid], reference.characters[cid], rng
        )
        searches[cid] = [
            _PieceSearch(
                piece,
                relic_request.effective_stat_weights,
                relic_request.tier_weights,
                rng,
                budget,
            )
            for piece in sorted(_complete_piece_specs(relic_request.pieces), key=lambda p: p.slot)
        ]
        lower = sum(piece.interval[0] for piece in searches[cid])
        upper = sum(piece.interval[1] for piece in searches[cid])
        if relic_request.min_total_effective_rolls is not None:
            lower = max(lower, relic_request.min_total_effective_rolls)
        if relic_request.max_total_effective_rolls is not None:
            upper = min(upper, relic_request.max_total_effective_rolls)
        if lower > upper + SCORE_TOLERANCE:
            raise _RosterRelicTargetNotReached(f"{cid} 的遗器条件与合法范围不相交")
        group_bounds[cid] = (lower, upper)
    lower = sum(bounds[0] for bounds in group_bounds.values())
    upper = sum(bounds[1] for bounds in group_bounds.values())
    if request.min_mean_effective_rolls is not None:
        lower = max(lower, request.min_mean_effective_rolls * len(ids) * 6)
    if request.max_mean_effective_rolls is not None:
        upper = min(upper, request.max_mean_effective_rolls * len(ids) * 6)
    if request.relic_quality_tier is not None:
        tier = request.relic_quality_tier
        total_upper = sum(uppers.values())
        if total_upper <= SCORE_TOLERANCE:
            raise _RosterRelicTargetNotReached("五档练度需要至少一种可获得正分的副词条")
        lower = max(lower, _QUALITY_RATIOS[tier - 1] * total_upper)
        tier_upper = _QUALITY_RATIOS[tier] * total_upper
        if tier < 5:
            tier_upper -= 2 * SCORE_TOLERANCE
        upper = min(upper, tier_upper)
    if lower > upper + SCORE_TOLERANCE:
        raise _RosterRelicTargetNotReached("账号遗器平均词条目标与角色配装范围不相交")
    strength = request.resource_concentration
    if request.relic_concentration is not None:
        strength = request.relic_concentration
    desired = _distribute(
        rng.uniform(lower, upper),
        tuple(group_bounds[cid][0] for cid in ids),
        tuple(group_bounds[cid][1] for cid in ids),
        _concentration_weights(ids, cores, strength),
    )
    for cid, target in zip(ids, desired, strict=True):
        lo, hi = group_bounds[cid]
        group_bounds[cid] = (
            max(lo, target - _INVESTMENT_SCORE_SPREAD),
            min(hi, target + _INVESTMENT_SCORE_SPREAD),
        )
    desired_by_id = dict(zip(ids, desired, strict=True))
    groups = sorted(ids, key=lambda cid: group_bounds[cid][1] - group_bounds[cid][0])
    order = [
        (cid, piece)
        for cid in groups
        for piece in sorted(searches[cid], key=lambda piece: piece.interval[1] - piece.interval[0])
    ]
    suffix_low = [0.0] * (len(order) + 1)
    suffix_high = [0.0] * (len(order) + 1)
    for position in range(len(order) - 1, -1, -1):
        suffix_low[position] = suffix_low[position + 1] + order[position][1].interval[0]
        suffix_high[position] = suffix_high[position + 1] + order[position][1].interval[1]
    scores = dict.fromkeys(ids, 0.0)
    remaining_low = {cid: sum(p.interval[0] for p in searches[cid]) for cid in ids}
    remaining_high = {cid: sum(p.interval[1] for p in searches[cid]) for cid in ids}
    remaining_count = dict.fromkeys(ids, 6)
    results: dict[str, dict[int, tuple[Relic, float]]] = {cid: {} for cid in ids}

    # 中档分数由角色权重的整数组合构成，提前排除剩余角色无法凑齐的余数。
    scale = lcm(
        *(
            Fraction(str(weight)).denominator
            for _, piece in order
            for weight in piece.weights.values()
        )
    )
    suffix_grid = [0] * (len(order) + 1)
    for position in range(len(order) - 1, -1, -1):
        _, piece = order[position]
        units = (int(Fraction(str(weight)) * scale) for weight in piece.weights.values())
        suffix_grid[position] = gcd(suffix_grid[position + 1], *units)
    failed: set[tuple[int, float, float, bool]] = set()

    def solve(position: int, score: float, middle_only: bool) -> bool:
        """为下一个部位搜索同时满足角色与账号目标的成品。

        Args:
            position: 当前部位在账号搜索顺序中的位置。
            score: 已选遗器的账号累计得分。
            middle_only: 本轮是否在允许的中档网格上搜索。

        Returns:
            找到完整组合时返回 True，并把遗器与分数写入 results。
            失败分支恢复角色累计得分与剩余范围。

        Raises:
            GenerationTargetNotReached: 共享搜索预算耗尽。
        """
        if position == len(order):
            return lower - SCORE_TOLERANCE <= score <= upper + SCORE_TOLERANCE
        if middle_only:
            unit = suffix_grid[position] / scale
            remaining = (lower + upper) / 2 - score
            nearest = round(remaining / unit) * unit if unit else 0
            if abs(remaining - nearest) > SCORE_TOLERANCE + (upper - lower) / 2:
                return False
        cid, piece = order[position]
        state = (position, score, scores[cid], middle_only)
        if state in failed:
            return False
        remaining_low[cid] -= piece.interval[0]
        remaining_high[cid] -= piece.interval[1]
        remaining_count[cid] -= 1
        target = (
            max(
                lower - score - suffix_high[position + 1],
                group_bounds[cid][0] - scores[cid] - remaining_high[cid],
            ),
            min(
                upper - score - suffix_low[position + 1],
                group_bounds[cid][1] - scores[cid] - remaining_low[cid],
            ),
        )
        preferred = (desired_by_id[cid] - scores[cid]) / (remaining_count[cid] + 1)
        for relic, piece_score in piece.candidates(
            target,
            preferred_score=preferred,
            middle_only=middle_only,
            distinct_scores=True,
        ):
            results[cid][relic.slot] = (relic, piece_score)
            scores[cid] += piece_score
            if solve(position + 1, score + piece_score, middle_only):
                return True
            scores[cid] -= piece_score
        remaining_low[cid] += piece.interval[0]
        remaining_high[cid] += piece.interval[1]
        remaining_count[cid] += 1
        failed.add(state)
        return False

    # 精确总分先在全账号中档网格上搜索，再搜索全部允许的档位。
    phases = (
        (True, False)
        if upper - lower <= SCORE_TOLERANCE and all(piece.tier_weights[1] > 0 for _, piece in order)
        else (False,)
    )
    if not any(solve(0, 0, middle_only) for middle_only in phases):
        raise _RosterRelicTargetNotReached("账号遗器目标未找到满足投入分配的合法离散组合")
    return (
        {cid: tuple(results[cid][slot] for slot in range(1, 7)) for cid in ids},
        uppers,
    )
