"""按固定推荐关系选择持有角色，并搜索满足金数和唯一光锥限制的配装。"""

import random
from collections.abc import Iterator
from dataclasses import dataclass, replace

from ..characters.generator import _check_character_investment
from ..characters.models import CharacterSpec, LightConeSpec, RecommendedTeam
from ..relics.single import GenerationTargetNotReached, _SearchBudget
from .models import AccountReference, AccountRequest, GoldCategory
from .rules import GOLD_HALF_UNITS


@dataclass(frozen=True)
class _Build:
    """一名角色可采用的星魂与光锥组合。

    Attributes:
        eidolons: 星魂数。
        cone_id: 光锥种类 ID。
        superimposition: 光锥叠影。
        units: 当前预算类别下的总半金单位数。
        cone_units: 其中光锥占用的半金单位数。
    """

    eidolons: int
    cone_id: str
    superimposition: int
    units: int
    cone_units: int


def _gold_units(category: GoldCategory, copies: int, request: AccountRequest) -> int:
    """按账号统计类别将五星份数折算为半金单位。"""
    return copies * GOLD_HALF_UNITS[category] if category in request.gold_categories else 0


def _character_spec(
    cid: str, request: AccountRequest, reference: AccountReference
) -> CharacterSpec:
    """取得角色覆盖，或按五星统一配装构造生成条件。"""
    if cid in request.character_overrides:
        return request.character_overrides[cid]
    if request.five_star_build is not None and reference.characters[cid].rarity == 5:
        eidolons, superimposition = request.five_star_build
        return CharacterSpec(
            eidolons=eidolons,
            light_cone=LightConeSpec(
                light_cone_id=reference.signature_light_cone_ids[cid],
                superimposition=superimposition,
            ),
        )
    return CharacterSpec()


def _build_menus(
    request: AccountRequest, reference: AccountReference
) -> dict[str, tuple[_Build, ...]]:
    """枚举候选角色的星魂、光锥和叠影，保留显式配装覆盖。

    Args:
        request: 候选范围、统计类别及固定配装。
        reference: 角色、光锥命途与五星分类。

    Returns:
        可生成角色的配装菜单；没有自动光锥候选的角色不进入菜单。

    Raises:
        ValueError: 显式原始练度不合法。
        KeyError: 请求引用了参考资料中没有的角色或光锥。
    """
    pool = request.character_pool
    if pool is None:
        pool = tuple(reference.characters)
    menus = {}
    for cid in dict.fromkeys(pool):
        profile = reference.characters[cid]
        spec = _character_spec(cid, request, reference)
        _check_character_investment(spec)
        fixed = cid in request.character_overrides or (
            request.five_star_build is not None and profile.rarity == 5
        )
        if spec.light_cone.light_cone_id is not None:
            cone_ids = (spec.light_cone.light_cone_id,)
        else:
            cone_pool = request.light_cone_pool
            if cone_pool is None:
                cone_pool = tuple(reference.light_cones)
            cone_ids = tuple(
                cone_id
                for cone_id in dict.fromkeys(cone_pool)
                if reference.light_cones[cone_id].path == profile.path
            )
        options = []
        for cone_id in cone_ids:
            cone = reference.light_cones[cone_id]
            for eidolons in (int(spec.eidolons),) if fixed else range(7):
                for stack in (int(spec.light_cone.superimposition),) if fixed else range(1, 6):
                    body = _gold_units(
                        reference.character_gold_categories[cid], eidolons + 1, request
                    )
                    cone_units = _gold_units(cone.gold_category, stack, request)
                    options.append(_Build(eidolons, cone_id, stack, body + cone_units, cone_units))
        if options:
            menus[cid] = tuple(options)
    return menus


def _add_costs(left: set[int], right: set[int], cap: int) -> set[int]:
    """合并两组可达半金数，保留预算上限以内的和。"""
    return {a + b for a in left for b in right if a + b <= cap}


def _shift_costs(reachable: int, costs: set[int]) -> int:
    """将每种配装费用加入可达金额，整数的第 n 位表示 n 个半金单位。"""
    result = 0
    for cost in costs:
        result |= reachable << cost
    return result


def _roster_fits(
    selected: list[str],
    candidates: list[str],
    count: int,
    target: int,
    costs: dict[str, set[int]],
) -> bool:
    """检查还能否从候选中选足人数，并凑到指定半金数。"""
    if target > sum(max(costs[cid]) for cid in (*selected, *candidates)):
        return False
    if count == len(candidates):
        selected = selected + candidates
        candidates = []
        count = 0
    target_bit = 1 << target
    mask = (target_bit << 1) - 1
    sums = 1
    for cid in selected:
        sums = _shift_costs(sums, costs[cid]) & mask
    reachable = [0] * (count + 1)
    reachable[0] = sums
    for index, cid in enumerate(candidates):
        for size in range(min(count, index + 1), 0, -1):
            reachable[size] |= _shift_costs(reachable[size - 1], costs[cid]) & mask
    return bool(reachable[count] & target_bit)


def _choose_rosters(
    request: AccountRequest,
    reference: AccountReference,
    menus: dict[str, tuple[_Build, ...]],
    target: int | None,
    rng: random.Random,
    budget: _SearchBudget,
) -> Iterator[tuple[tuple[str, ...], tuple[str, ...]]]:
    """按推荐关联提供可达到指定金数的名单，供配装约束继续筛选。

    Args:
        request: 持有人数、候选范围、核心及队友关联强度。
        reference: 固定的推荐队伍与替补关系。
        menus: 各候选角色的可用配装。
        target: 精确半金数，None 时由后续配装确定。
        rng: 选择角色与推荐队伍的随机源，搜索会推进其状态。
        budget: 与配装和遗器搜索共用的候选考察预算。

    Yields:
        持有角色 ID 与核心角色 ID。每种持有集合与核心集合的组合至多提供一次；
        配装无法满足唯一光锥约束时，调用方继续读取下一组。

    Raises:
        ValueError: 必需角色不在可用候选中，或数量超过持有规模。
        GenerationTargetNotReached: 持有人数与金数不相容或预算耗尽。
    """
    selected = list(dict.fromkeys((*request.core_character_ids, *request.character_overrides)))
    if any(cid not in menus for cid in selected):
        raise ValueError("核心或覆盖角色需要位于候选范围内，并有可用的光锥")
    if len(selected) > request.character_count:
        raise ValueError("角色数量不足以容纳核心和显式覆盖角色")
    initial_candidates = [cid for cid in menus if cid not in selected]
    needed = request.character_count - len(selected)
    if needed > len(initial_candidates):
        raise GenerationTargetNotReached("候选范围内的可生成角色数量不足")
    costs = {cid: {build.units for build in options} for cid, options in menus.items()}
    if target is not None and not _roster_fits(selected, initial_candidates, needed, target, costs):
        raise GenerationTargetNotReached("角色数量、配装条件与指定金数无法同时满足")
    all_candidates_required = needed == len(initial_candidates)
    plans: list[RecommendedTeam] = []
    for cid in selected:
        teams = reference.characters[cid].recommended_teams
        if teams:
            plans.append(rng.choice(teams))
    # 只有核心与非核心需要区分时，才单独枚举核心集合。
    core_count = (
        request.core_count
        if not request.core_character_ids and request.core_count < request.character_count
        else 0
    )

    def choose(
        available: list[str], core_candidates: list[str]
    ) -> Iterator[tuple[tuple[str, ...], tuple[str, ...]]]:
        """搜索下一个角色；完整名单交给调用方验证配装后再继续回溯。

        Args:
            available: 当前分支仍可选择的角色。
            core_candidates: 当前分支仍可选择为核心的角色。
        Yields:
            尚未尝试过的持有集合与核心集合的组合。

        Raises:
            GenerationTargetNotReached: 共享候选预算耗尽。
        """
        needed = request.character_count - len(selected)
        if needed == 0:
            cores = request.core_character_ids or tuple(selected[: request.core_count])
            yield tuple(selected), tuple(dict.fromkeys(cores))
            return
        cores_needed = core_count - len(selected)
        picking_core = cores_needed > 0
        candidates = (core_candidates if picking_core else available).copy()
        minimum_candidates = cores_needed if picking_core else needed
        scores = dict.fromkeys(candidates, 0.0)
        for team in plans:
            for member, backups in zip(team.members, team.backups, strict=True):
                if any(cid in selected for cid in (member, *backups)):
                    continue
                if member in scores:
                    scores[member] += 1
                for backup in backups:
                    if backup in scores:
                        scores[backup] += 0.5
        while len(candidates) >= minimum_candidates:
            total = sum(scores[cid] for cid in candidates)
            weights = [
                (1 - request.teammate_bias) / len(candidates)
                + request.teammate_bias * scores[cid] / total
                if total > 0
                else 1
                for cid in candidates
            ]
            cid = rng.choices(candidates, weights=weights, k=1)[0]
            candidates.remove(cid)
            # 先前分支已覆盖包含其首选角色的组合；跳过的核心候选仍可作为队友。
            if picking_core:
                rest = [other for other in available if other != cid]
            else:
                rest = candidates
            budget.consume()
            if (
                target is not None
                and not all_candidates_required
                and not _roster_fits(selected + [cid], rest, needed - 1, target, costs)
            ):
                continue
            selected.append(cid)
            teams = reference.characters[cid].recommended_teams
            if teams:
                plans.append(rng.choice(teams))
            yield from choose(rest, candidates)
            if teams:
                plans.pop()
            selected.pop()

    yield from choose(initial_candidates, initial_candidates)


def _concentration_weights(
    ids: tuple[str, ...], cores: tuple[str, ...], strength: float
) -> tuple[float, ...]:
    """将平均投入与核心投入按集中程度混合为角色分配权重。"""
    return tuple(
        (1 - strength) / len(ids) + (strength / len(cores) if cid in cores else 0) for cid in ids
    )


def _distribute(
    total: float,
    lower: tuple[float, ...],
    upper: tuple[float, ...],
    weights: tuple[float, ...],
) -> tuple[float, ...]:
    """在各项上下限内按权重分配总量，容量饱和后分配剩余投入。

    Args:
        total: 已落在合计上下限内的总量。
        lower: 各项必需投入。
        upper: 各项可容纳的最高投入。
        weights: 非负分配权重；剩余项权重均为零时平均分配。

    Returns:
        与输入项顺序一致的各项目标投入，总和为 total。
    """
    values = list(lower)
    remaining = total - sum(values)
    active = {i for i in range(len(values)) if upper[i] > values[i] + 1e-10}
    while remaining > 1e-10 and active:
        weight_sum = sum(weights[i] for i in active)
        portions = {
            i: remaining * (weights[i] / weight_sum if weight_sum else 1 / len(active))
            for i in active
        }
        saturated = [i for i in active if portions[i] >= upper[i] - values[i]]
        if not saturated:
            for i in active:
                values[i] += portions[i]
            break
        for i in saturated:
            remaining -= upper[i] - values[i]
            values[i] = upper[i]
            active.remove(i)
    return tuple(values)


def _choose_builds(
    ids: tuple[str, ...],
    cores: tuple[str, ...],
    menus: dict[str, tuple[_Build, ...]],
    request: AccountRequest,
    reference: AccountReference,
    target: int | None,
    rng: random.Random,
    budget: _SearchBudget,
) -> dict[str, _Build] | None:
    """搜索精确半金总数，统一处理活动与模拟宇宙光锥的唯一实例限制。

    Args:
        ids: 已选角色 ID。
        cores: 核心角色 ID。
        menus: 各角色可用的星魂、光锥和叠影组合。
        request: 集中程度、光锥投入倾向和金数口径。
        reference: 光锥的唯一限制与推荐资料。
        target: 精确半金数；None 时随机排列候选金额，逐个满足唯一光锥约束。
        rng: 候选排序与可达总量选择共用的随机源。
        budget: 与角色选择、遗器搜索共用的候选考察预算。

    Returns:
        每名角色实际采用的配装；当前名单和金额均无可用分配时为 None。

    Raises:
        GenerationTargetNotReached: 共享搜索预算耗尽。
    """
    costs = {cid: {build.units for build in menus[cid]} for cid in ids}
    if target is None:
        possible = {0}
        maximum = sum(max(costs[cid]) for cid in ids)
        for cid in ids:
            possible = _add_costs(possible, costs[cid], maximum)
        amounts = sorted(possible)
        rng.shuffle(amounts)
    else:
        amounts = [target]
    for amount in amounts:
        result = _choose_builds_for_amount(
            ids, cores, menus, request, reference, amount, rng, budget
        )
        if result is not None:
            return result
    return None


def _choose_builds_for_amount(
    ids: tuple[str, ...],
    cores: tuple[str, ...],
    menus: dict[str, tuple[_Build, ...]],
    request: AccountRequest,
    reference: AccountReference,
    target: int,
    rng: random.Random,
    budget: _SearchBudget,
) -> dict[str, _Build] | None:
    """按集中程度搜索一个金额下满足唯一光锥约束的配装。

    Args:
        ids: 当前持有角色名单。
        cores: 接受集中投入的核心名单。
        menus: 各角色可用的配装候选。
        request: 投入集中程度和光锥份额。
        reference: 推荐光锥与唯一实例资料。
        target: 本次搜索的精确半金数。
        rng: 候选排序使用的随机源，调用会推进其状态。
        budget: 与角色选择、遗器生成共用的候选考察预算。

    Returns:
        满足金额与唯一实例约束的配装；无可用分配时为 None。

    Raises:
        GenerationTargetNotReached: 共享搜索预算耗尽。
    """
    costs = {cid: {build.units for build in menus[cid]} for cid in ids}
    strength = request.resource_concentration
    if request.gold_concentration is not None:
        strength = request.gold_concentration
    targets = _distribute(
        target,
        tuple(min(costs[cid]) for cid in ids),
        tuple(max(costs[cid]) for cid in ids),
        _concentration_weights(ids, cores, strength),
    )
    desired = dict(zip(ids, targets, strict=True))
    order = sorted(ids, key=lambda cid: len({option.cone_id for option in menus[cid]}))
    suffix = [set() for _ in range(len(ids) + 1)]
    suffix[-1] = {0}
    for position in range(len(ids) - 1, -1, -1):
        suffix[position] = _add_costs(costs[order[position]], suffix[position + 1], target)
    result: dict[str, _Build] = {}
    used_unique: set[str] = set()

    def solve(position: int, remaining: int) -> bool:
        """按剩余预算尝试配装，回溯被占用的唯一光锥。

        Args:
            position: 当前角色在配装搜索顺序中的位置。
            remaining: 尚待分配的半金单位数。

        Returns:
            找到完整分配时返回 True，并写入 result。
            失败分支释放此前占用的唯一光锥。

        Raises:
            GenerationTargetNotReached: 共享搜索预算耗尽。
        """
        if position == len(order):
            return remaining == 0
        cid = order[position]
        options = [
            option for option in menus[cid] if remaining - option.units in suffix[position + 1]
        ]
        # 先接近各角色投入，再接近光锥份额，最后按推荐与随机顺序选择。
        ranked = sorted(
            options,
            key=lambda option: (
                abs(option.units - desired[cid]),
                abs(option.cone_units - option.units * request.light_cone_gold_share),
                option.cone_id not in reference.characters[cid].recommended_light_cone_ids,
                rng.random(),
            ),
        )
        for option in ranked:
            budget.consume()
            unique = reference.light_cones[option.cone_id].unique
            if unique and option.cone_id in used_unique:
                continue
            result[cid] = option
            if unique:
                used_unique.add(option.cone_id)
            if solve(position + 1, remaining - option.units):
                return True
            if unique:
                used_unique.remove(option.cone_id)
            del result[cid]
        return False

    if target in suffix[0] and solve(0, target):
        return result
    return None


def _spec_with_build(
    cid: str, build: _Build, request: AccountRequest, reference: AccountReference
) -> CharacterSpec:
    """把已确定的星魂与光锥叠影写入角色原始生成条件。"""
    spec = _character_spec(cid, request, reference)
    return replace(
        spec,
        eidolons=build.eidolons,
        light_cone=replace(
            spec.light_cone,
            light_cone_id=build.cone_id,
            superimposition=build.superimposition,
        ),
    )
