"""账号金数、统一配装、推荐关联与库存约束的行为验证。"""

import json
import random
import tempfile
import unittest
from dataclasses import asdict, replace
from pathlib import Path

from research.account_generation import (
    BUNDLED_REFERENCE_DIR,
    AccountRequest,
    CharacterRelicsRequest,
    CharacterSpec,
    GenerationTargetNotReached,
    LightConeSpec,
    RelicSpec,
    generate_account,
    load_account_reference,
    load_character_reference,
)
from research.account_generation.accounts.allocation import (
    _build_menus,
    _choose_rosters,
    _roster_fits,
)
from research.account_generation.characters.models import RecommendedTeam
from research.account_generation.relics.rules import SUBSTAT_ROLLS
from research.account_generation.relics.single import _SearchBudget

from .test_characters import write_reference_fixture


def write_account_fixture(base_dir: Path) -> None:
    """在最小角色样例上构造账号预算所需的四类持有资源。

    Args:
        base_dir: 本用例独占的临时资料目录。
    """
    write_reference_fixture(base_dir)
    template = json.loads((base_dir / "character/1004.json").read_text(encoding="utf-8"))
    rows = {
        "1004": (5, "Warlock", [23004, 23006, 21001, 24003, 22001]),
        "1005": (5, "Warlock", [23006, 23004, 21001, 24003, 22001]),
        "1103": (4, "Mage", [23000, 21006]),
        "8009": (5, "Elation", [23054, 24006, 22007]),
    }
    catalog = {}
    for cid, (rarity, path, recommended) in rows.items():
        data = json.loads(json.dumps(template))
        data["rarity"] = f"CombatPowerAvatarRarityType{rarity}"
        data["base_type"] = path
        data["lightcones"] = recommended
        data["teams"] = []
        for point in data["skill_trees"].values():
            for level in point.values():
                level["point_id"] = int(cid) * 1000 + level["point_id"] % 1000
                level["level_up_skill_id"] = [
                    int(cid) * 100 + skill % 100 for skill in level["level_up_skill_id"]
                ]
                level["pre_point"] = [
                    int(cid) * 1000 + parent % 1000 for parent in level["pre_point"]
                ]
        if cid == "1004":
            data["teams"] = [
                {
                    "team_id": 1,
                    "position": 1,
                    "member_list": [1005, 1306, 1310],
                    "backup_list1": [1103],
                    "backup_list2": [],
                    "backup_list3": [],
                    "backup_group_list1": [101],
                    "backup_group_list2": [],
                    "backup_group_list3": [],
                }
            ]
        (base_dir / "character" / f"{cid}.json").write_text(json.dumps(data), encoding="utf-8")
        catalog[cid] = {"rank": data["rarity"], "baseType": path}
    cones = {
        "23004": (5, "Warlock"),
        "23006": (5, "Warlock"),
        "21001": (4, "Warlock"),
        "22001": (4, "Warlock"),
        "24003": (5, "Warlock"),
        "21006": (4, "Mage"),
        "23000": (5, "Mage"),
        "23054": (5, "Elation"),
        "24006": (5, "Elation"),
        "22007": (4, "Elation"),
    }
    (base_dir / "character.json").write_text(json.dumps(catalog), encoding="utf-8")
    (base_dir / "lightcone.json").write_text(
        json.dumps(
            {
                cone_id: {"rank": f"CombatPowerLightconeRarity{rarity}", "baseType": path}
                for cone_id, (rarity, path) in cones.items()
            }
        ),
        encoding="utf-8",
    )


class AccountGenerationTests(unittest.TestCase):
    """验证账号原始持有数据、分配条件与预算统计。

    Attributes:
        base_dir: 最小账号参考资料目录。
        reference: 通过公开加载接口读取的账号参考资料。
    """

    def setUp(self):
        """为账号用例准备独立的最小资料。"""
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base_dir = Path(directory.name)
        write_account_fixture(self.base_dir)
        self.reference = load_account_reference(self.base_dir)

    def request(self, **changes):
        """构造两名五星角色的快速验证请求，支持覆盖指定字段。"""
        return replace(
            AccountRequest(
                account_id="test",
                character_count=2,
                character_pool=("1004", "1005"),
                core_character_ids=("1005",),
                min_mean_effective_rolls=0,
                max_mean_effective_rolls=0,
            ),
            **changes,
        )

    def generate(self, request, seed=42):
        """使用固定种子调用账号公开生成接口。"""
        return generate_account(request, reference=self.reference, rng=random.Random(seed))

    def test_bundled_reference_loads_without_configured_directory(self):
        """默认资料可同时用于账号和单角色加载，并生成带版本的账号。"""
        cid = "1004"
        reference = load_account_reference()
        self.assertEqual(
            reference.characters[cid], load_character_reference(BUNDLED_REFERENCE_DIR, cid)
        )
        request = self.request(
            character_count=1,
            character_pool=(cid,),
            core_character_ids=(cid,),
            five_star_build=(0, 1),
        )
        result = generate_account(request, reference=reference, rng=random.Random(42))
        self.assertEqual(result.characters[0].character_id, cid)
        self.assertEqual(len(result.relics), 6)
        self.assertTrue(result.game_version)
        self.assertIsNone(result.data_snapshot)

    def test_roster_retries_when_relic_score_target_is_unreachable(self):
        """首个角色的推荐副词条得分为零时，继续寻找能满足词条下限的角色。"""
        low_ceiling = self.reference.characters["1004"]
        self.reference.characters["1004"] = replace(
            low_ceiling,
            relic_request=replace(low_ceiling.relic_request, effective_stat_weights={}),
        )
        request = self.request(
            character_count=1,
            core_character_ids=(),
            min_mean_effective_rolls=1,
            max_mean_effective_rolls=None,
        )
        for tier in (None, 5):
            with self.subTest(tier=tier):
                result = self.generate(replace(request, relic_quality_tier=tier), seed=1)
                self.assertEqual(
                    tuple(character.character_id for character in result.characters), ("1005",)
                )
                self.assertGreaterEqual(result.mean_effective_rolls, 1)

    def test_roster_search_skips_permutations_before_spending_budget(self):
        """从 11 名候选中选 10 名，避开令零分目标不可达的首选角色。"""
        template = self.reference.characters["1004"]
        pool = tuple(str(cid) for cid in range(3001, 3012))
        blocked = pool[1]  # 种子 1 在无队友偏好时先抽到第二名候选。
        for cid in pool:
            pieces = template.relic_request.pieces
            if cid == blocked:
                pieces = (replace(pieces[0], required_substats=("SPD",)), *pieces[1:])
            self.reference.characters[cid] = replace(
                template,
                character_id=cid,
                recommended_teams=(),
                relic_request=replace(
                    template.relic_request,
                    pieces=pieces,
                    effective_stat_weights={"SPD": 1} if cid == blocked else {},
                ),
            )
            self.reference.character_gold_categories[cid] = "standard"
        result = self.generate(
            self.request(
                character_count=10,
                character_pool=pool,
                core_character_ids=(),
                light_cone_pool=("21001",),
                gold_count=5,
                max_attempts=10_000,
            ),
            seed=1,
        )
        self.assertEqual(
            {character.character_id for character in result.characters}, set(pool) - {blocked}
        )
        self.assertEqual(result.gold_count, 5)
        self.assertEqual(result.mean_effective_rolls, 0)

    def test_roster_search_distinguishes_automatic_cores(self):
        """相同持有集合中，任一角色都可以成为自动选择的核心。"""
        request = self.request(core_character_ids=())
        menus = _build_menus(request, self.reference)
        rosters = list(
            _choose_rosters(
                request, self.reference, menus, None, random.Random(1), _SearchBudget(10)
            )
        )
        self.assertCountEqual(
            [(frozenset(ids), cores) for ids, cores in rosters],
            [(frozenset(request.character_pool), (cid,)) for cid in request.character_pool],
        )

    def test_override_guarantees_ownership_and_keeps_automatic_cores_selectable(self):
        """固定持有角色仍可被抽为核心，其他持有角色也保留核心候选资格。"""
        request = self.request(
            core_character_ids=(), character_overrides={"1004": CharacterSpec()}, teammate_bias=0
        )
        self.assertEqual(self.generate(request, seed=0).core_character_ids, ("1005",))
        all_fixed = replace(
            request, character_overrides=dict.fromkeys(request.character_pool, CharacterSpec())
        )
        menus = _build_menus(all_fixed, self.reference)
        rosters = _choose_rosters(
            all_fixed, self.reference, menus, None, random.Random(1), _SearchBudget(10)
        )
        self.assertCountEqual(
            [(frozenset(ids), cores) for ids, cores in rosters],
            [(frozenset(request.character_pool), (cid,)) for cid in request.character_pool],
        )

    def test_roster_gold_reachability_preserves_gaps_for_each_roster_size(self):
        """固定角色与候选的离散半金数按所需人数分别计算。"""
        costs = {"a": {1, 3}, "b": {0, 4}, "c": {2}}
        for count, expected in enumerate(({1, 3}, {1, 3, 5, 7}, {3, 5, 7, 9})):
            with self.subTest(count=count):
                self.assertEqual(
                    {n for n in range(11) if _roster_fits(["a"], ["b", "c"], count, n, costs)},
                    expected,
                )

    def test_roster_search_enumerates_large_core_sets_within_budget(self):
        """全员持有时，从 11 名角色中选 10 个核心，考察全部 11 种核心集合。"""
        template = self.reference.characters["1004"]
        pool = tuple(str(cid) for cid in range(3001, 3012))
        for cid in pool:
            self.reference.characters[cid] = replace(
                template, character_id=cid, recommended_teams=()
            )
            self.reference.character_gold_categories[cid] = "standard"
        request = self.request(
            character_count=11,
            character_pool=pool,
            core_character_ids=(),
            core_count=10,
            light_cone_pool=("21001",),
        )
        menus = _build_menus(request, self.reference)
        rosters = list(
            _choose_rosters(
                request, self.reference, menus, None, random.Random(1), _SearchBudget(100)
            )
        )
        self.assertCountEqual(
            [(frozenset(ids), frozenset(cores)) for ids, cores in rosters],
            [(frozenset(pool), frozenset(pool) - {cid}) for cid in pool],
        )

    def test_reference_classification_and_team_structure(self):
        """检查常驻、限定、免费、唯一光锥及队伍替补分组。"""
        self.assertEqual(self.reference.data_snapshot, "test-snapshot")
        self.assertEqual(
            self.reference.character_gold_categories,
            {
                "1004": "standard",
                "1005": "limited",
                "1103": "none",
                "8009": "free",
            },
        )
        self.assertEqual(self.reference.light_cones["23004"].gold_category, "standard")
        self.assertEqual(self.reference.light_cones["23006"].gold_category, "limited")
        self.assertEqual(self.reference.light_cones["24003"].gold_category, "free")
        self.assertTrue(self.reference.light_cones["22001"].unique)
        self.assertTrue(self.reference.light_cones["24003"].unique)
        self.assertFalse(self.reference.light_cones["21001"].unique)
        team = self.reference.characters["1004"].recommended_teams[0]
        self.assertEqual(team.members, ("1005", "1306", "1310"))
        self.assertEqual(team.backups, (("1103",), (), ()))
        self.assertEqual(team.backup_groups, (("101",), (), ()))
        overridden = load_account_reference(
            self.base_dir,
            character_categories={"1005": "standard"},
            signature_light_cones={"1005": "23004"},
        )
        self.assertEqual(overridden.character_gold_categories["1005"], "standard")
        self.assertEqual(overridden.signature_light_cone_ids["1005"], "23004")

    def test_signature_override_with_no_same_path_recommendation(self):
        """先采用专锥覆盖，缺少覆盖时报告无同命途推荐的角色 ID。"""
        path = self.base_dir / "character/1004.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["lightcones"] = [21006]  # Mage，角色为 Warlock。
        path.write_text(json.dumps(data), encoding="utf-8")

        reference = load_account_reference(self.base_dir, signature_light_cones={"1004": "23004"})
        self.assertEqual(reference.characters["1004"].recommended_light_cone_ids, ())
        self.assertEqual(reference.signature_light_cone_ids["1004"], "23004")
        with self.assertRaisesRegex(ValueError, "1004.*同命途"):
            load_account_reference(self.base_dir)

    def test_standard_copies_count_half_and_inventory_references_resolve(self):
        """检查全 21 两名角色分别贡献常驻两金、限定四金，库存无重复保存。"""
        result = self.generate(self.request(gold_count=6, five_star_build=(2, 1)))
        self.assertEqual(result.gold_count, 6)
        self.assertEqual(result.gold_by_category, {"limited": 4, "standard": 2})
        self.assertEqual(result.copies_by_category["limited"], 4)
        self.assertEqual(result.copies_by_category["standard"], 4)
        self.assertEqual(len(result.characters), 2)
        self.assertEqual(len(result.light_cones), 2)
        self.assertEqual(len(result.relics), 12)
        cone_ids = {cone.instance_id for cone in result.light_cones}
        relic_ids = {relic.instance_id for relic in result.relics}
        self.assertEqual(len(relic_ids), 12)
        for character in result.characters:
            self.assertIn(character.light_cone_instance_id, cone_ids)
            self.assertEqual(len(character.relic_instance_ids), 6)
            self.assertTrue(set(character.relic_instance_ids) <= relic_ids)
        payload = json.loads(json.dumps(asdict(result)))
        self.assertTrue(all("source" not in row for row in payload["characters"]))
        self.assertTrue(all("form_id" not in row for row in payload["characters"]))

    def test_fixed_investment_accepts_integral_values_and_rejects_fractions(self):
        """固定星魂和叠影采用整数份数，允许值为整数的浮点输入。"""
        cone = LightConeSpec(light_cone_id="23004", superimposition=2.0)
        spec = CharacterSpec(eidolons=0.0, light_cone=cone)
        request = self.request(
            character_count=1,
            character_pool=("1004",),
            core_character_ids=("1004",),
            character_overrides={"1004": spec},
            gold_count=1.5,
        )
        result = self.generate(request)
        self.assertEqual(result.gold_count, 1.5)
        self.assertEqual(result.characters[0].eidolons, 0)
        self.assertEqual(result.light_cones[0].superimposition, 2)
        for invalid in (
            replace(spec, eidolons=0.5),
            replace(spec, light_cone=replace(cone, superimposition=1.5)),
        ):
            with self.subTest(spec=invalid), self.assertRaisesRegex(ValueError, "整数"):
                self.generate(replace(request, character_overrides={"1004": invalid}))

    def test_half_gold_budget(self):
        """检查半金预算只能支付一份常驻角色，自动光锥沿用同命途。"""
        result = self.generate(
            self.request(
                gold_count=0.5,
                character_count=1,
                character_pool=("1004",),
                core_character_ids=("1004",),
            )
        )
        self.assertEqual(result.gold_count, 0.5)
        self.assertEqual(result.characters[0].eidolons, 0)
        self.assertIn(result.light_cones[0].light_cone_id, ("21001", "22001", "24003"))

    def test_unified_five_star_builds_work_without_gold_budget(self):
        """检查全 01／全 21 独立生成，四星角色按自身覆盖配置生成。"""
        for eidolons in (0, 2):
            result = self.generate(
                self.request(
                    gold_count=None,
                    five_star_build=(eidolons, 1),
                    character_count=3,
                    character_pool=("1004", "1005", "1103"),
                    character_overrides={
                        "1103": CharacterSpec(
                            eidolons=6,
                            light_cone=LightConeSpec(
                                light_cone_id="21006",
                                superimposition=5,
                            ),
                        ),
                    },
                )
            )
            cones = {cone.instance_id: cone for cone in result.light_cones}
            for character in result.characters:
                cone = cones[character.light_cone_instance_id]
                if character.character_id == "1103":
                    self.assertEqual(character.eidolons, 6)
                    self.assertEqual(cone.superimposition, 5)
                else:
                    self.assertEqual(character.eidolons, eidolons)
                    self.assertEqual(cone.superimposition, 1)
                    self.assertEqual(
                        cone.light_cone_id,
                        self.reference.signature_light_cone_ids[character.character_id],
                    )

    def test_build_and_gold_targets_conflict(self):
        """检查固定全 01 总价三金时，四金条件报告无法同时满足。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "无法同时满足"):
            self.generate(self.request(gold_count=4, five_star_build=(0, 1)))

    def test_selected_gold_categories_and_free_resource_copies(self):
        """检查单独统计限定金，以及免费资源保留份数但不占默认预算。"""
        limited = self.generate(
            self.request(
                gold_count=2,
                five_star_build=(0, 1),
                gold_categories=("limited",),
            )
        )
        self.assertEqual(limited.gold_count, 2)
        self.assertEqual(limited.copies_by_category["standard"], 2)
        free = self.generate(
            self.request(
                gold_count=0,
                character_count=1,
                character_pool=("8009",),
                core_character_ids=("8009",),
                character_overrides={
                    "8009": CharacterSpec(
                        eidolons=2,
                        light_cone=LightConeSpec(
                            light_cone_id="24006",
                            superimposition=5,
                        ),
                    ),
                },
            )
        )
        self.assertEqual(free.gold_count, 0)
        self.assertEqual(free.copies_by_category["free"], 8)

    def test_unique_cones_and_multiple_normal_instances(self):
        """检查唯一光锥自动避让，普通光锥支持同种类多个实例。"""
        automatic = self.generate(
            self.request(
                gold_count=1.5,
                light_cone_pool=("24003", "21001"),
            )
        )
        self.assertLessEqual(
            sum(cone.light_cone_id == "24003" for cone in automatic.light_cones),
            1,
        )
        overrides = {
            cid: CharacterSpec(light_cone=LightConeSpec(light_cone_id="21001"))
            for cid in ("1004", "1005")
        }
        normal = self.generate(self.request(gold_count=1.5, character_overrides=overrides))
        self.assertEqual(tuple(cone.light_cone_id for cone in normal.light_cones), ("21001",) * 2)
        self.assertEqual(len({cone.instance_id for cone in normal.light_cones}), 2)
        unique = {
            cid: CharacterSpec(light_cone=LightConeSpec(light_cone_id="24003"))
            for cid in ("1004", "1005")
        }
        with self.assertRaisesRegex(GenerationTargetNotReached, "唯一实例"):
            self.generate(self.request(gold_count=1.5, character_overrides=unique))

    def test_unspecified_gold_retries_amounts_under_unique_cone_constraint(self):
        """检查未指定金数时继续尝试可用金额，避开重复占用唯一光锥。"""
        request = self.request(
            gold_count=None,
            core_character_ids=("1004",),
            character_overrides={"1004": CharacterSpec(), "1005": CharacterSpec()},
            light_cone_pool=("24003", "23004"),
        )
        for seed in range(6):
            with self.subTest(seed=seed):
                result = self.generate(request, seed)
                self.assertIn(result.gold_count, (2, 2.5))
                self.assertLessEqual(
                    sum(cone.light_cone_id == "24003" for cone in result.light_cones), 1
                )

    def test_roster_retries_when_teammate_requires_reserved_unique_cone(self):
        """检查首选队友与唯一光锥冲突时，继续选取能完成配装的角色。"""
        request = self.request(
            gold_count=None,
            character_pool=("1004", "1005", "1103"),
            core_character_ids=("1004",),
            character_overrides={
                "1004": CharacterSpec(light_cone=LightConeSpec(light_cone_id="24003"))
            },
            light_cone_pool=("24003", "21006"),
            teammate_bias=1,
        )
        for seed in range(6):
            with self.subTest(seed=seed):
                result = self.generate(request, seed)
                self.assertEqual(
                    {character.character_id for character in result.characters},
                    {"1004", "1103"},
                )

    def test_explicit_cross_path_cone_and_original_levels_are_preserved(self):
        """检查角色覆盖保留异命途光锥和低等级原始数据。"""
        result = self.generate(
            self.request(
                gold_count=0.5,
                character_count=1,
                character_pool=("1004",),
                core_character_ids=("1004",),
                light_cone_pool=("23006",),
                character_overrides={
                    "1004": CharacterSpec(
                        level=20,
                        promotion=0,
                        light_cone=LightConeSpec(light_cone_id="21006", superimposition=3),
                    ),
                },
            )
        )
        self.assertEqual(result.characters[0].level, 20)
        self.assertEqual(result.characters[0].promotion, 0)
        self.assertEqual(result.light_cones[0].light_cone_id, "21006")
        self.assertEqual(result.light_cones[0].superimposition, 3)

    def test_gold_and_relic_concentration_change_actual_core_shares(self):
        """检查核心优先分配增加其金数和词条份额，并兑现账号平均范围。"""
        even = self.generate(self.request(gold_count=8, resource_concentration=0))
        focused = self.generate(self.request(gold_count=8, resource_concentration=1))
        self.assertEqual(even.gold_count, 8)
        self.assertEqual(focused.gold_count, 8)
        self.assertGreater(focused.core_gold_share, even.core_gold_share + 0.2)
        relic_request = self.request(
            gold_count=6,
            five_star_build=(2, 1),
            min_mean_effective_rolls=3,
            max_mean_effective_rolls=3.05,
        )
        even_relics = self.generate(relic_request)
        core_relics = self.generate(replace(relic_request, relic_concentration=1))
        for result in (even_relics, core_relics):
            self.assertGreaterEqual(result.mean_effective_rolls, 3 - 1e-6)
            self.assertLessEqual(result.mean_effective_rolls, 3.05 + 1e-6)
        self.assertGreater(core_relics.core_relic_share, even_relics.core_relic_share + 0.35)

    def test_teammate_bias_uses_current_recommended_team(self):
        """检查强关联优先选择已抽队伍中的队友，零关联允许其他角色。"""
        request = self.request(
            core_character_ids=("1004",),
            character_pool=("1004", "1005", "8009"),
            five_star_build=(0, 1),
            teammate_bias=1,
        )
        result = self.generate(request)
        self.assertEqual({c.character_id for c in result.characters}, {"1004", "1005"})
        alternatives = {
            next(
                c.character_id
                for c in self.generate(
                    replace(request, teammate_bias=0),
                    seed,
                ).characters
                if c.character_id != "1004"
            )
            for seed in range(12)
        }
        self.assertEqual(alternatives, {"1005", "8009"})

    def test_selected_backup_fills_one_recommended_slot(self):
        """同一替补覆盖两个位置时，仍优先选择余下位置的首选队友。"""
        team = RecommendedTeam(
            team_id="test",
            position=1,
            members=("1005", "1103"),
            backups=(("8009",), ("8009",)),
            backup_groups=((), ()),
        )
        self.reference.characters["1004"] = replace(
            self.reference.characters["1004"], recommended_teams=(team,)
        )
        request = self.request(
            character_count=3,
            character_pool=("1004", "1005", "1103", "8009"),
            core_character_ids=("1004", "8009"),
            teammate_bias=1,
        )
        result = self.generate(request, seed=1)
        self.assertEqual(
            tuple(character.character_id for character in result.characters),
            ("1004", "8009", "1103"),
        )

    def test_account_quality_tier_and_single_sided_bounds(self):
        """检查账号五档按总理论上限换算，平均词条上下限可独立设置。"""
        request = self.request(
            gold_count=3,
            five_star_build=(0, 1),
            relic_quality_tier=5,
            min_mean_effective_rolls=None,
            max_mean_effective_rolls=None,
        )
        result = self.generate(request)
        upper = sum(item.relic_score_upper for item in result.investments.values())
        score = sum(item.total_effective_rolls for item in result.investments.values())
        self.assertGreaterEqual(score, 0.8 * upper - 1e-6)
        self.assertLessEqual(score, upper + 1e-6)
        for changes in (
            {"min_mean_effective_rolls": 4, "max_mean_effective_rolls": None},
            {"min_mean_effective_rolls": None, "max_mean_effective_rolls": 1},
        ):
            result = self.generate(self.request(**changes))
            if changes["min_mean_effective_rolls"] is not None:
                self.assertGreaterEqual(result.mean_effective_rolls, 4 - 1e-6)
            else:
                self.assertLessEqual(result.mean_effective_rolls, 1 + 1e-6)

    def test_exact_mean_uses_legal_discrete_relic_values(self):
        """检查相同上下限的精确平均词条目标可由合法离散组合凑齐。"""
        result = self.generate(
            self.request(
                gold_count=5,
                min_mean_effective_rolls=4,
                max_mean_effective_rolls=4,
            ),
            seed=49,
        )
        self.assertAlmostEqual(result.mean_effective_rolls, 4, places=6)

    def test_sampled_investment_gap_retries_the_requested_range(self):
        """投入目标落在离散得分之间时，继续寻找请求区间内的合法得分。"""
        relics = CharacterRelicsRequest(
            effective_stat_weights={"SPD": 10},
            tier_weights=(0, 1, 0),
            pieces=(
                RelicSpec(slot=1, required_substats=("SPD", "ATK", "DEF", "HP_")),
                *(
                    RelicSpec(slot=slot, min_effective_rolls=0, max_effective_rolls=0)
                    for slot in range(2, 7)
                ),
            ),
        )
        result = self.generate(
            self.request(
                gold_count=0.5,
                character_count=1,
                character_pool=("1004",),
                core_character_ids=("1004",),
                character_overrides={"1004": CharacterSpec(relics=relics)},
                min_mean_effective_rolls=10 / 6,
                max_mean_effective_rolls=20 / 6,
            )
        )
        head = next(piece.relic for piece in result.relics if piece.relic.slot == 1)
        self.assertIn(head.substats["SPD"], (2.3, 4.6))
        self.assertAlmostEqual(result.mean_effective_rolls, head.substats["SPD"] / 2.3 * 10 / 6)

    def test_strict_targets_on_full_catalog_use_legal_middle_rolls(self):
        """精确值和曾耗尽预算的窄范围均能从 360 件合法遗器凑齐。"""
        reference = load_account_reference()
        for gold, lo, hi in ((120, 3, 3), (120, 2.99, 3.01), (400, 6.5, 6.5)):
            with self.subTest(gold=gold, lo=lo, hi=hi):
                request = AccountRequest(
                    account_id="full-strict",
                    character_count=60,
                    gold_count=gold,
                    min_mean_effective_rolls=lo,
                    max_mean_effective_rolls=hi,
                    max_attempts=10_000,
                )
                result = generate_account(request, reference=reference, rng=random.Random(42))
                self.assertEqual(result.gold_count, gold)
                self.assertEqual(len(result.relics), 360)
                self.assertGreaterEqual(result.mean_effective_rolls, lo - 1e-6)
                self.assertLessEqual(result.mean_effective_rolls, hi + 1e-6)
                for instance in result.relics:
                    rolls = [
                        value / SUBSTAT_ROLLS[key][1]
                        for key, value in instance.relic.substats.items()
                    ]
                    self.assertEqual(len(rolls), 4)
                    self.assertIn(sum(map(round, rolls)), (8, 9))
                    for count in rolls:
                        self.assertAlmostEqual(count, round(count))

    def test_exact_mean_retries_when_middle_grid_is_unavailable(self):
        """全中档分数不可达、禁用中档或网格过细时，通用搜索仍可达标。"""
        for weight, tiers, speed in (
            (1, (1, 1, 1), 2.6),
            (1, (0, 0, 1), 2.6),
            (0.12345, (0, 1, 0), 2.3),
        ):
            with self.subTest(weight=weight, tiers=tiers):
                mean = weight * speed / 2.3 / 6
                relics = CharacterRelicsRequest(
                    effective_stat_weights={"SPD": weight},
                    tier_weights=tiers,
                    pieces=(
                        RelicSpec(slot=1, required_substats=("SPD",)),
                        *(
                            RelicSpec(slot=slot, min_effective_rolls=0, max_effective_rolls=0)
                            for slot in range(2, 7)
                        ),
                    ),
                )
                result = self.generate(
                    self.request(
                        gold_count=0.5,
                        character_count=1,
                        character_pool=("1004",),
                        core_character_ids=("1004",),
                        character_overrides={"1004": CharacterSpec(relics=relics)},
                        min_mean_effective_rolls=mean,
                        max_mean_effective_rolls=mean,
                    )
                )
                self.assertAlmostEqual(result.mean_effective_rolls, mean, places=6)
                head = next(piece.relic for piece in result.relics if piece.relic.slot == 1)
                self.assertAlmostEqual(head.substats["SPD"], speed)

    def test_middle_search_leaves_budget_for_low_and_high_rolls(self):
        """两个部位须分别用低档和高档，全档搜索保有足够的候选额度。"""
        cid = "1004"
        pieces = [
            RelicSpec(slot=slot, min_effective_rolls=0, max_effective_rolls=0)
            for slot in range(1, 5)
        ]
        for slot, speed in ((5, 2.0), (6, 2.6)):
            score = speed / 2.3
            pieces.append(
                RelicSpec(
                    slot=slot,
                    required_substats=("SPD",),
                    min_effective_rolls=score - 0.01,
                    max_effective_rolls=score + 0.01,
                )
            )
        request = self.request(
            character_count=1,
            character_pool=(cid,),
            core_character_ids=(cid,),
            gold_count=0.5,
            character_overrides={
                cid: CharacterSpec(
                    relics=CharacterRelicsRequest(
                        effective_stat_weights={"SPD": 1},
                        pieces=tuple(pieces),
                    )
                )
            },
            min_mean_effective_rolls=2 / 6,
            max_mean_effective_rolls=2 / 6,
            max_attempts=100,
        )
        result = generate_account(
            request, reference=load_account_reference(), rng=random.Random(42)
        )
        self.assertLessEqual(result.attempts, request.max_attempts)
        self.assertAlmostEqual(result.mean_effective_rolls, 2 / 6)
        self.assertEqual([piece.relic.substats["SPD"] for piece in result.relics[4:]], [2.0, 2.6])

    def test_middle_grid_respects_role_bounds_near_score_tolerance(self):
        """边缘格点不能占用还原预算，角色下限稍高时选择下一合法分数。"""
        pool = ("1005", "1004")
        overrides = {
            cid: CharacterSpec(
                relics=CharacterRelicsRequest(
                    effective_stat_weights={"SPD": 1},
                    tier_weights=(0, 1, 0),
                    min_total_effective_rolls=minimum,
                    pieces=(
                        RelicSpec(slot=1, required_substats=("SPD",)),
                        *(
                            RelicSpec(slot=slot, min_effective_rolls=0, max_effective_rolls=0)
                            for slot in range(2, 7)
                        ),
                    ),
                )
            )
            for cid, minimum in zip(pool, (1, 1.0000010001), strict=True)
        }
        request = AccountRequest(
            account_id="tolerance-edge",
            character_count=2,
            character_pool=pool,
            core_character_ids=("1005",),
            character_overrides=overrides,
            resource_concentration=1,
            min_mean_effective_rolls=0.25,
            max_mean_effective_rolls=0.25,
            max_attempts=300,
        )
        result = generate_account(request, reference=load_account_reference(), rng=random.Random(0))
        self.assertAlmostEqual(result.investments["1005"].total_effective_rolls, 1)
        self.assertAlmostEqual(result.investments["1004"].total_effective_rolls, 2)

    def test_relic_fallback_skips_equivalent_zero_score_allocations(self):
        """细权重网格走通用搜索，同分遗器共用一次搜索。"""
        pool = ("1004", "1005", "8009")
        factor = 1.0001
        overrides = {
            cid: CharacterSpec(
                relics=CharacterRelicsRequest(
                    effective_stat_weights={"SPD": weight},
                    min_total_effective_rolls=weight,
                    max_total_effective_rolls=2 * weight,
                    pieces=(
                        RelicSpec(slot=1, required_substats=("SPD", "ATK", "DEF", "HP_")),
                        *(
                            RelicSpec(slot=slot, min_effective_rolls=0, max_effective_rolls=0)
                            for slot in range(2, 7)
                        ),
                    ),
                )
            )
            for cid, weight in zip(pool, (10 * factor, 7 * factor, 0), strict=True)
        }
        result = self.generate(
            self.request(
                character_count=3,
                character_pool=pool,
                core_character_ids=("1004",),
                character_overrides=overrides,
                min_mean_effective_rolls=24 * factor / 18,
                max_mean_effective_rolls=24 * factor / 18,
                resource_concentration=1,
                max_attempts=10_000,
            ),
            seed=2,
        )
        self.assertAlmostEqual(result.mean_effective_rolls, 24 * factor / 18)
        for cid, score in zip(pool, (10 * factor, 14 * factor, 0), strict=True):
            self.assertAlmostEqual(result.investments[cid].total_effective_rolls, score)

    def test_empty_account_and_fixed_seed(self):
        """检查空账号保持角色缺席，完整账号能够按种子复现。"""
        empty = self.generate(AccountRequest(account_id="empty", character_count=0, gold_count=0))
        self.assertEqual(empty.characters, ())
        self.assertEqual(empty.light_cones, ())
        self.assertEqual(empty.relics, ())
        self.assertEqual(empty.gold_count, 0)
        request = self.request(gold_count=5)
        self.assertEqual(self.generate(request), self.generate(request))

    def test_infeasible_targets_and_budget_are_reported(self):
        """检查容量、词条范围、搜索预算和非半金步长能够报告具体原因。"""
        with self.assertRaisesRegex(GenerationTargetNotReached, "无法同时满足"):
            self.generate(self.request(gold_count=100))
        with self.assertRaisesRegex(GenerationTargetNotReached, "不相交"):
            self.generate(self.request(min_mean_effective_rolls=100, max_mean_effective_rolls=101))
        with self.assertRaisesRegex(GenerationTargetNotReached, "预算耗尽"):
            self.generate(self.request(gold_count=3, max_attempts=1))
        with self.assertRaisesRegex(ValueError, "0.5"):
            self.generate(self.request(gold_count=0.25))


if __name__ == "__main__":
    unittest.main()
