"""从已有常规资料加载账号生成所需的角色与光锥。"""

import json
from pathlib import Path

from ..characters.references import load_character_reference
from ..data import BUNDLED_REFERENCE_DIR
from .models import AccountReference, GoldCategory, LightConeReference
from .rules import (
    FREE_CHARACTER_PREFIX,
    STANDARD_CHARACTER_IDS,
    STANDARD_LIGHT_CONE_IDS,
    UNIQUE_LIGHT_CONE_PREFIXES,
)


def load_account_reference(
    base_dir: Path = BUNDLED_REFERENCE_DIR,
    *,
    character_categories: dict[str, GoldCategory] | None = None,
    light_cone_categories: dict[str, GoldCategory] | None = None,
    signature_light_cones: dict[str, str] | None = None,
) -> AccountReference:
    """加载同一版本的账号生成资料，允许覆盖五星资源分类。

    Args:
        base_dir: 参考资料目录，默认使用随模块安装的内置快照。
        character_categories: 覆盖指定角色的限定、常驻、免费或低星分类。
        light_cone_categories: 覆盖指定光锥的五星份数分类。
        signature_light_cones: 覆盖五星统一配装使用的首选光锥 ID，默认取同命途推荐首项。

    Returns:
        可在多次账号生成之间复用的参考资料。

    Raises:
        OSError: 所需本地文件无法读取。
        KeyError: 资料字段缺失或角色属性尚未适配。
        ValueError: 五星角色缺少同命途推荐光锥及专锥覆盖。
    """
    manifest = json.loads((base_dir / "source-manifest.json").read_text(encoding="utf-8"))
    catalog = json.loads((base_dir / "character.json").read_text(encoding="utf-8"))
    cones = json.loads((base_dir / "lightcone.json").read_text(encoding="utf-8"))
    characters = {cid: load_character_reference(base_dir, cid) for cid in catalog}
    categories: dict[str, GoldCategory] = {}
    for cid, profile in characters.items():
        if profile.rarity != 5:
            categories[cid] = "none"
        elif cid.startswith(FREE_CHARACTER_PREFIX):
            categories[cid] = "free"
        elif cid in STANDARD_CHARACTER_IDS:
            categories[cid] = "standard"
        else:
            categories[cid] = "limited"
    if character_categories is not None:
        categories.update(character_categories)
    light_cones = {}
    for cone_id, data in cones.items():
        rarity = int(data["rank"][-1])
        category: GoldCategory
        if rarity != 5:
            category = "none"
        elif cone_id.startswith("24"):
            category = "free"
        elif cone_id in STANDARD_LIGHT_CONE_IDS:
            category = "standard"
        else:
            category = "limited"
        if light_cone_categories is not None and cone_id in light_cone_categories:
            category = light_cone_categories[cone_id]
        light_cones[cone_id] = LightConeReference(
            path=data["baseType"],
            rarity=rarity,
            gold_category=category,
            unique=cone_id.startswith(UNIQUE_LIGHT_CONE_PREFIXES),
        )
    signatures = dict(signature_light_cones or {})
    for cid, profile in characters.items():
        if profile.rarity != 5 or cid in signatures:
            continue
        if not profile.recommended_light_cone_ids:
            raise ValueError(f"五星角色 {cid} 缺少同命途推荐光锥，请指定 signature_light_cones")
        signatures[cid] = profile.recommended_light_cone_ids[0]
    return AccountReference(
        characters=characters,
        character_gold_categories=categories,
        signature_light_cone_ids=signatures,
        light_cones=light_cones,
        game_version=manifest["version"],
        data_snapshot=manifest.get("catalog_snapshot"),
    )
