"""遗器成品生成的请求和结果。"""

from dataclasses import dataclass, field
from typing import Literal


@dataclass(frozen=True, kw_only=True)
class RelicSpec:
    """单个部位的成品配装条件，供单件与六件套生成共用。

    Attributes:
        set_id: 套装 ID；None 表示从该部位适用的套装中随机选择。
        slot: 部位数字 ID，1～6 依次为头、手、躯干、脚、球、绳。
        mainstat: 一个主词条键或候选键元组；None 表示该部位主词条不限。
        initial_substat_count: 初始三／四词条，决定成品八／九次获得数值的总量；
            None 时以三词条∶四词条 = 5∶2 抽取优先候选，两种均可参与目标搜索。
        required_substats: 成品必须包含的互异副词条键。
        min_effective_rolls: 单件有效词条下限；None 表示不设该端限制。
        max_effective_rolls: 单件有效词条上限；None 表示不设该端限制。
    """

    slot: int
    set_id: str | None = None
    mainstat: str | tuple[str, ...] | None = None
    initial_substat_count: Literal[3, 4] | None = None
    required_substats: tuple[str, ...] = ()
    min_effective_rolls: float | None = None
    max_effective_rolls: float | None = None


@dataclass(frozen=True, kw_only=True)
class RelicRequest(RelicSpec):
    """生成单件遗器所需的配装条件和搜索控制。

    配装字段继承 RelicSpec。

    Attributes:
        effective_stat_weights: 副词条键及角色权重，未列出的属性计零分。
        tier_weights: 低、中、高档的有限非负相对权重，至少一档为正；
            零权重档位排除，其余权重影响候选排序。
        max_attempts: 单件搜索考察属性集合与数值分配的尝试次数上限。
    """

    effective_stat_weights: dict[str, float]
    tier_weights: tuple[float, float, float] = (1, 1, 1)
    max_attempts: int = 10000


@dataclass(frozen=True, kw_only=True)
class CharacterRelicsRequest:
    """角色六件遗器的配装需求、评分口径和共同目标。

    Attributes:
        pieces: 需要约束的互异部位；缺少的部位随机补齐。结果先按此顺序返回，
            补齐部位按头、手、躯干、脚、球、绳的顺序追加。
        effective_stat_weights: 六件共用的角色副词条权重，未列出属性计零分。
        min_total_effective_rolls: 六件合计有效词条下限；None 表示不设该端限制。
        max_total_effective_rolls: 六件合计有效词条上限；None 表示不设该端限制。
        tier_weights: 低、中、高档的有限非负相对权重，至少一档为正。
        max_attempts: 六件生成与回溯共用的搜索尝试次数上限。
    """

    pieces: tuple[RelicSpec, ...] = ()
    effective_stat_weights: dict[str, float]
    min_total_effective_rolls: float | None = None
    max_total_effective_rolls: float | None = None
    tier_weights: tuple[float, float, float] = (1, 1, 1)
    max_attempts: int = 100000


@dataclass(frozen=True)
class Relic:
    """生成完成的五星 +15 遗器。

    Attributes:
        set_id: 套装 ID。
        slot: 部位数字 ID，取值 1～6。
        mainstat: 实际选中的一个主词条键。
        substats: 四个副词条属性键到最终数值的字典；百分比使用比例值，例如 0.0324 表示 3.24%。
        level: 强化等级，固定为 15。
        rarity: 品质，固定为五星对应的 5。
    """

    set_id: str
    slot: int
    mainstat: str
    substats: dict[str, float]
    level: int = field(default=15, init=False)
    rarity: int = field(default=5, init=False)


@dataclass(frozen=True)
class GeneratedRelic:
    """单件遗器的生成结果与搜索统计。

    Attributes:
        relic: 满足配装条件的成品遗器。
        effective_rolls: 按本次角色权重及单次中档数值折算的有效词条数。
        attempts: 考察属性集合与数值分配时实际消耗的搜索尝试次数。
    """

    relic: Relic
    effective_rolls: float
    attempts: int


@dataclass(frozen=True)
class GeneratedCharacterRelics:
    """角色六件遗器的成品、得分和搜索统计。

    Attributes:
        relics: 与请求中部位顺序一致的六件成品。
        piece_scores: 与 relics 一一对应的各件有效词条得分。
        attempts: 六件生成与回溯累计消耗的搜索尝试次数。
        total_effective_rolls: 由各件得分计算的合计有效词条数。
        mean_effective_rolls: 由合计得分计算的每件平均有效词条数。
    """

    relics: tuple[Relic, ...]
    piece_scores: tuple[float, ...]
    attempts: int

    @property
    def total_effective_rolls(self) -> float:
        """返回六件遗器的有效词条得分之和。"""
        return sum(self.piece_scores)

    @property
    def mean_effective_rolls(self) -> float:
        """返回每件遗器的平均有效词条得分。"""
        return self.total_effective_rolls / len(self.relics)
