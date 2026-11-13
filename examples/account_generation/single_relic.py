"""在 IDE 中修改请求参数，在 result 处查看生成结果。"""

import random
from dataclasses import asdict
from pprint import pprint

from research.account_generation import RelicRequest, generate_relic, score_relic


def example():
    """生成单件样例并打印两组角色权重下的评分。

    Returns:
        GeneratedRelic: 满足示例配装条件的生成结果。
    """
    request = RelicRequest(
        set_id="115",
        slot=1,
        mainstat="HP",
        initial_substat_count=None,
        effective_stat_weights={"CRIT Rate_": 1, "CRIT DMG_": 1, "ATK_": 0.5, "SPD": 1},
        required_substats=("CRIT Rate_", "CRIT DMG_"),
        min_effective_rolls=5,
        max_effective_rolls=6,
        tier_weights=(1, 1, 1),
    )
    result = generate_relic(request, rng=random.Random(20261110))
    alternate_score = score_relic(
        result.relic, effective_stat_weights={"HP_": 1, "SPD": 1, "Effect RES_": 0.5}
    )
    pprint(asdict(result), sort_dicts=False)
    print("另一种角色需求的有效词条：", alternate_score)
    return result


if __name__ == "__main__":
    example()
