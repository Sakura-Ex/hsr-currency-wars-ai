"""联合搜索账号内全部遗器，满足平均词条目标与角色投入分布。"""

import random
from collections.abc import Callable
from fractions import Fraction
from math import gcd, lcm

from ..characters.generator import _QUALITY_RATIOS, _prepare_character_relics
from ..characters.models import CharacterSpec
from ..relics.models import Relic
from ..relics.sets import _complete_piece_specs
from ..relics.single import SCORE_TOLERANCE, GenerationTargetNotReached, _PieceSearch, _SearchBudget
from .allocation import _concentration_weights, _distribute
from .models import AccountReference, AccountRequest
from .relic_score_dp import _try_middle_relics

# 优先搜索与角色连续投入目标相差半条以内的成品。
_INVESTMENT_SCORE_SPREAD = 0.5
# 窄区间对应已复现长尾的平均有效词条 2.99～3.01。
_NARROW_MEAN_SCORE_WIDTH = 0.02


class _RosterRelicTargetNotReached(GenerationTargetNotReached):
    """当前名单的遗器无法满足目标，可继续检查其他名单。"""


def _try_middle_phase[T](
    budget: _SearchBudget, limit: int, rng: random.Random, search: Callable[[], T]
) -> T | None:
    """用共享预算的一部分优先搜索全中档成品。

    Args:
        budget: 记录全部候选考察次数的共享预算。
        limit: 中档阶段可使用的累计次数上限。
        rng: 搜索随机源。中档额度耗尽时恢复阶段开始前的状态。
        search: 在当前额度下执行的中档搜索。
    Returns:
        搜索结果。当前中档额度耗尽时返回 None，并保留实际考察次数。
    """
    if budget.attempts >= limit:
        return None
    full_limit = budget.limit
    state = rng.getstate()
    budget.limit = limit
    try:
        return search()
    except GenerationTargetNotReached:
        rng.setstate(state)
        return None
    finally:
        budget.limit = full_limit


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
    desired_by_id = dict(zip(ids, desired, strict=True))
    preferred_bounds = {
        cid: (
            max(lo, desired_by_id[cid] - _INVESTMENT_SCORE_SPREAD),
            min(hi, desired_by_id[cid] + _INVESTMENT_SCORE_SPREAD),
        )
        for cid, (lo, hi) in group_bounds.items()
    }
    minimum, maximum = request.min_mean_effective_rolls, request.max_mean_effective_rolls
    only_middle_allowed = all(
        not (piece.tier_weights[0] or piece.tier_weights[2])
        for pieces in searches.values()
        for piece in pieces
    )
    # 中档优先阶段最多使用当前名单剩余额度的一半，其余供全档搜索。
    middle_limit = budget.attempts + (budget.limit - budget.attempts) // 2
    if (
        minimum is not None
        and maximum is not None
        and maximum - minimum <= _NARROW_MEAN_SCORE_WIDTH + SCORE_TOLERANCE / (len(ids) * 6)
    ):

        def find_middle() -> dict[str, tuple[tuple[Relic, float], ...]] | None:
            """在角色和账号的分数范围内还原位集给出的全中档成品。"""
            return _try_middle_relics(
                searches, group_bounds, preferred_bounds, (lower, upper), desired_by_id, rng
            )

        middle_results = (
            find_middle()
            if only_middle_allowed
            else _try_middle_phase(budget, middle_limit, rng, find_middle)
        )
        if middle_results is not None:
            return middle_results, uppers
    groups = sorted(ids, key=lambda cid: preferred_bounds[cid][1] - preferred_bounds[cid][0])
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

    def solve(
        position: int, score: float, middle_only: bool, bounds: dict[str, tuple[float, float]]
    ) -> bool:
        """为下一个部位搜索同时满足角色与账号目标的成品。

        Args:
            position: 当前部位在账号搜索顺序中的位置。
            score: 已选遗器的账号累计得分。
            middle_only: 本轮是否在允许的中档网格上搜索。
            bounds: 本轮各角色的得分范围。

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
        current_score = scores[cid]
        previous_low, previous_high = remaining_low[cid], remaining_high[cid]
        remaining_low[cid] = previous_low - piece.interval[0]
        remaining_high[cid] = previous_high - piece.interval[1]
        remaining_count[cid] -= 1
        target = (
            max(
                lower - score - suffix_high[position + 1],
                bounds[cid][0] - scores[cid] - remaining_high[cid],
            ),
            min(
                upper - score - suffix_low[position + 1],
                bounds[cid][1] - scores[cid] - remaining_low[cid],
            ),
        )
        preferred = (desired_by_id[cid] - scores[cid]) / (remaining_count[cid] + 1)
        try:
            for relic, piece_score in piece.candidates(
                target,
                preferred_score=preferred,
                middle_only=middle_only,
                distinct_scores=True,
            ):
                results[cid][relic.slot] = (relic, piece_score)
                scores[cid] = current_score + piece_score
                if solve(position + 1, score + piece_score, middle_only, bounds):
                    return True
                scores[cid] = current_score
        finally:
            scores[cid] = current_score
            remaining_low[cid], remaining_high[cid] = previous_low, previous_high
            remaining_count[cid] += 1
        failed.add(state)
        return False

    # 精确总分先在全账号中档网格上搜索，再搜索全部允许的档位。
    prioritize_middle = upper - lower <= SCORE_TOLERANCE and all(
        piece.tier_weights[1] > 0 for _, piece in order
    )
    ranges = (
        (preferred_bounds, group_bounds) if preferred_bounds != group_bounds else (group_bounds,)
    )
    for bounds in ranges:
        failed.clear()
        if (
            prioritize_middle
            and not only_middle_allowed
            and _try_middle_phase(
                budget, middle_limit, rng, lambda bounds=bounds: solve(0, 0, True, bounds)
            )
        ):
            break
        if solve(0, 0, prioritize_middle and only_middle_allowed, bounds):
            break
    else:
        raise _RosterRelicTargetNotReached("账号遗器目标未找到满足请求范围的合法离散组合")
    return (
        {cid: tuple(results[cid][slot] for slot in range(1, 7)) for cid in ids},
        uppers,
    )
