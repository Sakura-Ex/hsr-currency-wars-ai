"""完整角色、推荐资料适配和理论练度上限的行为验证。"""

import json
import random
import tempfile
import unittest
from dataclasses import asdict, replace
from pathlib import Path

from research.account_generation import (
    CharacterRelicsRequest,
    CharacterRequest,
    GenerationTargetNotReached,
    LightConeSpec,
    RelicSpec,
    TraceConfig,
    generate_character,
    load_character_reference,
    maximum_character_relic_score,
    score_relic,
)

from .test_relic_generation import ALL_WEIGHTS


def write_reference_fixture(base_dir: Path) -> None:
    """写入用于验证资料适配的最小字段样例。

    样例包含本组测试所需的节点、光锥与推荐配装候选。

    Args:
        base_dir: 测试独占的临时目录。
    """
    traces = {}
    for suffix, maximum in ((1, 6), (2, 10), (3, 10), (4, 10), (7, 1), (420, 10)):
        traces[f"point{suffix}"] = {
            str(level): {
                "point_id": 1004000 + suffix,
                "level_up_skill_id": [100400 + suffix],
                "avatar_level_limit": None,
                "avatar_promotion_limit": None if level == 1 else 6,
                "pre_point": [],
            }
            for level in ({1, maximum})
        }
    traces["extra"] = {
        "1": {
            "point_id": 1004101,
            "level_up_skill_id": [],
            "avatar_level_limit": None,
            "avatar_promotion_limit": 2,
            "pre_point": [],
        }
    }
    traces["attribute"] = {
        "1": {
            "point_id": 1004201,
            "level_up_skill_id": [],
            "avatar_level_limit": 75,
            "avatar_promotion_limit": None,
            "pre_point": [1004101],
        }
    }
    data = {
        "source-manifest.json": {
            "version": "test-version",
            "catalog_snapshot": "test-snapshot",
        },
        "lightcone.json": {
            "23004": {"baseType": "Warlock"},
            "21001": {"baseType": "Warlock"},
            "21015": {"baseType": "Warlock"},
            "20000": {"baseType": "Rogue"},
        },
        "character/1004.json": {
            "base_type": "Warlock",
            "rarity": "CombatPowerAvatarRarityType5",
            "teams": [],
            "skill_trees": traces,
            "relics": {
                "set4_id_list": [115, 117],
                "set2_id_list": [301, 309],
                "property_list3": ["CriticalChanceBase", "CriticalDamageBase"],
                "property_list4": ["SpeedDelta"],
                "property_list5": ["ImaginaryAddedRatio"],
                "property_list6": ["AttackAddedRatio", "SPRatioBase"],
                "sub_affix_property_list": [
                    "CriticalChanceBase",
                    "CriticalDamageBase",
                    "AttackAddedRatio",
                    "SpeedDelta",
                    "StatusProbabilityBase",
                ],
            },
            "lightcones": [23004, 20000, 21001],
        },
    }
    for name, content in data.items():
        path = base_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(content), encoding="utf-8")


class CharacterGenerationTests(unittest.TestCase):
    """验证原始角色配置、资料适配和已有遗器生成的组合。

    Attributes:
        base_dir: 最小参考资料目录，随测试结束清理。
        reference: 通过公开读取接口加载的测试角色资料。
    """

    def setUp(self):
        """为每个用例创建独立参考资料并读取生成依据。"""
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base_dir = Path(directory.name)
        write_reference_fixture(self.base_dir)
        self.reference = load_character_reference(self.base_dir, "1004")

    def test_loader_preserves_ids_candidates_and_ordered_stat_weights(self):
        """检查来源 ID、主词条候选与第五项副词条的末档权重。"""
        self.assertEqual(self.reference.character_id, "1004")
        self.assertEqual(self.reference.game_version, "test-version")
        self.assertEqual(self.reference.data_snapshot, "test-snapshot")
        self.assertEqual(
            self.reference.relic_request.effective_stat_weights,
            {
                "CRIT Rate_": 1,
                "CRIT DMG_": 1,
                "ATK_": 0.5,
                "SPD": 0.3,
                "Effect Hit Rate_": 0.3,
            },
        )
        self.assertEqual(
            self.reference.relic_request.pieces[2].mainstat,
            ("CRIT Rate_", "CRIT DMG_"),
        )
        self.assertEqual(
            self.reference.relic_request.pieces[5].mainstat,
            ("ATK_", "Energy Regeneration Rate_"),
        )
        self.assertEqual(self.reference.recommended_light_cone_ids, ("23004", "21001"))

    def test_full_configuration_reproduces_and_equipment_references_resolve(self):
        """检查固定种子、原始练度、四加二套装、六件评分及可序列化装备引用。"""
        request = CharacterRequest(config_id="example", eidolons=6, relic_quality_tier=4)
        result = generate_character(request, reference=self.reference, rng=random.Random(42))
        self.assertEqual(
            result,
            generate_character(request, reference=self.reference, rng=random.Random(42)),
        )
        self.assertEqual(result.character.level, 80)
        self.assertEqual(result.character.promotion, 6)
        self.assertEqual(result.character.eidolons, 6)
        self.assertEqual(result.character.light_cone_instance_id, result.light_cone.instance_id)
        self.assertEqual(
            result.character.relic_instance_ids,
            tuple(instance.instance_id for instance in result.relics),
        )
        self.assertEqual(tuple(r.relic.slot for r in result.relics), (1, 2, 3, 4, 5, 6))
        self.assertEqual(len({r.relic.set_id for r in result.relics[:4]}), 1)
        self.assertEqual(len({r.relic.set_id for r in result.relics[4:]}), 1)
        self.assertIn(result.relics[0].relic.set_id, self.reference.outer_set_ids)
        self.assertIn(result.relics[4].relic.set_id, self.reference.planar_set_ids)
        weights = self.reference.relic_request.effective_stat_weights
        for instance, score in zip(result.relics, result.piece_scores, strict=True):
            self.assertAlmostEqual(
                score_relic(instance.relic, effective_stat_weights=weights), score
            )
        payload = json.loads(json.dumps(asdict(result)))
        self.assertEqual(payload["schema_version"], 2)
        self.assertNotIn("source", payload["character"])
        self.assertNotIn("form_id", payload["character"])
        self.assertNotIn("source", payload["light_cone"])
        for relic in payload["relics"]:
            self.assertNotIn("source", relic)
        self.assertEqual(result.game_version, "test-version")
        self.assertEqual(result.data_snapshot, "test-snapshot")

    def test_traces_preserve_base_levels_and_include_special_skill_nodes(self):
        """检查星魂保留原始数目，行迹使用基础等级并包含特殊技能节点。"""
        result = generate_character(
            CharacterRequest(config_id="full", eidolons=6),
            reference=self.reference,
            rng=random.Random(1),
        )
        self.assertEqual(result.character.traces.skill_levels["1004001"], 6)
        self.assertEqual(result.character.traces.skill_levels["1004002"], 10)
        self.assertEqual(result.character.traces.skill_levels["1004007"], 1)
        self.assertEqual(result.character.traces.skill_levels["1004420"], 10)
        self.assertEqual(result.character.traces.unlocked_nodes, ("1004101", "1004201"))

    def test_lower_original_level_limits_automatic_trace_unlocks(self):
        """检查低等级原始配置按等级和突破条件生成行迹。"""
        result = generate_character(
            CharacterRequest(config_id="low", level=20, promotion=0),
            reference=self.reference,
            rng=random.Random(1),
        )
        self.assertTrue(all(level == 1 for level in result.character.traces.skill_levels.values()))
        self.assertEqual(result.character.traces.unlocked_nodes, ())
        self.assertEqual(result.character.level, 20)

    def test_explicit_traces_and_cone_investment_are_preserved(self):
        """检查显式行迹和光锥练度保存到原始配置。"""
        traces = TraceConfig(
            skill_levels={
                "1004001": 3,
                "1004002": 8,
                "1004003": 8,
                "1004004": 8,
                "1004007": 1,
                "1004420": 8,
            },
            unlocked_nodes=("1004101",),
        )
        result = generate_character(
            CharacterRequest(
                config_id="custom",
                traces=traces,
                light_cone=LightConeSpec(
                    light_cone_id="21001",
                    level=60,
                    promotion=4,
                    superimposition=5,
                ),
            ),
            reference=self.reference,
            rng=random.Random(5),
        )
        self.assertEqual(result.character.traces, traces)
        self.assertEqual(result.light_cone.light_cone_id, "21001")
        self.assertEqual(result.light_cone.level, 60)
        self.assertEqual(result.light_cone.promotion, 4)
        self.assertEqual(result.light_cone.superimposition, 5)

    def test_partial_relic_override_keeps_other_recommendations_and_score_order(self):
        """检查部分部位覆盖、推荐补齐和最终结果按部位排列。"""
        result = generate_character(
            CharacterRequest(
                config_id="partial",
                relics=replace(
                    self.reference.relic_request,
                    pieces=(RelicSpec(slot=5, set_id="306", mainstat=("HP_", "DEF_")),),
                    min_total_effective_rolls=12,
                    max_total_effective_rolls=18,
                ),
            ),
            reference=self.reference,
            rng=random.Random(4),
        )
        self.assertEqual(result.relics[4].relic.set_id, "306")
        self.assertIn(result.relics[4].relic.mainstat, ("HP_", "DEF_"))
        self.assertEqual(result.relics[3].relic.mainstat, "SPD")
        self.assertGreaterEqual(result.total_effective_rolls, 12 - 1e-6)
        self.assertLessEqual(result.total_effective_rolls, 18 + 1e-6)
        self.assertEqual(tuple(r.relic.slot for r in result.relics), (1, 2, 3, 4, 5, 6))

    def test_all_five_quality_tiers_use_theoretical_upper(self):
        """检查五档按独立确定的上限划分，并能生成理论上限成品。"""
        # 两种有效属性均权重 1，四词条起始共有九次；另两种至少各一次。
        # 中档条件下每件最高七条，六件最高 42 条。
        relics = CharacterRelicsRequest(
            pieces=tuple(
                RelicSpec(
                    slot=slot,
                    mainstat={1: "HP", 2: "ATK"}.get(slot, "HP_"),
                )
                for slot in range(1, 7)
            ),
            effective_stat_weights={"CRIT Rate_": 1, "CRIT DMG_": 1},
            tier_weights=(0, 1, 0),
        )
        ratios = (0, 0.35, 0.5, 0.65, 0.8, 1)
        for tier in range(1, 6):
            result = generate_character(
                CharacterRequest(config_id=f"tier-{tier}", relics=relics, relic_quality_tier=tier),
                reference=self.reference,
                rng=random.Random(42),
            )
            self.assertEqual(result.relic_score_upper, 42)
            score = result.total_effective_rolls
            self.assertGreaterEqual(score, ratios[tier - 1] * 42 - 1e-6)
            if tier < 5:
                self.assertLess(score, ratios[tier] * 42)
            else:
                self.assertLessEqual(score, 42 + 1e-6)
        result = generate_character(
            CharacterRequest(
                config_id="maximum",
                relic_quality_tier=5,
                relics=replace(relics, min_total_effective_rolls=42, max_total_effective_rolls=42),
            ),
            reference=self.reference,
            rng=random.Random(1),
        )
        self.assertAlmostEqual(result.total_effective_rolls, 42, places=6)

    def test_theoretical_upper_uses_initial_limits_and_ignores_score_targets(self):
        """检查上限包含未指定时的四词条，并反映显式起始限制与档位条件。"""
        request = CharacterRelicsRequest(effective_stat_weights=ALL_WEIGHTS, tier_weights=(0, 1, 0))
        self.assertAlmostEqual(maximum_character_relic_score(request), 54)
        limited = replace(
            request,
            pieces=tuple(RelicSpec(slot=slot, initial_substat_count=3) for slot in range(1, 7)),
            min_total_effective_rolls=0,
            max_total_effective_rolls=0,
        )
        self.assertAlmostEqual(maximum_character_relic_score(limited), 48)

    def test_quality_and_explicit_score_constraints_are_intersected(self):
        """检查显式得分条件与练度档位冲突时报告目标无解。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "不相交"):
            generate_character(
                CharacterRequest(
                    config_id="conflict",
                    relic_quality_tier=5,
                    relics=replace(self.reference.relic_request, max_total_effective_rolls=0),
                ),
                reference=self.reference,
                rng=random.Random(1),
            )

    def test_equipment_instance_prefix_distinguishes_configurations(self):
        """检查不同配置标识产生可区分的光锥和遗器实例。"""
        first = generate_character(
            CharacterRequest(config_id="first"),
            reference=self.reference,
            rng=random.Random(1),
        )
        second = generate_character(
            CharacterRequest(config_id="second"),
            reference=self.reference,
            rng=random.Random(1),
        )
        first_ids = {first.light_cone.instance_id, *first.character.relic_instance_ids}
        second_ids = {second.light_cone.instance_id, *second.character.relic_instance_ids}
        self.assertTrue(first_ids.isdisjoint(second_ids))

    def test_explicit_same_path_cone_can_be_outside_recommendations(self):
        """检查同命途非推荐光锥可显式指定，并保留叠影等级。"""
        result = generate_character(
            CharacterRequest(
                config_id="explicit-cone",
                light_cone=LightConeSpec(light_cone_id="21015", superimposition=3),
            ),
            reference=self.reference,
            rng=random.Random(1),
        )
        self.assertEqual(result.light_cone.light_cone_id, "21015")
        self.assertEqual(result.light_cone.superimposition, 3)

    def test_explicit_different_path_cone_is_preserved(self):
        """检查显式指定的异命途光锥及其叠影直接写入原始配置。"""
        result = generate_character(
            CharacterRequest(
                config_id="explicit-path",
                light_cone=LightConeSpec(light_cone_id="20000", superimposition=3),
            ),
            reference=self.reference,
            rng=random.Random(1),
        )
        self.assertEqual(result.light_cone.light_cone_id, "20000")
        self.assertEqual(result.light_cone.superimposition, 3)

    def test_unspecified_cone_uses_same_path_recommendations(self):
        """检查光锥留空时始终从同命途推荐列表选择。"""
        for seed in range(10):
            with self.subTest(seed=seed):
                result = generate_character(
                    CharacterRequest(config_id=f"default-cone-{seed}"),
                    reference=self.reference,
                    rng=random.Random(seed),
                )
                self.assertIn(result.light_cone.light_cone_id, ("23004", "21001"))


if __name__ == "__main__":
    unittest.main()
