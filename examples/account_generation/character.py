"""在 IDE 中设置参考资料路径和练度参数，查看完整角色配置。"""

import random
from dataclasses import asdict
from pprint import pprint

from research.account_generation import (
    BUNDLED_REFERENCE_DIR,
    CharacterRequest,
    LightConeSpec,
    generate_character,
    load_character_reference,
)

# 可在 IDE 中选择其他参考资料目录。
DATA_DIR = BUNDLED_REFERENCE_DIR


def example():
    """生成二魂瓦尔特的完整原始配置，使用第四档遗器质量。

    Returns:
        完整角色配置、装备实例、遗器评分与所用资料版本。
    """
    reference = load_character_reference(DATA_DIR, "1004")
    request = CharacterRequest(
        config_id="example-welt-001",
        eidolons=2,
        light_cone=LightConeSpec(light_cone_id="23004", superimposition=1),
        relic_quality_tier=4,
    )
    result = generate_character(request, reference=reference, rng=random.Random(20261110))
    pprint(asdict(result), sort_dicts=False)
    return result


if __name__ == "__main__":
    example()
