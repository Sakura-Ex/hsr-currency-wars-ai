"""账号生成的预算、参考资料、原始库存与投入统计。"""

from dataclasses import dataclass, field
from typing import Literal

from ..characters.models import (
    CharacterConfig,
    CharacterReference,
    CharacterSpec,
    LightConeInstance,
    RelicInstance,
)
from .rules import GOLD_HALF_UNITS

# 统计类别描述本次生成预算的计算口径。
GoldCategory = Literal["limited", "standard", "free", "none"]


@dataclass(frozen=True)
class LightConeReference:
    """账号分配光锥时使用的条目属性。

    Attributes:
        path: 命途键。
        rarity: 稀有度，取 3～5。
        gold_category: 五星份数的统计类别，低星光锥为 none。
        unique: 同种类是否至多持有一个实例。
    """

    path: str
    rarity: int
    gold_category: GoldCategory
    unique: bool


@dataclass(frozen=True)
class AccountReference:
    """同一版本中可生成的角色、光锥和预算分类。

    Attributes:
        characters: 常规角色 ID 到角色生成依据的映射。
        character_gold_categories: 角色五星份数的统计类别。
        signature_light_cone_ids: 五星统一配装使用的首选光锥映射，可在加载时覆盖。
        light_cones: 光锥 ID 到命途、稀有度、统计类别及唯一限制的映射。
        game_version: 游戏资料版本。
        data_snapshot: 数据快照标识。
    """

    characters: dict[str, CharacterReference]
    character_gold_categories: dict[str, GoldCategory]
    signature_light_cone_ids: dict[str, str]
    light_cones: dict[str, LightConeReference]
    game_version: str
    data_snapshot: str


@dataclass(frozen=True, kw_only=True)
class AccountRequest:
    """指定账号规模、金数、遗器水平及投入分布。

    Attributes:
        account_id: 账号配置 ID，也用于生成装备实例 ID。
        gold_count: 可选的指定金数，支持半金步长；常驻每份 0.5 金。
            None 时由其余配装条件确定生成范围。
        five_star_build: 所有持有五星角色的统一星魂数与首选光锥叠影。
            (0, 1) 表示全 01，(2, 1) 表示全 21；角色覆盖优先。
        character_count: 持有角色数量，允许零。
        character_pool: 可选角色 ID；None 使用参考资料中的全部角色。
        core_character_ids: 指定核心角色，必定持有；留空时抽取核心。
        core_count: 核心留空时抽取的核心数量，空账号取零。
        character_overrides: 完整覆盖对应角色的原始生成条件，并保证持有。
            星魂和光锥叠影沿用覆盖值，光锥 ID 留空时仍自动选择。
        light_cone_pool: 自动选择允许使用的光锥；None 使用全部同命途候选。
            显式光锥 ID 直接采用。
        gold_categories: 计入 gold_count 的类别，其他类别仍单独统计。
        resource_concentration: 0 表示平均分配可分配投入，1 表示优先投入核心；
            满足各角色下限后，超出核心容量的投入分配给其他角色。
        gold_concentration: 单独覆盖金数集中程度；None 沿用共同值。
        relic_concentration: 单独覆盖遗器集中程度；None 沿用共同值。
        teammate_bias: 0 表示按候选等概率选择，1 优先选择已抽推荐队伍中的队友；
            中间值混合两种选择。
        light_cone_gold_share: 搜索时光锥份数占角色金数的目标比例，取 0～1。
        min_mean_effective_rolls: 账号全部已装备遗器的平均有效词条下限。
        max_mean_effective_rolls: 同一统计范围的平均有效词条上限，两端独立设置。
        relic_quality_tier: 用账号理论总上限换算五档区间，与显式范围取交集。
        max_attempts: 金数分配与遗器搜索共用的候选考察次数上限。
    """

    account_id: str
    gold_count: float | None = None
    five_star_build: tuple[int, int] | None = None
    character_count: int
    character_pool: tuple[str, ...] | None = None
    core_character_ids: tuple[str, ...] = ()
    core_count: int = 1
    character_overrides: dict[str, CharacterSpec] = field(default_factory=dict)
    light_cone_pool: tuple[str, ...] | None = None
    gold_categories: tuple[GoldCategory, ...] = ("limited", "standard")
    resource_concentration: float = 0
    gold_concentration: float | None = None
    relic_concentration: float | None = None
    teammate_bias: float = 0.7
    light_cone_gold_share: float = 0.25
    min_mean_effective_rolls: float | None = None
    max_mean_effective_rolls: float | None = None
    relic_quality_tier: Literal[1, 2, 3, 4, 5] | None = None
    max_attempts: int = 1_000_000


@dataclass(frozen=True)
class CharacterInvestment:
    """一名持有角色的投入统计，得分随角色权重计算。

    Attributes:
        copies_by_category: 角色本体、星魂和所装备光锥的五星份数。
        piece_scores: 按部位 1～6 排列的六件有效词条得分。
        relic_score_upper: 当前配装的理论最高有效词条总数。
        total_effective_rolls: 该角色六件得分之和，通过属性读取。
    """

    copies_by_category: dict[GoldCategory, int]
    piece_scores: tuple[float, ...]
    relic_score_upper: float

    @property
    def total_effective_rolls(self) -> float:
        """返回该角色六件遗器的有效词条总数。"""
        return sum(self.piece_scores)


@dataclass(frozen=True)
class GeneratedAccount:
    """账号实际持有的角色、装备库存与生成统计。

    Attributes:
        account_id: 账号配置 ID。
        characters: 持有角色，条目中的实例 ID 引用下方统一库存。
        light_cones: 光锥实例库存，每件实例分配给一个角色。
        relics: 遗器实例库存，每个角色对应部位 1～6。
        core_character_ids: 本次采用的核心角色 ID。
        investments: 各角色的金数分类与遗器得分。
        gold_categories: 本次指定金数采用的类别。
        game_version: 所用游戏资料版本。
        data_snapshot: 所用数据快照标识。
        attempts: 金数与遗器搜索实际考察次数。
        schema_version: 账号输出字段版本。
        generator_version: 账号生成规则版本。
        copies_by_category: 各类别五星持有份数，通过属性读取。
        gold_by_category: 当前统计口径的各类别金数，通过属性读取。
        gold_count: 本次口径下的实际总金数，通过属性读取。
        mean_effective_rolls: 已装备遗器的平均有效词条，通过属性读取。
        core_gold_share: 核心角色占实际金数的比例，通过属性读取。
        core_relic_share: 核心角色占遗器总分的比例，通过属性读取。
    """

    account_id: str
    characters: tuple[CharacterConfig, ...]
    light_cones: tuple[LightConeInstance, ...]
    relics: tuple[RelicInstance, ...]
    core_character_ids: tuple[str, ...]
    investments: dict[str, CharacterInvestment]
    gold_categories: tuple[GoldCategory, ...]
    game_version: str
    data_snapshot: str
    attempts: int
    schema_version: int = field(default=1, init=False)
    generator_version: str = field(default="account-v1", init=False)

    @property
    def copies_by_category(self) -> dict[GoldCategory, int]:
        """返回持有角色与所装备光锥的五星份数分类统计。"""
        return {
            category: sum(item.copies_by_category[category] for item in self.investments.values())
            for category in ("limited", "standard", "free", "none")
        }

    @property
    def gold_by_category(self) -> dict[GoldCategory, float]:
        """返回当前预算口径下各类别的折算金数，常驻每份为半金。"""
        copies = self.copies_by_category
        return {
            category: copies[category] * GOLD_HALF_UNITS[category] / 2
            for category in self.gold_categories
        }

    @property
    def gold_count(self) -> float:
        """返回本次预算口径下的实际金数。"""
        return sum(self.gold_by_category.values())

    @property
    def mean_effective_rolls(self) -> float:
        """返回已装备遗器的每件平均有效词条数，空账号为零。"""
        return (
            sum(item.total_effective_rolls for item in self.investments.values()) / len(self.relics)
            if self.relics
            else 0
        )

    @property
    def core_gold_share(self) -> float:
        """返回核心角色使用的预算金数占比，零金账号为零。"""
        core = sum(
            self.investments[cid].copies_by_category[category] * GOLD_HALF_UNITS[category] / 2
            for cid in self.core_character_ids
            for category in self.gold_categories
        )
        return core / self.gold_count if self.gold_count else 0

    @property
    def core_relic_share(self) -> float:
        """返回核心角色的有效词条得分占比，零分账号为零。"""
        total = sum(item.total_effective_rolls for item in self.investments.values())
        core = sum(self.investments[cid].total_effective_rolls for cid in self.core_character_ids)
        return core / total if total else 0
