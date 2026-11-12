"""在 IDE 中设置账号金数、统一配装、遗器水平与投入集中程度。"""

import random
from pathlib import Path
from pprint import pprint

from research.account_generation import (
    AccountRequest,
    generate_account,
    load_account_reference,
)

DATA_DIR = Path(__file__).resolve().parents[2] / "local-data/account-reference"


def example():
    """生成二十金、十二名持有角色的账号，返回完整原始配置。

    Returns:
        角色、光锥与遗器统一库存，以及实际金数和投入统计。
    """
    reference = load_account_reference(DATA_DIR)
    request = AccountRequest(
        account_id="account-example-001",
        character_count=12,
        gold_count=20,
        # 全 01 可设置 gold_count=None、five_star_build=(0, 1)，全 21 使用 (2, 1)。
        five_star_build=None,
        core_character_ids=("1310", "1225"),
        resource_concentration=0.7,
        teammate_bias=0.7,
        min_mean_effective_rolls=4.5,
        max_mean_effective_rolls=5,
    )
    result = generate_account(request, reference=reference, rng=random.Random(20261111))
    pprint(
        {
            "account_id": result.account_id,
            "characters": tuple(character.character_id for character in result.characters),
            "gold_count": result.gold_count,
            "gold_by_category": result.gold_by_category,
            "mean_effective_rolls": result.mean_effective_rolls,
            "core_gold_share": result.core_gold_share,
            "core_relic_share": result.core_relic_share,
            "attempts": result.attempts,
        },
        sort_dicts=False,
    )
    return result


if __name__ == "__main__":
    example()
