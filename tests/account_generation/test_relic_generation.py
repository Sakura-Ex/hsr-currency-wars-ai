"""单件遗器成品的独立数值核对与搜索验证。"""

import random
import unittest
from collections import Counter
from dataclasses import replace
from itertools import combinations_with_replacement, product

from research.account_generation import (
    GenerationTargetNotReached,
    Relic,
    RelicRequest,
    generate_relic,
    score_relic,
)

# 独立数值样例；百分比使用比例值，按离散档位核对八／九次总量。
ROLL_VALUES = {
    "HP": (33.87004, 38.103798, 42.33755),
    "ATK": (16.935019, 19.051895, 21.168774),
    "DEF": (16.935019, 19.051895, 21.168774),
    "HP_": (0.034560003, 0.038880002, 0.0432),
    "ATK_": (0.034560003, 0.038880002, 0.0432),
    "DEF_": (0.0432, 0.0486, 0.054),
    "SPD": (2.0, 2.3, 2.6),
    "CRIT Rate_": (0.02592, 0.029160001, 0.0324),
    "CRIT DMG_": (0.05184, 0.058320003, 0.0648),
    "Effect Hit Rate_": (0.034560003, 0.038880002, 0.0432),
    "Effect RES_": (0.034560003, 0.038880002, 0.0432),
    "Break Effect_": (0.05184, 0.058320003, 0.0648),
}
ALL_WEIGHTS = dict.fromkeys(ROLL_VALUES, 1)
BASE = RelicRequest(
    initial_substat_count=4,
    set_id="115",
    slot=1,
    mainstat="HP",
    effective_stat_weights={"SPD": 1, "CRIT Rate_": 1, "CRIT DMG_": 1, "ATK_": 0.5},
)


def assert_legal_values(test: unittest.TestCase, relic: Relic, total: int):
    """用独立档位样例检查成品数值是否存在合法分解。

    Args:
        test: 接收断言结果的 unittest 测试实例。
        relic: 待检查副词条最终数值的遗器。
        total: 成品应具有的总获得次数，取八次或九次。

    Raises:
        AssertionError: 某个数值无法分解，或总次数不符合要求。
    """
    possible_counts = []
    for key, value in relic.substats.items():
        counts = {
            count
            for count in range(1, total - 2)
            if any(
                abs(sum(values) - value) < 1e-9
                for values in combinations_with_replacement(ROLL_VALUES[key], count)
            )
        }
        test.assertTrue(counts, (key, value))
        possible_counts.append(counts)
    test.assertTrue(any(sum(counts) == total for counts in product(*possible_counts)))


class RelicGenerationTests(unittest.TestCase):
    """验证单件成品的规则约束、评分、目标搜索和随机复现。"""

    def test_ratio_values_and_flat_values_use_their_actual_units(self):
        """检查生成的百分比使用比例值，速度与固定防御保持原值。"""
        result = generate_relic(
            replace(
                BASE,
                required_substats=("CRIT Rate_", "ATK_", "SPD", "DEF"),
                tier_weights=(0, 0, 1),
                effective_stat_weights={},
            ),
            rng=random.Random(8),
        )
        for key, single in {
            "CRIT Rate_": 0.0324,
            "ATK_": 0.0432,
            "SPD": 2.6,
            "DEF": 21.168774,
        }.items():
            count = result.relic.substats[key] / single
            self.assertAlmostEqual(count, round(count), places=6)
            self.assertGreaterEqual(count, 1)
        self.assertLess(result.relic.substats["CRIT Rate_"], 1)
        self.assertGreaterEqual(result.relic.substats["SPD"], 2.6)

    def test_ratio_units_preserve_mid_roll_scoring(self):
        """检查比例数值下的一条暴击、两条生命和两条速度按权重计分。"""
        relic = Relic(
            "115",
            1,
            "HP",
            {"CRIT Rate_": 0.029160001, "HP_": 0.077760004, "SPD": 4.6, "DEF": 16.935019},
        )
        self.assertAlmostEqual(
            score_relic(relic, effective_stat_weights={"CRIT Rate_": 1, "HP_": 0.5, "SPD": 1}),
            4,
        )

    def test_unspecified_initial_stats_follow_five_to_two_distribution(self):
        """检查默认三／四词条按 5∶2 抽样，成品分别具有八／九次中档数值。"""
        request = RelicRequest(
            set_id="115",
            slot=1,
            mainstat="HP",
            effective_stat_weights=ALL_WEIGHTS,
            tier_weights=(0, 1, 0),
        )
        counts = Counter()
        for seed in range(700):
            result = generate_relic(request, rng=random.Random(seed))
            total = round(result.effective_rolls)
            self.assertIn(total, (8, 9))
            self.assertAlmostEqual(result.effective_rolls, total, places=6)
            counts[total] += 1
        self.assertAlmostEqual(counts[8] / 700, 5 / 7, delta=0.04)
        self.assertAlmostEqual(counts[9] / 700, 2 / 7, delta=0.04)

    def test_initial_count_search_obeys_target_instead_of_fixing_first_draw(self):
        """检查目标可筛选四词条或三词条，显式限制仍参与合法性判断。"""
        for target in (8, 9):
            request = replace(
                BASE,
                initial_substat_count=None,
                effective_stat_weights=ALL_WEIGHTS,
                tier_weights=(0, 1, 0),
                min_effective_rolls=target,
                max_effective_rolls=target,
            )
            for seed in range(20):
                with self.subTest(target=target, seed=seed):
                    result = generate_relic(request, rng=random.Random(seed))
                    self.assertAlmostEqual(result.effective_rolls, target, places=6)
                    assert_legal_values(self, result.relic, target)
        with self.assertRaisesRegex(GenerationTargetNotReached, "超出合法成品范围"):
            generate_relic(
                replace(request, initial_substat_count=3),
                rng=random.Random(0),
            )

    def test_seed_reproduces_result(self):
        """检查相同随机源初始状态能够复现完整生成结果。"""
        self.assertEqual(
            generate_relic(BASE, rng=random.Random(42)),
            generate_relic(BASE, rng=random.Random(42)),
        )

    def test_finished_values_have_a_legal_eight_or_nine_roll_decomposition(self):
        """检查成品的四条副词条能够分解为八次或九次合法档位数值。"""
        for initial in (3, 4):
            for seed in range(10):
                with self.subTest(initial=initial, seed=seed):
                    result = generate_relic(
                        replace(BASE, initial_substat_count=initial), rng=random.Random(seed)
                    )
                    self.assertEqual((result.relic.rarity, result.relic.level), (5, 15))
                    self.assertEqual(len(result.relic.substats), 4)
                    self.assertNotIn("HP", result.relic.substats)
                    assert_legal_values(self, result.relic, initial + 5)

    def test_mainstat_exclusions_and_required_stats(self):
        """检查各部位主副词条排斥及必含副词条共同生效。"""
        for slot, mainstat, excluded, required in (
            (1, "HP", "HP", "HP_"),
            (2, "ATK", "ATK", "ATK_"),
            (3, "HP_", "HP_", "HP"),
            (3, "CRIT Rate_", "CRIT Rate_", "HP"),
            (4, "SPD", "SPD", "ATK_"),
            (5, "Fire DMG Boost_", "Fire DMG Boost_", "ATK_"),
            (6, "Break Effect_", "Break Effect_", "SPD"),
        ):
            result = generate_relic(
                replace(
                    BASE, set_id=None, slot=slot, mainstat=mainstat, required_substats=(required,)
                ),
                rng=random.Random(1),
            )
            keys = result.relic.substats
            self.assertIn(required, keys)
            self.assertNotIn(excluded, keys)

    def test_conflicting_generation_constraints(self):
        """检查不符合游戏规则的主副词条条件报告具体冲突。"""
        for spec in (
            replace(BASE, slot=1, mainstat="SPD"),
            replace(BASE, required_substats=("HP",)),
        ):
            with self.assertRaisesRegex(ValueError, "主词条"):
                generate_relic(spec, rng=random.Random(1))

    def test_all_mid_rolls_score_exactly_eight_or_nine(self):
        """检查全中档、全有效的成品按总获得次数计分。"""
        for initial in (3, 4):
            result = generate_relic(
                replace(
                    BASE,
                    initial_substat_count=initial,
                    effective_stat_weights=ALL_WEIGHTS,
                    tier_weights=(0, 1, 0),
                    min_effective_rolls=initial + 5,
                    max_effective_rolls=initial + 5,
                ),
                rng=random.Random(2),
            )
            self.assertAlmostEqual(result.effective_rolls, initial + 5, places=6)
            for key, value in result.relic.substats.items():
                count = value / ROLL_VALUES[key][1]
                self.assertAlmostEqual(count, round(count), places=6)

    def test_tier_weights_limit_available_values(self):
        """检查零权重档位不会参与成品数值组合。"""
        for tier in (0, 2):
            result = generate_relic(
                replace(
                    BASE,
                    required_substats=("SPD",),
                    tier_weights=tuple(int(i == tier) for i in range(3)),
                ),
                rng=random.Random(3),
            )
            for key, value in result.relic.substats.items():
                count = value / ROLL_VALUES[key][tier]
                self.assertAlmostEqual(count, round(count), places=6)

    def test_role_specific_score_uses_mid_value(self):
        """检查同一遗器按不同角色权重及中档基准正确计分。"""
        relic = Relic("115", 1, "HP", {"SPD": 4.6, "CRIT Rate_": 0.0324})
        self.assertAlmostEqual(score_relic(relic, effective_stat_weights={"SPD": 1}), 2)
        self.assertAlmostEqual(
            score_relic(relic, effective_stat_weights={"CRIT Rate_": 0.5}), 5 / 9, places=6
        )
        self.assertEqual(score_relic(relic, effective_stat_weights={}), 0)

    def test_narrow_target_is_constructed_from_discrete_values(self):
        """检查精确词条目标由合法离散档位组合达成。"""
        result = generate_relic(
            replace(
                BASE,
                effective_stat_weights={"SPD": 1},
                required_substats=("SPD",),
                min_effective_rolls=3,
                max_effective_rolls=3,
            ),
            rng=random.Random(42),
        )
        self.assertAlmostEqual(result.effective_rolls, 3, places=6)
        assert_legal_values(self, result.relic, 9)

    def test_target_interval(self):
        """检查生成结果落在指定有效词条区间。"""
        result = generate_relic(
            replace(
                BASE,
                min_effective_rolls=5,
                max_effective_rolls=6,
                required_substats=("CRIT Rate_", "CRIT DMG_"),
            ),
            rng=random.Random(42),
        )
        self.assertGreaterEqual(result.effective_rolls, 5 - 1e-6)
        self.assertLessEqual(result.effective_rolls, 6 + 1e-6)

    def test_explicit_set_matches_slot_and_initial_count_is_legal(self):
        """检查套装部位和初始三／四条约束，避免生成游戏中不存在的成品。"""
        for slot, set_id in ((1, "301"), (5, "115"), (1, "999")):
            with self.subTest(slot=slot, set_id=set_id):
                with self.assertRaisesRegex(ValueError, "套装"):
                    generate_relic(
                        replace(BASE, slot=slot, set_id=set_id, mainstat=None),
                        rng=random.Random(0),
                    )
        with self.assertRaisesRegex(ValueError, "初始词条数"):
            generate_relic(replace(BASE, initial_substat_count=5), rng=random.Random(0))

    def test_bounds_discrete_gap_and_search_budget(self):
        """区分目标超界、离散无解与搜索预算耗尽。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "超出合法成品范围"):
            generate_relic(
                replace(BASE, min_effective_rolls=100, max_effective_rolls=101),
                rng=random.Random(4),
            )
        with self.assertRaisesRegex(GenerationTargetNotReached, "不存在合法档位组合"):
            generate_relic(
                replace(
                    BASE,
                    effective_stat_weights={"SPD": 1},
                    min_effective_rolls=0.1,
                    max_effective_rolls=0.1,
                ),
                rng=random.Random(4),
            )
        with self.assertRaisesRegex(GenerationTargetNotReached, "预算耗尽"):
            generate_relic(replace(BASE, max_attempts=1), rng=random.Random(4))


if __name__ == "__main__":
    unittest.main()
