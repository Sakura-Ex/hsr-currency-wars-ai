"""角色六件遗器的目标分配与搜索验证。"""

import random
import unittest
from dataclasses import replace

from research.account_generation import (
    CharacterRelicsRequest,
    GenerationTargetNotReached,
    RelicSpec,
    generate_character_relics,
    score_relic,
)

from .test_relic_generation import ALL_WEIGHTS, assert_legal_values

PIECES = (
    RelicSpec(set_id="115", slot=1, mainstat="HP"),
    RelicSpec(set_id="115", slot=2, mainstat="ATK"),
    RelicSpec(set_id="115", slot=3, mainstat="CRIT Rate_"),
    RelicSpec(set_id="115", slot=4, mainstat="SPD"),
    RelicSpec(set_id="306", slot=5, mainstat="Fire DMG Boost_"),
    RelicSpec(set_id="306", slot=6, mainstat="ATK_"),
)
BASE = CharacterRelicsRequest(
    pieces=PIECES,
    effective_stat_weights={"CRIT Rate_": 1, "CRIT DMG_": 1, "ATK_": 0.5, "SPD": 1},
    min_total_effective_rolls=30,
    max_total_effective_rolls=36,
)


class CharacterRelicTests(unittest.TestCase):
    """验证角色六件遗器的共同目标、部位顺序和共享搜索预算。"""

    def test_full_set_preserves_piece_order_and_both_targets(self):
        """检查各件与整套目标同时达成，并保留部位顺序及合法数值。"""
        pieces = tuple(
            replace(
                p,
                initial_substat_count=3 if i % 2 else 4,
                required_substats=("CRIT DMG_",),
                min_effective_rolls=4,
                max_effective_rolls=7,
            )
            for i, p in enumerate(reversed(PIECES))
        )
        result = generate_character_relics(replace(BASE, pieces=pieces), rng=random.Random(42))
        self.assertEqual(tuple(r.slot for r in result.relics), tuple(p.slot for p in pieces))
        for relic, score, spec in zip(result.relics, result.piece_scores, pieces, strict=True):
            self.assertEqual(relic.set_id, spec.set_id)
            self.assertEqual(relic.mainstat, spec.mainstat)
            self.assertIn("CRIT DMG_", relic.substats)
            self.assertGreaterEqual(score, 4 - 1e-6)
            self.assertLessEqual(score, 7 + 1e-6)
            self.assertAlmostEqual(
                score, score_relic(relic, effective_stat_weights=BASE.effective_stat_weights)
            )
            assert_legal_values(self, relic, spec.initial_substat_count + 5)
        self.assertGreaterEqual(result.total_effective_rolls, 30 - 1e-6)
        self.assertLessEqual(result.total_effective_rolls, 36 + 1e-6)
        self.assertAlmostEqual(result.mean_effective_rolls, result.total_effective_rolls / 6)
        self.assertLessEqual(result.attempts, BASE.max_attempts)

    def test_default_initial_sampling_applies_to_supplied_and_missing_pieces(self):
        """检查显式部位和自动补齐部位共用初始抽样，并尊重指定的三／四词条。"""
        for pieces in ((), PIECES):
            totals = set()
            for seed in range(5):
                result = generate_character_relics(
                    replace(
                        BASE,
                        pieces=pieces,
                        effective_stat_weights=ALL_WEIGHTS,
                        tier_weights=(0, 1, 0),
                        min_total_effective_rolls=None,
                        max_total_effective_rolls=None,
                    ),
                    rng=random.Random(seed),
                )
                for score in result.piece_scores:
                    total = round(score)
                    self.assertIn(total, (8, 9))
                    self.assertAlmostEqual(score, total, places=6)
                    totals.add(total)
            self.assertEqual(totals, {8, 9})
        result = generate_character_relics(
            replace(
                BASE,
                pieces=(
                    replace(PIECES[0], initial_substat_count=3),
                    replace(PIECES[1], initial_substat_count=4),
                ),
                effective_stat_weights=ALL_WEIGHTS,
                tier_weights=(0, 1, 0),
                min_total_effective_rolls=None,
                max_total_effective_rolls=None,
            ),
            rng=random.Random(2),
        )
        self.assertAlmostEqual(result.piece_scores[0], 8, places=6)
        self.assertAlmostEqual(result.piece_scores[1], 9, places=6)

    def test_whole_set_target_can_select_initial_counts(self):
        """检查整套目标能达到四词条上限，并能筛选三／四词条混合组合。"""
        for target in (48, 51, 54):
            request = replace(
                BASE,
                effective_stat_weights=ALL_WEIGHTS,
                tier_weights=(0, 1, 0),
                min_total_effective_rolls=target,
                max_total_effective_rolls=target,
            )
            for seed in range(5):
                with self.subTest(target=target, seed=seed):
                    result = generate_character_relics(request, rng=random.Random(seed))
                    self.assertAlmostEqual(result.total_effective_rolls, target, places=6)
                    four_count = 0
                    for relic, score in zip(result.relics, result.piece_scores, strict=True):
                        total = round(score)
                        self.assertIn(total, (8, 9))
                        self.assertAlmostEqual(score, total, places=6)
                        assert_legal_values(self, relic, total)
                        four_count += total == 9
                    self.assertEqual(four_count, target - 48)

    def test_seed_reproduces_whole_set(self):
        """检查同一随机种子能够复现整套遗器。"""
        self.assertEqual(
            generate_character_relics(BASE, rng=random.Random(9)),
            generate_character_relics(BASE, rng=random.Random(9)),
        )

    def test_exact_whole_set_target_with_mixed_initial_counts(self):
        """检查三／四词条起始混合时，整套精确目标仍能达成。"""
        pieces = tuple(
            replace(p, initial_substat_count=3 if i % 2 else 4) for i, p in enumerate(PIECES)
        )
        result = generate_character_relics(
            replace(
                BASE,
                pieces=pieces,
                effective_stat_weights=ALL_WEIGHTS,
                tier_weights=(0, 1, 0),
                min_total_effective_rolls=51,
                max_total_effective_rolls=51,
            ),
            rng=random.Random(2),
        )
        self.assertAlmostEqual(result.total_effective_rolls, 51, places=6)

    def test_piece_targets_work_without_whole_set_target(self):
        """检查省略整套目标时，各件得分条件独立生效。"""
        result = generate_character_relics(
            replace(
                BASE,
                pieces=tuple(
                    replace(p, min_effective_rolls=3, max_effective_rolls=5) for p in PIECES
                ),
                min_total_effective_rolls=None,
                max_total_effective_rolls=None,
            ),
            rng=random.Random(8),
        )
        self.assertTrue(all(3 - 1e-6 <= score <= 5 + 1e-6 for score in result.piece_scores))

    def test_incompatible_targets_and_budget_are_reported(self):
        """检查单件与整套目标冲突、部位重复和预算耗尽均能报告原因。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "不相交"):
            generate_character_relics(
                replace(
                    BASE,
                    pieces=tuple(
                        replace(p, min_effective_rolls=2, max_effective_rolls=3) for p in PIECES
                    ),
                    min_total_effective_rolls=30,
                    max_total_effective_rolls=36,
                ),
                rng=random.Random(1),
            )
        with self.assertRaisesRegex(GenerationTargetNotReached, "预算耗尽"):
            generate_character_relics(replace(BASE, max_attempts=1), rng=random.Random(1))
        with self.assertRaisesRegex(ValueError, "指定部位需要互异"):
            generate_character_relics(
                replace(BASE, pieces=PIECES[:-1] + (PIECES[0],)),
                rng=random.Random(1),
            )


if __name__ == "__main__":
    unittest.main()
