"""在 IDE 中配置角色的六件遗器及整套有效词条目标。"""

import random
from dataclasses import asdict
from pprint import pprint

from research.account_generation import (
    CharacterRelicsRequest,
    RelicSpec,
    generate_character_relics,
)


def example():
    """生成角色六件遗器，并打印各件属性与整套评分。

    Returns:
        GeneratedCharacterRelics: 满足示例整套目标的生成结果。
    """
    request = CharacterRelicsRequest(
        pieces=(
            RelicSpec(set_id="115", slot=1, mainstat="HP"),
            RelicSpec(set_id="115", slot=2, mainstat="ATK"),
            RelicSpec(set_id="115", slot=3, mainstat="CRIT Rate_"),
            RelicSpec(set_id="115", slot=4, mainstat="SPD"),
            RelicSpec(set_id="306", slot=5, mainstat="Fire DMG Boost_"),
            RelicSpec(set_id="306", slot=6, mainstat="ATK_"),
        ),
        effective_stat_weights={"CRIT Rate_": 1, "CRIT DMG_": 1, "ATK_": 0.5, "SPD": 1},
        min_total_effective_rolls=30,
        max_total_effective_rolls=36,
    )
    result = generate_character_relics(request, rng=random.Random(20261110))
    pprint(asdict(result), sort_dicts=False)
    print("合计有效词条：", result.total_effective_rolls)
    print("平均每件有效词条：", result.mean_effective_rolls)
    return result


if __name__ == "__main__":
    example()
