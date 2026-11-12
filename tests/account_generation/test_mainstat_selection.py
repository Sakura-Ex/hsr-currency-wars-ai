"""主词条选择及主副词条共用属性键的验证。"""

import json
import random
import unittest
from dataclasses import asdict, replace

from research.account_generation import RelicRequest, generate_character_relics, generate_relic

from .test_character_relics import BASE as CHARACTER_BASE
from .test_relic_generation import assert_legal_values

BASE = RelicRequest(
    initial_substat_count=4,
    set_id="306",
    slot=5,
    mainstat=("HP_", "DEF_"),
    effective_stat_weights={"HP_": 1, "DEF_": 1, "SPD": 1},
)


class MainstatSelectionTests(unittest.TestCase):
    """验证主词条候选、不限主词条和统一属性键下的生成约束。"""

    def test_slot_ids_remain_numbers_in_generated_and_serialized_results(self):
        """检查六个部位 ID 对应正确主词条，且序列化后保持数字。"""
        for slot, mainstat in (
            (1, "HP"),
            (2, "ATK"),
            (3, "CRIT Rate_"),
            (4, "SPD"),
            (5, "Fire DMG Boost_"),
            (6, "Energy Regeneration Rate_"),
        ):
            result = generate_relic(
                RelicRequest(slot=slot, mainstat=mainstat, effective_stat_weights={}),
                rng=random.Random(slot),
            )
            exported = json.loads(json.dumps(asdict(result)))
            self.assertIs(type(result.relic.slot), int)
            self.assertIs(type(exported["relic"]["slot"]), int)
            self.assertEqual(exported["relic"]["slot"], slot)
            self.assertIsInstance(exported["relic"]["substats"], dict)
            self.assertEqual(exported["relic"]["substats"], result.relic.substats)
            self.assertEqual(len(exported["relic"]["substats"]), 4)
            self.assertEqual(result.relic.mainstat, mainstat)

    def test_support_orb_chooses_only_hp_or_defense(self):
        """检查辅助角色的球只从生命%和防御%候选中选择。"""
        seen = set()
        for seed in range(10):
            result = generate_relic(BASE, rng=random.Random(seed))
            mainstat = result.relic.mainstat
            self.assertIn(mainstat, {"HP_", "DEF_"})
            self.assertNotIn(mainstat, result.relic.substats)
            assert_legal_values(self, result.relic, 9)
            seen.add(mainstat)
        self.assertEqual(seen, {"HP_", "DEF_"})

    def test_mainstat_choice_obeys_required_substat(self):
        """检查主词条选择避开必含副词条的排斥关系。"""
        result = generate_relic(replace(BASE, required_substats=("HP_",)), rng=random.Random(1))
        self.assertEqual(result.relic.mainstat, "DEF_")
        self.assertIn("HP_", result.relic.substats)

    def test_score_target_can_require_another_mainstat(self):
        """检查得分目标能够引导搜索选择相容的主词条。"""
        result = generate_relic(
            replace(
                BASE,
                effective_stat_weights={"HP_": 1},
                tier_weights=(0, 1, 0),
                min_effective_rolls=5,
                max_effective_rolls=5,
            ),
            rng=random.Random(3),
        )
        self.assertEqual(result.relic.mainstat, "DEF_")
        self.assertAlmostEqual(result.effective_rolls, 5, places=6)

    def test_unrestricted_mainstats_resolve_and_preserve_whole_set_target(self):
        """检查不限主词条时仍返回明确主词条，并满足整套目标及复现要求。"""
        pieces = tuple(replace(p, mainstat=None) for p in CHARACTER_BASE.pieces)
        request = replace(CHARACTER_BASE, pieces=pieces)
        result = generate_character_relics(request, rng=random.Random(4))
        self.assertEqual(result.relics[0].mainstat, "HP")
        self.assertEqual(result.relics[1].mainstat, "ATK")
        for relic in result.relics:
            self.assertIsInstance(relic.mainstat, str)
            self.assertNotIn(relic.mainstat, relic.substats)
        self.assertGreaterEqual(result.total_effective_rolls, 30 - 1e-6)
        self.assertLessEqual(result.total_effective_rolls, 36 + 1e-6)
        self.assertEqual(result, generate_character_relics(request, rng=random.Random(4)))

    def test_optional_mainstat_and_six_piece_candidate_list(self):
        """检查省略主词条与六件套中的候选列表均可正常调用。"""
        request = RelicRequest(set_id="115", slot=1, effective_stat_weights={})
        self.assertEqual(generate_relic(request, rng=random.Random(1)).relic.mainstat, "HP")
        pieces = tuple(
            replace(p, mainstat=("HP_", "DEF_")) if p.slot == 5 else p
            for p in CHARACTER_BASE.pieces
        )
        result = generate_character_relics(
            replace(CHARACTER_BASE, pieces=pieces), rng=random.Random(5)
        )
        self.assertIn(result.relics[4].mainstat, {"HP_", "DEF_"})
        self.assertGreaterEqual(result.total_effective_rolls, 30 - 1e-6)
        self.assertLessEqual(result.total_effective_rolls, 36 + 1e-6)


if __name__ == "__main__":
    unittest.main()
