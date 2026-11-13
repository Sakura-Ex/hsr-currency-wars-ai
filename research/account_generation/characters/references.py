"""读取本地常规资料，为角色生成准备推荐配装与行迹。"""

import json
from pathlib import Path

from ..relics.models import CharacterRelicsRequest, RelicSpec
from .models import CharacterReference, RecommendedTeam, TraceReference

# 来源属性名称适配到生成器统一键；代码中的百分比数值仍使用比例值。
_PROPERTY_KEYS = {
    "HPDelta": "HP",
    "AttackDelta": "ATK",
    "DefenceDelta": "DEF",
    "HPAddedRatio": "HP_",
    "AttackAddedRatio": "ATK_",
    "DefenceAddedRatio": "DEF_",
    "SpeedDelta": "SPD",
    "CriticalChanceBase": "CRIT Rate_",
    "CriticalDamageBase": "CRIT DMG_",
    "StatusProbabilityBase": "Effect Hit Rate_",
    "StatusResistanceBase": "Effect RES_",
    "BreakDamageAddedRatioBase": "Break Effect_",
    "HealRatioBase": "Outgoing Healing Boost_",
    "SPRatioBase": "Energy Regeneration Rate_",
    "PhysicalAddedRatio": "Physical DMG Boost_",
    "FireAddedRatio": "Fire DMG Boost_",
    "IceAddedRatio": "Ice DMG Boost_",
    "ThunderAddedRatio": "Lightning DMG Boost_",
    "WindAddedRatio": "Wind DMG Boost_",
    "QuantumAddedRatio": "Quantum DMG Boost_",
    "ImaginaryAddedRatio": "Imaginary DMG Boost_",
}

# 推荐副词条按位置赋权，第五项及后续沿用末档。
_RECOMMENDATION_WEIGHTS = (1, 1, 0.5, 0.3)


def load_character_reference(base_dir: Path, character_id: str) -> CharacterReference:
    """读取一个已下载的货币战争角色对应的常规资料。

    Args:
        base_dir: 含 character/、lightcone.json 和 source-manifest.json 的资料目录。
        character_id: 已下载的常规角色 ID。

    Returns:
        版本、行迹条件、推荐套装、主副词条及光锥候选。

    Raises:
        KeyError: 角色或资料字段缺失，或来源属性键尚未适配。
        OSError: 指定路径下的资料无法读取。
    """
    manifest = json.loads((base_dir / "source-manifest.json").read_text(encoding="utf-8"))
    data = json.loads((base_dir / "character" / f"{character_id}.json").read_text(encoding="utf-8"))
    light_cones = json.loads((base_dir / "lightcone.json").read_text(encoding="utf-8"))
    compatible_cones = tuple(
        cone_id for cone_id, entry in light_cones.items() if entry["baseType"] == data["base_type"]
    )
    relics = data["relics"]
    pieces = (
        RelicSpec(slot=1, mainstat="HP"),
        RelicSpec(slot=2, mainstat="ATK"),
        *(
            RelicSpec(
                slot=slot,
                mainstat=tuple(_PROPERTY_KEYS[key] for key in relics[f"property_list{slot}"]),
            )
            for slot in range(3, 7)
        ),
    )
    weights = {
        _PROPERTY_KEYS[key]: _RECOMMENDATION_WEIGHTS[min(index, 3)]
        for index, key in enumerate(relics["sub_affix_property_list"])
    }
    traces = {}
    for point in data["skill_trees"].values():
        first = point["1"]
        traces[str(first["point_id"])] = TraceReference(
            skill_ids=tuple(str(skill_id) for skill_id in first["level_up_skill_id"]),
            level_requirements={
                int(level): (
                    entry["avatar_level_limit"] or 1,
                    entry["avatar_promotion_limit"] or 0,
                )
                for level, entry in point.items()
            },
            prerequisites=tuple(str(node_id) for node_id in first["pre_point"]),
        )
    return CharacterReference(
        character_id=character_id,
        rarity=int(data["rarity"][-1]),
        path=data["base_type"],
        recommended_teams=tuple(
            RecommendedTeam(
                team_id=str(team["team_id"]),
                position=team["position"],
                members=tuple(str(cid) for cid in team["member_list"]),
                backups=tuple(
                    tuple(str(cid) for cid in team[f"backup_list{position}"])
                    for position in range(1, 4)
                ),
                backup_groups=tuple(
                    tuple(str(group) for group in team[f"backup_group_list{position}"])
                    for position in range(1, 4)
                ),
            )
            for team in data["teams"]
        ),
        game_version=manifest["version"],
        data_snapshot=manifest.get("catalog_snapshot"),
        traces=traces,
        relic_request=CharacterRelicsRequest(pieces=pieces, effective_stat_weights=weights),
        outer_set_ids=tuple(str(set_id) for set_id in relics["set4_id_list"]),
        planar_set_ids=tuple(str(set_id) for set_id in relics["set2_id_list"]),
        recommended_light_cone_ids=tuple(
            str(cone_id) for cone_id in data["lightcones"] if str(cone_id) in compatible_cones
        ),
    )
