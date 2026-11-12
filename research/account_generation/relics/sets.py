"""同时满足单件与整套有效词条目标的角色遗器生成。"""

import random
from dataclasses import replace

from .models import CharacterRelicsRequest, GeneratedCharacterRelics, Relic, RelicSpec
from .rules import SLOT_MAINSTATS
from .single import (
    SCORE_TOLERANCE,
    GenerationTargetNotReached,
    _PieceSearch,
    _SearchBudget,
)


def _complete_piece_specs(pieces: tuple[RelicSpec, ...]) -> tuple[RelicSpec, ...]:
    """保留输入顺序并补齐六个部位，检查部位集合。

    Args:
        pieces: 调用方提供的互异部位条件。

    Returns:
        输入部位在前、缺少部位按 1～6 顺序追加的完整配置。

    Raises:
        ValueError: 部位重复或不属于六个合法部位。
    """
    provided_slots = {p.slot for p in pieces}
    if len(provided_slots) != len(pieces) or not provided_slots <= set(SLOT_MAINSTATS):
        raise ValueError("指定部位需要互异，且属于六个合法部位")
    return pieces + tuple(
        RelicSpec(slot=slot) for slot in SLOT_MAINSTATS if slot not in provided_slots
    )


def maximum_character_relic_score(request: CharacterRelicsRequest) -> float:
    """计算配装条件下六件遗器的理论有效词条上限。

    主词条、必含属性、初始条数和可用档位参与计算。单件与整套的得分目标
    用于筛选成品，上限计算保留这些目标施加前的最高合法得分。
    初始条数留空时包含四词条成品；计算使用独立随机源，结果与候选顺序无关。

    Args:
        request: 六件配装条件、角色权重与允许的档位。

    Returns:
        各部位最高合法得分的总和，单位为加权中档词条。

    Raises:
        ValueError: 部位、主副词条条件或档位权重不合法。
    """
    rng = random.Random(0)
    budget = _SearchBudget(0)
    return sum(
        _PieceSearch(
            replace(piece, min_effective_rolls=None, max_effective_rolls=None),
            request.effective_stat_weights,
            request.tier_weights,
            rng,
            budget,
        ).interval[1]
        for piece in _complete_piece_specs(request.pieces)
    )


def generate_character_relics(
    request: CharacterRelicsRequest,
    *,
    rng: random.Random,
) -> GeneratedCharacterRelics:
    """生成同时满足单件与整套得分目标的六件角色遗器。

    Args:
        request: 需要约束的部位、角色副词条权重、整套得分区间、
            档位权重及六件共用的搜索预算。
        rng: 控制候选顺序的随机源。相同请求与初始状态产生相同结果，
            调用会推进该随机源的状态。

    Returns:
        六件遗器、各件得分和搜索尝试次数。输入部位按原顺序返回，
        缺少部位随机补齐后，按标准部位顺序追加。
        合计与平均得分通过结果对象的属性读取。

    Raises:
        ValueError: 部位重复、未知，或主副词条条件、档位权重不合法。
        GenerationTargetNotReached: 单件与整套目标冲突、无合法组合或预算耗尽。
    """
    pieces = _complete_piece_specs(request.pieces)
    budget = _SearchBudget(request.max_attempts)
    searches = [
        _PieceSearch(piece, request.effective_stat_weights, request.tier_weights, rng, budget)
        for piece in pieces
    ]
    natural_lower = sum(s.interval[0] for s in searches)
    natural_upper = sum(s.interval[1] for s in searches)
    lower = (
        natural_lower
        if request.min_total_effective_rolls is None
        else max(natural_lower, request.min_total_effective_rolls)
    )
    upper = (
        natural_upper
        if request.max_total_effective_rolls is None
        else min(natural_upper, request.max_total_effective_rolls)
    )
    if lower > upper + SCORE_TOLERANCE:
        raise GenerationTargetNotReached("整套目标与各件的合法范围不相交")

    # 先满足限制较紧的部位，余下目标由其他部位共同承担。
    order = sorted(range(6), key=lambda i: searches[i].interval[1] - searches[i].interval[0])
    results: dict[int, tuple[Relic, float]] = {}

    def solve(position: int, score: float) -> bool:
        """为剩余部位寻找能够满足整套目标的分配。

        Args:
            position: 当前部位在搜索顺序中的位置。
            score: 已选遗器的累计有效词条得分。

        Returns:
            找到完整方案时返回 True，并将选中遗器写入 results；
            当前分支无解时返回 False。

        Raises:
            GenerationTargetNotReached: 六件共用的搜索预算耗尽。
        """
        if position == 6:
            return lower - SCORE_TOLERANCE <= score <= upper + SCORE_TOLERANCE
        index = order[position]
        rest = [searches[i] for i in order[position + 1 :]]
        target = None
        if (
            request.min_total_effective_rolls is not None
            or request.max_total_effective_rolls is not None
        ):
            target = (
                lower - score - sum(s.interval[1] for s in rest),
                upper - score - sum(s.interval[0] for s in rest),
            )
        for relic, piece_score in searches[index].candidates(target):
            results[index] = relic, piece_score
            if solve(position + 1, score + piece_score):
                return True
        return False

    if not solve(0, 0):
        raise GenerationTargetNotReached("整套目标不存在满足各件约束的合法档位组合")
    return GeneratedCharacterRelics(
        relics=tuple(results[i][0] for i in range(6)),
        piece_scores=tuple(results[i][1] for i in range(6)),
        attempts=budget.attempts,
    )
