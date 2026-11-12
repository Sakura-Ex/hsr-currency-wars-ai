"""生成满足金数、统一配装、遗器水平和资源分布的原始账号。"""

import random

from ..characters.generator import _generate_traces
from ..characters.models import CharacterConfig, LightConeInstance, RelicInstance
from ..relics.single import GenerationTargetNotReached, _SearchBudget
from .allocation import (
    _build_menus,
    _choose_builds,
    _choose_rosters,
    _spec_with_build,
)
from .models import (
    AccountReference,
    AccountRequest,
    CharacterInvestment,
    GeneratedAccount,
)
from .relics import _generate_account_relics, _RosterRelicTargetNotReached


def generate_account(
    request: AccountRequest,
    *,
    reference: AccountReference,
    rng: random.Random,
) -> GeneratedAccount:
    """生成持有角色与统一装备库存，兑现指定的账号资源条件。

    Args:
        request: 持有人数、半金预算、统一五星配装及资源分配条件。
        reference: 已加载的同版本角色、光锥和推荐关系。
        rng: 角色选择、配装分配和遗器搜索共用的随机源，调用会推进其状态。

    Returns:
        持有角色、统一光锥与遗器库存、核心名单、投入统计和资料版本。
        相同请求、资料与随机源初始状态产生相同结果。

    Raises:
        ValueError: 请求的范围、配装或统计类别不符合接口约定。
        GenerationTargetNotReached: 资源条件无法凑齐、离散目标未找到解或预算耗尽。
        KeyError: 请求引用的角色或光锥不在参考资料中。
    """
    if request.character_count < 0:
        raise ValueError("持有角色数量需要非负")
    if request.gold_count is not None and (
        request.gold_count < 0 or request.gold_count * 2 != round(request.gold_count * 2)
    ):
        raise ValueError("金数需要非负，并以 0.5 金为步长")
    if len(set(request.gold_categories)) != len(request.gold_categories) or not set(
        request.gold_categories
    ) <= {"limited", "standard", "free"}:
        raise ValueError("金数类别采用互异的 limited、standard 或 free")
    for strength in (
        request.resource_concentration,
        request.gold_concentration,
        request.relic_concentration,
        request.teammate_bias,
        request.light_cone_gold_share,
    ):
        if strength is not None and not 0 <= strength <= 1:
            raise ValueError("集中程度、队友关联和光锥份额取 0～1")
    if (
        not request.core_character_ids
        and request.character_count > 0
        and not (0 <= request.core_count <= request.character_count)
    ):
        raise ValueError("核心数量需要落在持有角色数量范围内")
    if request.relic_quality_tier is not None and request.relic_quality_tier not in range(1, 6):
        raise ValueError("遗器练度档位取 1～5")
    if request.five_star_build is not None:
        eidolons, stack = request.five_star_build
        if not 0 <= eidolons <= 6 or not 1 <= stack <= 5:
            raise ValueError("统一五星配装的星魂取 0～6，光锥叠影取 1～5")
    budget = _SearchBudget(request.max_attempts)
    target = None if request.gold_count is None else round(request.gold_count * 2)
    menus = _build_menus(request, reference)
    relic_failure: _RosterRelicTargetNotReached | None = None
    for ids, cores in _choose_rosters(request, reference, menus, target, rng, budget):
        builds = _choose_builds(ids, cores, menus, request, reference, target, rng, budget)
        if builds is None:
            continue
        specs = {cid: _spec_with_build(cid, builds[cid], request, reference) for cid in ids}
        try:
            relic_results, uppers = _generate_account_relics(
                ids, cores, specs, request, reference, rng, budget
            )
        except _RosterRelicTargetNotReached as error:
            relic_failure = error
            continue
        break
    else:
        if relic_failure is not None:
            raise relic_failure
        raise GenerationTargetNotReached("角色选择、金数与光锥唯一实例条件没有找到可用分配")
    characters = []
    cones = []
    relics = []
    investments = {}
    for cid in ids:
        spec = specs[cid]
        build = builds[cid]
        cone = LightConeInstance(
            instance_id=f"{request.account_id}:{cid}:light-cone:1",
            light_cone_id=build.cone_id,
            level=spec.light_cone.level,
            promotion=spec.light_cone.promotion,
            superimposition=build.superimposition,
        )
        cones.append(cone)
        pieces = tuple(
            RelicInstance(
                instance_id=f"{request.account_id}:{cid}:relic:{relic.slot}",
                relic=relic,
            )
            for relic, _ in relic_results[cid]
        )
        relics.extend(pieces)
        characters.append(
            CharacterConfig(
                character_id=cid,
                level=spec.level,
                promotion=spec.promotion,
                eidolons=build.eidolons,
                traces=_generate_traces(spec, reference.characters[cid]),
                light_cone_instance_id=cone.instance_id,
                relic_instance_ids=tuple(piece.instance_id for piece in pieces),
            )
        )
        copies = dict.fromkeys(("limited", "standard", "free", "none"), 0)
        body_category = reference.character_gold_categories[cid]
        if body_category != "none":
            copies[body_category] += build.eidolons + 1
        cone_category = reference.light_cones[build.cone_id].gold_category
        if cone_category != "none":
            copies[cone_category] += build.superimposition
        investments[cid] = CharacterInvestment(
            copies_by_category=copies,
            piece_scores=tuple(score for _, score in relic_results[cid]),
            relic_score_upper=uppers[cid],
        )
    return GeneratedAccount(
        account_id=request.account_id,
        characters=tuple(characters),
        light_cones=tuple(cones),
        relics=tuple(relics),
        core_character_ids=cores,
        investments=investments,
        gold_categories=request.gold_categories,
        game_version=reference.game_version,
        data_snapshot=reference.data_snapshot,
        attempts=budget.attempts,
    )
