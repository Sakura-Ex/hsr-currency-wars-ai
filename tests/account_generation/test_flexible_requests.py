"""部分配装自动补齐与独立上下限参数的验证。"""

import random
import unittest
from dataclasses import replace

from research.account_generation import (
    CharacterRelicsRequest,
    GenerationTargetNotReached,
    RelicRequest,
    RelicSpec,
    generate_character_relics,
    generate_relic,
)

from .test_character_relics import BASE as CHARACTER_BASE
from .test_relic_generation import ALL_WEIGHTS, BASE


class FlexibleRequestTests(unittest.TestCase):
    """验证可选配装条件、随机补齐及单边和双边范围约束。"""

    def test_empty_pieces_generate_six_random_legal_relics(self):
        """检查未传部位时自动补齐六件、合法套装及三／四词条总量。"""
        request = CharacterRelicsRequest(effective_stat_weights=ALL_WEIGHTS, tier_weights=(0, 1, 0))
        result = generate_character_relics(request, rng=random.Random(4))
        self.assertEqual(
            tuple(r.slot for r in result.relics),
            (1, 2, 3, 4, 5, 6),
        )
        for relic, score in zip(result.relics, result.piece_scores, strict=True):
            self.assertIn(
                int(relic.set_id),
                range(301, 331) if relic.slot in (5, 6) else range(101, 135),
            )
            self.assertNotIn(relic.mainstat, relic.substats)
            self.assertTrue(abs(score - 8) < 1e-6 or abs(score - 9) < 1e-6)
        self.assertEqual(result, generate_character_relics(request, rng=random.Random(4)))

    def test_partial_specs_preserve_constraints_and_append_missing_slots(self):
        """检查部分部位条件保留，补齐后仍满足整套目标。"""
        request = replace(
            CHARACTER_BASE,
            pieces=(
                RelicSpec(slot=4, mainstat="SPD"),
                RelicSpec(slot=5, set_id="306", mainstat=("HP_", "DEF_")),
            ),
        )
        result = generate_character_relics(request, rng=random.Random(7))
        self.assertEqual(
            tuple(r.slot for r in result.relics),
            (4, 5, 1, 2, 3, 6),
        )
        self.assertEqual(result.relics[0].mainstat, "SPD")
        self.assertIn(int(result.relics[0].set_id), range(101, 135))
        self.assertEqual(result.relics[1].set_id, "306")
        self.assertIn(result.relics[1].mainstat, {"HP_", "DEF_"})
        self.assertGreaterEqual(result.total_effective_rolls, 30 - 1e-6)
        self.assertLessEqual(result.total_effective_rolls, 36 + 1e-6)

    def test_single_relic_can_omit_set_and_mainstat(self):
        """检查单件可按部位随机选择套装和主词条。"""
        result = generate_relic(
            RelicRequest(slot=1, effective_stat_weights={}), rng=random.Random(3)
        )
        self.assertEqual(result.relic.mainstat, "HP")
        self.assertIn(int(result.relic.set_id), range(101, 135))

    def test_single_piece_bounds_are_independent_and_accept_zero(self):
        """检查单件下限、上限及零边界均可独立传入。"""
        lower = generate_relic(replace(BASE, min_effective_rolls=5), rng=random.Random(1))
        self.assertGreaterEqual(lower.effective_rolls, 5 - 1e-6)
        upper = generate_relic(replace(BASE, max_effective_rolls=2), rng=random.Random(1))
        self.assertLessEqual(upper.effective_rolls, 2 + 1e-6)
        zero = generate_relic(
            replace(BASE, effective_stat_weights={}, min_effective_rolls=0, max_effective_rolls=0),
            rng=random.Random(1),
        )
        self.assertEqual(zero.effective_rolls, 0)

    def test_whole_set_bounds_are_independent_and_accept_zero(self):
        """检查整套上下限能分别生效，且零值不会被视为未设置。"""
        lower = generate_character_relics(
            replace(CHARACTER_BASE, pieces=(), max_total_effective_rolls=None),
            rng=random.Random(2),
        )
        self.assertGreaterEqual(lower.total_effective_rolls, 30 - 1e-6)
        upper = generate_character_relics(
            replace(
                CHARACTER_BASE,
                pieces=(),
                min_total_effective_rolls=None,
                max_total_effective_rolls=12,
            ),
            rng=random.Random(2),
        )
        self.assertLessEqual(upper.total_effective_rolls, 12 + 1e-6)
        zero = generate_character_relics(
            CharacterRelicsRequest(effective_stat_weights={}, max_total_effective_rolls=0),
            rng=random.Random(2),
        )
        self.assertEqual(zero.total_effective_rolls, 0)

    def test_reversed_bounds_report_conflict(self):
        """检查分别传入的上下限相互冲突时报告无解。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "超出合法成品范围"):
            generate_relic(
                replace(BASE, min_effective_rolls=6, max_effective_rolls=5), rng=random.Random(1)
            )
        with self.assertRaisesRegex(GenerationTargetNotReached, "不相交"):
            generate_character_relics(
                replace(CHARACTER_BASE, min_total_effective_rolls=20, max_total_effective_rolls=10),
                rng=random.Random(1),
            )


if __name__ == "__main__":
    unittest.main()
