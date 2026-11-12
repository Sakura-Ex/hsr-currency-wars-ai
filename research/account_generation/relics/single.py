"""直接搜索满足目标的五星 +15 遗器成品。"""

import random
from collections.abc import Iterator
from dataclasses import dataclass
from functools import cache
from itertools import combinations
from math import comb, isfinite, log, log1p

from .models import GeneratedRelic, Relic, RelicRequest, RelicSpec
from .rules import INITIAL_SUBSTAT_WEIGHTS, SLOT_MAINSTATS, SLOT_SET_IDS, SUBSTAT_ROLLS

# 有效词条目标边界比较时允许的分数误差。
SCORE_TOLERANCE = 1e-6


class GenerationTargetNotReached(ValueError):
    """生成器因目标无解或搜索预算耗尽而未能提供成品。

    异常消息区分目标范围冲突、穷尽合法组合和预算耗尽。预算耗尽表示
    本次搜索尚未找到结果。
    """


def score_relic(relic: Relic, *, effective_stat_weights: dict[str, float]) -> float:
    """按角色权重计算遗器的有效词条数。

    Args:
        relic: 包含副词条最终数值的遗器，百分比使用比例值，例如 0.0324 表示 3.24%。
        effective_stat_weights: 副词条键及其角色权重，未列出的属性计零分。

    Returns:
        各属性按单次中档数值折算后，乘以角色权重的得分之和。
    """
    return sum(
        effective_stat_weights.get(key, 0) * value / SUBSTAT_ROLLS[key][1]
        for key, value in relic.substats.items()
    )


@dataclass
class _SearchBudget:
    """在单件或整套回溯中共享的搜索尝试计数器。

    Attributes:
        limit: 允许考察的属性集合与数值分配选项的总次数上限。
        attempts: 已消耗的尝试次数，由 consume 统一更新。
    """

    limit: int
    attempts: int = 0

    def consume(self) -> None:
        """记录一次搜索尝试，并检查共享预算。

        Raises:
            GenerationTargetNotReached: 预算已用尽，本次尝试尚未计入。
        """
        if self.attempts >= self.limit:
            raise GenerationTargetNotReached(f"搜索预算耗尽，已尝试 {self.limit} 次分配")
        self.attempts += 1


@cache
def _tier_counts(count: int, weights: tuple[float, float, float]):
    """枚举指定次数对应的低、中、高档数量组合。

    Args:
        count: 分配给一种副词条的总获得次数。
        weights: 低、中、高档的有限非负相对权重，至少一档为正。

    Returns:
        由档位数量元组和对数组合概率组成的元组。每项形如
        ((低档次数, 中档次数, 高档次数), 对数概率)，排除使用零权重档位的组合。
    """
    largest = max(weights)
    # 缩放后计算对数总权重，保留极小正权重对应的合法组合。
    log_total = log(largest) + log(sum(w / largest for w in weights))
    log_probabilities = tuple(log(w) - log_total if w else 0 for w in weights)
    result = []
    for low in range(count + 1 if weights[0] else 1):
        for mid in range(count - low + 1 if weights[1] else 1):
            high = count - low - mid
            if high and not weights[2]:
                continue
            log_probability = (
                log(comb(count, low) * comb(count - low, mid))
                + log_probabilities[0] * low
                + log_probabilities[1] * mid
                + log_probabilities[2] * high
            )
            result.append(((low, mid, high), log_probability))
    return tuple(result)


class _PieceSearch:
    """维护单件成品的主副词条候选，按得分范围进行离散搜索。

    Attributes:
        spec: 当前部位的配装条件。
        set_id: 实际使用的套装 ID，来自指定值或随机选择。
        weights: 角色有效副词条权重。
        tier_weights: 低、中、高档的候选排序权重。
        rng: 准备候选和搜索时共用的随机源。
        budget: 本次搜索共用的尝试计数器。
        total_roll_options: 按抽样顺序排列的成品获得总次数，取八次或九次。
            未指定初始条数时包含两种，显式指定时包含一种。
        keysets: 主词条与四种相容副词条键组成的候选集合，按搜索顺序排列。
        unit_bounds: 各属性单次获得能够贡献的最低、最高有效词条得分。
        interval: 合法得分上下界与单件目标相交后的区间；
            具体档位组合的可达性由搜索确定。
    """

    def __init__(
        self,
        spec: RelicSpec,
        weights: dict[str, float],
        tier_weights: tuple[float, float, float],
        rng: random.Random,
        budget: _SearchBudget,
    ):
        """准备单件遗器的主副词条候选和得分范围。

        Args:
            spec: 部位、主词条候选、必含副词条及单件得分目标。
            weights: 按角色提供的副词条有效权重。
            tier_weights: 低、中、高档的有限非负相对权重，至少一档为正。
            rng: 控制候选顺序的随机源，其状态随准备和搜索推进。
            budget: 单件或整套搜索共用的尝试预算。

        Raises:
            ValueError: 部位、套装、初始条数、词条或档位权重不合法。
            GenerationTargetNotReached: 目标区间与合法成品得分范围不相交。
        """
        if spec.slot not in SLOT_MAINSTATS:
            raise ValueError(f"未知遗器部位 {spec.slot}")
        if (
            len(tier_weights) != 3
            or any(not isfinite(weight) or weight < 0 for weight in tier_weights)
            or not any(tier_weights)
        ):
            raise ValueError("档位权重需要三个有限非负数，且至少一项大于零")
        allowed_mainstats = SLOT_MAINSTATS[spec.slot]
        if spec.set_id is not None and spec.set_id not in SLOT_SET_IDS[spec.slot]:
            raise ValueError("套装 ID 需要适用于指定部位")
        if spec.initial_substat_count not in (None, 3, 4):
            raise ValueError("初始词条数需要为 None、3 或 4")
        if spec.mainstat is None:
            mainstats = allowed_mainstats
        elif isinstance(spec.mainstat, str):
            mainstats = (spec.mainstat,)
        else:
            mainstats = spec.mainstat
        if not mainstats or not set(mainstats) <= set(allowed_mainstats):
            raise ValueError("该部位无法生成指定主词条")
        required = spec.required_substats
        if len(required) > 4 or len(set(required)) != len(required):
            raise ValueError("必含副词条需要互异、至多四种，并与主词条相容")
        self.spec = spec
        self.set_id = (
            spec.set_id if spec.set_id is not None else rng.choice(SLOT_SET_IDS[spec.slot])
        )
        self.weights = weights
        self.tier_weights = tier_weights
        self.rng = rng
        self.budget = budget
        if spec.initial_substat_count is None:
            first = rng.choices((8, 9), weights=INITIAL_SUBSTAT_WEIGHTS, k=1)[0]
            self.total_roll_options = (first, 9 if first == 8 else 8)
        else:
            self.total_roll_options = (spec.initial_substat_count + 5,)
        self.keysets = []
        for mainstat in dict.fromkeys(mainstats):
            pool = [key for key in SUBSTAT_ROLLS if key != mainstat]
            if not set(required) <= set(pool):
                continue
            self.keysets.extend(
                (mainstat, required + rest)
                for rest in combinations(
                    [key for key in pool if key not in required], 4 - len(required)
                )
            )
        if not self.keysets:
            raise ValueError("必含副词条与所有候选主词条冲突，或包含未知属性")
        rng.shuffle(self.keysets)
        self.unit_bounds = {}
        for key in SUBSTAT_ROLLS:
            scores = [
                weights.get(key, 0) * value / SUBSTAT_ROLLS[key][1]
                for value, probability in zip(SUBSTAT_ROLLS[key], tier_weights, strict=True)
                if probability > 0
            ]
            self.unit_bounds[key] = min(scores), max(scores)
        bounds = [
            self._bounds(keys, total_rolls)
            for total_rolls in self.total_roll_options
            for _, keys in self.keysets
        ]
        lower, upper = min(b[0] for b in bounds), max(b[1] for b in bounds)
        if spec.min_effective_rolls is not None:
            lower = max(lower, spec.min_effective_rolls)
        if spec.max_effective_rolls is not None:
            upper = min(upper, spec.max_effective_rolls)
        self.interval = lower, upper
        if lower > upper + SCORE_TOLERANCE:
            raise GenerationTargetNotReached(f"{spec.slot} 的目标超出合法成品范围")

    def _bounds(self, keys: tuple[str, ...], rolls: int) -> tuple[float, float]:
        """计算剩余获得次数能够贡献的得分上下界。

        Args:
            keys: 尚待分配的副词条键，至少包含一个属性。
            rolls: 分配给这些属性的总获得次数，每种属性至少分配一次。

        Returns:
            最低和最高贡献得分组成的元组，供后续离散搜索排除分支。
        """
        extra = rolls - len(keys)
        lows, highs = zip(*(self.unit_bounds[key] for key in keys), strict=True)
        return sum(lows) + extra * min(lows), sum(highs) + extra * max(highs)

    def candidates(
        self,
        target: tuple[float, float] | None = None,
        *,
        preferred_score: float | None = None,
        middle_only: bool = False,
        distinct_scores: bool = False,
    ) -> Iterator[tuple[Relic, float]]:
        """逐个提供满足单件目标与附加目标的成品。

        Args:
            target: 由整套搜索传入的附加得分区间。为 None 时使用单件范围。
            preferred_score: 区间内优先接近的分数；None 时使用区间中点。
            middle_only: 本轮使用中档组合，供账号精确目标先搜索整条或分数条网格。
            distinct_scores: 账号搜索按剩余计分属性、次数与累计分数合并等价分配。

        Yields:
            遗器及其有效词条得分组成的元组。搜索推进随机源和共享预算。

        Raises:
            GenerationTargetNotReached: 搜索尝试预算耗尽。
        """
        if middle_only and self.tier_weights[1] == 0:
            return
        lower, upper = self.interval
        if target is not None:
            lower, upper = max(lower, target[0]), min(upper, target[1])
        if lower > upper + 2 * SCORE_TOLERANCE:
            return
        guided = (
            target is not None
            or preferred_score is not None
            or self.spec.min_effective_rolls is not None
            or self.spec.max_effective_rolls is not None
        )

        preferred = (
            (lower + upper) / 2
            if preferred_score is None
            else min(upper, max(lower, preferred_score))
        )

        seen_states: set[tuple[tuple[str | None, ...], int, float]] = set()

        def search(
            keys: tuple[str, ...],
            rolls: int,
            score: float,
            stats: dict[str, float],
        ) -> Iterator[tuple[dict[str, float], float]]:
            """为剩余属性分配合法的获得次数和档位数量。

            Args:
                keys: 尚待分配的副词条键。
                rolls: 剩余获得次数。
                score: 已分配属性的累计有效词条得分。
                stats: 已确定最终数值的副词条。

            Yields:
                完整副词条字典及累计得分，满足本次搜索的目标范围。

            Raises:
                GenerationTargetNotReached: 搜索尝试预算耗尽。
            """
            if distinct_scores:
                # 四种属性已通过相容性检查，零权重属性只占用一次或多次获得次数。
                scoring_keys = tuple(key if self.weights.get(key, 0) else None for key in keys)
                state = (scoring_keys, rolls, score)
                if state in seen_states:
                    return
                seen_states.add(state)
            self.budget.consume()
            if not keys:
                if rolls == 0 and lower - SCORE_TOLERANCE <= score <= upper + SCORE_TOLERANCE:
                    yield stats, score
                return
            key, *tail = keys
            rest = tuple(tail)
            options = []
            for count in range(1, rolls - len(rest) + 1):
                # 最后一个属性取得所有剩余次数。
                if not rest and count != rolls:
                    continue
                rest_low, rest_high = self._bounds(rest, rolls - count) if rest else (0, 0)
                tier_weights = (0, 1, 0) if middle_only else self.tier_weights
                for tiers, log_probability in _tier_counts(count, tier_weights):
                    value = round(
                        sum(n * v for n, v in zip(tiers, SUBSTAT_ROLLS[key], strict=True)), 10
                    )
                    contribution = self.weights.get(key, 0) * value / SUBSTAT_ROLLS[key][1]
                    next_score = score + contribution
                    if (
                        next_score + rest_low > upper + SCORE_TOLERANCE
                        or next_score + rest_high < lower - SCORE_TOLERANCE
                    ):
                        continue
                    distance = abs(next_score + (rest_low + rest_high) / 2 - preferred)
                    # 在对数域比较指数随机值与组合概率之比。
                    draw = -log1p(-self.rng.random())
                    priority = log(draw) - log_probability if draw else float("-inf")
                    options.append(
                        (
                            distance if guided else 0,
                            priority,
                            count,
                            value,
                            contribution,
                        )
                    )
            options.sort()
            for _, _, count, value, contribution in options:
                yield from search(rest, rolls - count, score + contribution, stats | {key: value})

        for total_rolls in self.total_roll_options:
            bounds = [self._bounds(keys, total_rolls) for _, keys in self.keysets]
            if (
                min(b[0] for b in bounds) > upper + SCORE_TOLERANCE
                or max(b[1] for b in bounds) < lower - SCORE_TOLERANCE
            ):
                continue
            for (mainstat, keys), (bound_low, bound_high) in zip(self.keysets, bounds, strict=True):
                if bound_low > upper + SCORE_TOLERANCE or bound_high < lower - SCORE_TOLERANCE:
                    if not distinct_scores:
                        self.budget.consume()
                    continue
                for stats, score in search(keys, total_rolls, 0, {}):
                    yield Relic(self.set_id, self.spec.slot, mainstat, stats), score


def generate_relic(request: RelicRequest, *, rng: random.Random) -> GeneratedRelic:
    """生成一件满足配装条件与得分目标的五星 +15 遗器。

    Args:
        request: 主副词条条件、角色权重、档位权重、得分区间和搜索预算。
        rng: 控制搜索顺序的随机源。相同请求与初始状态产生相同结果，
            调用会推进该随机源的状态。

    Returns:
        最终遗器、有效词条得分及实际搜索尝试次数。

    Raises:
        ValueError: 部位、套装、初始条数、词条或档位权重不合法。
        GenerationTargetNotReached: 目标范围冲突、无合法档位组合或预算耗尽。
    """
    budget = _SearchBudget(request.max_attempts)
    search = _PieceSearch(
        request, request.effective_stat_weights, request.tier_weights, rng, budget
    )
    result = next(search.candidates(), None)
    if result is None:
        raise GenerationTargetNotReached(f"{request.slot} 的目标不存在合法档位组合")
    relic, score = result
    return GeneratedRelic(relic, score, budget.attempts)
