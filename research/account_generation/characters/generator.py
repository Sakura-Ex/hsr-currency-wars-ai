"""按角色参考资料组装原始角色、光锥和六件遗器。"""

import random
from dataclasses import replace

from ..relics.models import CharacterRelicsRequest
from ..relics.sets import generate_character_relics, maximum_character_relic_score
from ..relics.single import SCORE_TOLERANCE
from .models import (
    CharacterConfig,
    CharacterReference,
    CharacterRequest,
    CharacterSpec,
    GeneratedCharacter,
    LightConeInstance,
    RelicInstance,
    TraceConfig,
)

# 原始角色和光锥各突破阶段的等级范围，两者使用相同阶段上限。
_PROMOTION_LEVELS = ((1, 20), (20, 30), (30, 40), (40, 50), (50, 60), (60, 70), (70, 80))
# 五档占理论有效词条上限的比例，边界与设计文档一致。
_QUALITY_RATIOS = (0, 0.35, 0.5, 0.65, 0.8, 1)


def _generate_traces(request: CharacterSpec, reference: CharacterReference) -> TraceConfig:
    """按原始等级与突破阶段生成最高可用行迹，或沿用调用方明确配置。

    Args:
        request: 角色练度；显式 traces 是调用方提供的完整原始行迹。
        reference: 行迹等级条件、技能关联及节点前置关系。

    Returns:
        基础行迹等级和已解锁节点，星魂附加等级交给配置转换处理。
    """
    if request.traces is not None:
        return request.traces
    levels: dict[str, int] = {}

    def available_level(node_id: str) -> int:
        """计算节点满足等级、突破和前置关系时可达到的最高基础等级。

        Args:
            node_id: 当前资料中的行迹节点 ID。

        Returns:
            满足条件的最高等级；尚不可用时为零。
        """
        if node_id not in levels:
            node = reference.traces[node_id]
            levels[node_id] = (
                max(
                    (
                        level
                        for level, (minimum_level, promotion) in node.level_requirements.items()
                        if request.level >= minimum_level and request.promotion >= promotion
                    ),
                    default=0,
                )
                if all(available_level(parent) > 0 for parent in node.prerequisites)
                else 0
            )
        return levels[node_id]

    for node_id in reference.traces:
        available_level(node_id)
    return TraceConfig(
        skill_levels={
            node_id: levels[node_id] for node_id, node in reference.traces.items() if node.skill_ids
        },
        unlocked_nodes=tuple(
            node_id
            for node_id, node in reference.traces.items()
            if not node.skill_ids and levels[node_id] > 0
        ),
    )


def _prepare_character_relics(
    request: CharacterSpec,
    reference: CharacterReference,
    rng: random.Random,
) -> tuple[CharacterRelicsRequest, float]:
    """把角色配装与练度档位转换为六件遗器请求，推进随机源。

    Args:
        request: 推荐覆盖、显式分数范围和角色遗器练度档位。
        reference: 当前角色的推荐配装。
        rng: 用于推荐套装选择的随机源。

    Returns:
        已补齐推荐条件的六件请求和施加分数目标之前的理论上限。

    Raises:
        ValueError: 练度档位或对应的正分条件不符合要求。
    """
    outer_set = rng.choice(reference.outer_set_ids)
    planar_set = rng.choice(reference.planar_set_ids)
    relic_request = reference.relic_request if request.relics is None else request.relics
    # 仅显式提供 relics 时，其部位条件完整覆盖对应推荐；其余部位沿用推荐。
    provided = () if request.relics is None else relic_request.pieces
    supplied_slots = {piece.slot for piece in provided}
    recommended = tuple(
        replace(piece, set_id=outer_set if piece.slot <= 4 else planar_set)
        for piece in reference.relic_request.pieces
        if piece.slot not in supplied_slots
    )
    relic_request = replace(relic_request, pieces=provided + recommended)
    theoretical_upper = maximum_character_relic_score(relic_request)
    if request.relic_quality_tier is not None:
        tier = request.relic_quality_tier
        if tier not in range(1, 6):
            raise ValueError("遗器练度档位取 1～5")
        if theoretical_upper <= SCORE_TOLERANCE:
            raise ValueError("五档练度需要至少一种可获得正分的副词条")
        lower = _QUALITY_RATIOS[tier - 1] * theoretical_upper
        upper = _QUALITY_RATIOS[tier] * theoretical_upper
        # 前四档的右端属于下一档，留出比较容差，避免边界交叠。
        if tier < 5:
            upper -= 2 * SCORE_TOLERANCE
        relic_request = replace(
            relic_request,
            min_total_effective_rolls=(
                lower
                if relic_request.min_total_effective_rolls is None
                else max(lower, relic_request.min_total_effective_rolls)
            ),
            max_total_effective_rolls=(
                upper
                if relic_request.max_total_effective_rolls is None
                else min(upper, relic_request.max_total_effective_rolls)
            ),
        )
    return relic_request, theoretical_upper


def _check_character_investment(request: CharacterSpec) -> None:
    """检查原始等级、突破阶段、星魂和光锥叠影的可生成范围。

    Args:
        request: 本次使用的角色和光锥原始练度。

    Raises:
        ValueError: 原始练度超出游戏规定的范围。
    """
    for level, promotion in (
        (request.level, request.promotion),
        (request.light_cone.level, request.light_cone.promotion),
    ):
        if not 0 <= promotion <= 6 or not (
            _PROMOTION_LEVELS[promotion][0] <= level <= _PROMOTION_LEVELS[promotion][1]
        ):
            raise ValueError("角色与光锥的原始等级需要落在对应突破阶段的范围内")
    if not 0 <= request.eidolons <= 6 or not 1 <= request.light_cone.superimposition <= 5:
        raise ValueError("星魂数取 0～6，光锥叠影取 1～5")
    if request.eidolons != int(request.eidolons) or request.light_cone.superimposition != int(
        request.light_cone.superimposition
    ):
        raise ValueError("星魂数和光锥叠影需要为整数")


def generate_character(
    request: CharacterRequest,
    *,
    reference: CharacterReference,
    rng: random.Random,
) -> GeneratedCharacter:
    """生成账号持有角色的原始配置、一个光锥实例和六件五星 +15 遗器。

    Args:
        request: 配置标识、原始练度、光锥及遗器条件。
        reference: 对应角色的版本资料及推荐配装。
        rng: 套装、光锥选择和遗器搜索共用的随机源，调用会推进其状态。

    Returns:
        可交给账号组装和配置转换的原始配置，以及遗器得分与资料版本。
        相同请求、参考资料与随机源初始状态产生相同结果。

    Raises:
        ValueError: 原始等级、星魂数、光锥叠影或遗器练度档位不符合条件。
        GenerationTargetNotReached: 遗器目标冲突、离散无解或搜索预算耗尽。
    """
    _check_character_investment(request)
    cone_id = request.light_cone.light_cone_id
    if cone_id is None:
        cone_id = rng.choice(reference.recommended_light_cone_ids)
    light_cone = LightConeInstance(
        instance_id=f"{request.config_id}:light-cone:1",
        light_cone_id=cone_id,
        level=request.light_cone.level,
        promotion=request.light_cone.promotion,
        superimposition=request.light_cone.superimposition,
    )
    relic_request, theoretical_upper = _prepare_character_relics(request, reference, rng)
    generated = generate_character_relics(relic_request, rng=rng)
    order = sorted(range(6), key=lambda index: generated.relics[index].slot)
    generated = replace(
        generated,
        relics=tuple(generated.relics[index] for index in order),
        piece_scores=tuple(generated.piece_scores[index] for index in order),
    )
    relics = tuple(
        RelicInstance(
            instance_id=f"{request.config_id}:relic:{relic.slot}",
            relic=relic,
        )
        for relic in generated.relics
    )
    return GeneratedCharacter(
        config_id=request.config_id,
        character=CharacterConfig(
            character_id=reference.character_id,
            level=request.level,
            promotion=request.promotion,
            eidolons=request.eidolons,
            traces=_generate_traces(request, reference),
            light_cone_instance_id=light_cone.instance_id,
            relic_instance_ids=tuple(relic.instance_id for relic in relics),
        ),
        light_cone=light_cone,
        relics=relics,
        piece_scores=generated.piece_scores,
        attempts=generated.attempts,
        relic_score_upper=theoretical_upper,
        game_version=reference.game_version,
        data_snapshot=reference.data_snapshot,
    )
