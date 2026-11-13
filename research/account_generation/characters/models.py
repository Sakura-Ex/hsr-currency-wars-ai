"""完整角色生成所需的参考资料、原始配置与请求结果。"""

from dataclasses import dataclass, field
from typing import Literal

from ..relics.models import CharacterRelicsRequest, Relic


@dataclass(frozen=True)
class TraceReference:
    """一个行迹节点的升级条件与技能关联。

    Attributes:
        skill_ids: 由此节点提升的技能 ID；空元组表示额外能力或属性节点。
        level_requirements: 基础等级到最低角色等级、突破阶段的映射。
        prerequisites: 解锁此节点需要的前置节点 ID。
    """

    skill_ids: tuple[str, ...]
    level_requirements: dict[int, tuple[int, int]]
    prerequisites: tuple[str, ...]


@dataclass(frozen=True)
class RecommendedTeam:
    """保留一组固定推荐队伍及各位置的替补关系。

    Attributes:
        team_id: 来源中的队伍 ID。
        position: 当前角色在原队伍中的位置。
        members: 其余队友，按来源顺序排列。
        backups: 与三名队友分别对应的替补角色 ID。
        backup_groups: 对应位置的替补分组 ID，保留供后续资料关联。
    """

    team_id: str
    position: int
    members: tuple[str, ...]
    backups: tuple[tuple[str, ...], ...]
    backup_groups: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class CharacterReference:
    """从一个版本的资料中读取的角色生成依据。

    Attributes:
        character_id: 常规角色 ID。
        rarity: 稀有度，取 4 或 5。
        path: 常规命途键。
        recommended_teams: 按队伍及位置保留的固定推荐关系。
        game_version: 资料的游戏版本。
        data_snapshot: 自选资料提供的快照标识，可为 None。
        traces: 行迹节点 ID 与升级条件。
        relic_request: 推荐主词条及角色副词条权重，套装由生成时共同选择。
        outer_set_ids: 推荐外圈套装候选。
        planar_set_ids: 推荐位面饰品候选。
        recommended_light_cone_ids: 按原始顺序保留的同命途推荐光锥 ID。
    """

    character_id: str
    rarity: int
    path: str
    recommended_teams: tuple[RecommendedTeam, ...]
    game_version: str
    data_snapshot: str | None
    traces: dict[str, TraceReference]
    relic_request: CharacterRelicsRequest
    outer_set_ids: tuple[str, ...]
    planar_set_ids: tuple[str, ...]
    recommended_light_cone_ids: tuple[str, ...]


@dataclass(frozen=True, kw_only=True)
class TraceConfig:
    """保存行迹基础等级及已解锁的一次性节点。

    Attributes:
        skill_levels: 行迹节点 ID 到基础等级的字典，节点关联的技能共用此等级；
            包括普攻、战技、终结技、天赋、秘技、忆灵技能和欢愉技。
        unlocked_nodes: 已解锁的额外能力与属性节点 ID。
    """

    skill_levels: dict[str, int]
    unlocked_nodes: tuple[str, ...]


@dataclass(frozen=True, kw_only=True)
class LightConeSpec:
    """指定角色使用的光锥及其原始练度。

    Attributes:
        light_cone_id: 光锥种类 ID，显式指定时直接采用；None 时从同命途推荐列表中等概率选择。
        level: 原始等级。
        promotion: 原始突破阶段，取 0～6。
        superimposition: 叠影等级，取 1～5。
    """

    light_cone_id: str | None = None
    level: int = 80
    promotion: int = 6
    superimposition: int = 1


@dataclass(frozen=True, kw_only=True)
class CharacterSpec:
    """账号持有角色的生成条件，遗器通过现有请求传参。

    Attributes:
        level: 角色原始等级。
        promotion: 原始突破阶段，取 0～6。
        eidolons: 星魂数，取 0～6。
        traces: 原始行迹；None 时按等级与突破阶段解锁全部可用节点并升满。
        light_cone: 光锥选择及练度。
        relics: 遗器条件；None 时使用角色推荐，提供时缺少的部位沿用推荐。
            已提供部位中的 None 字段保留现有随机含义。
        relic_quality_tier: 遗器练度档位 1～5；None 时沿用显式得分条件。
            各档位与显式得分条件共同约束生成结果。
    """

    level: int = 80
    promotion: int = 6
    eidolons: int = 0
    traces: TraceConfig | None = None
    light_cone: LightConeSpec = field(default_factory=LightConeSpec)
    relics: CharacterRelicsRequest | None = None
    relic_quality_tier: Literal[1, 2, 3, 4, 5] | None = None


@dataclass(frozen=True, kw_only=True)
class CharacterRequest(CharacterSpec):
    """单角色调用的原始生成条件，配装字段继承 CharacterSpec。

    Attributes:
        config_id: 调用方分配的配置标识，也用作装备实例 ID 的前缀。
    """

    config_id: str


@dataclass(frozen=True)
class LightConeInstance:
    """一件可以被角色引用的原始光锥。

    Attributes:
        instance_id: 可分配的装备实例 ID。
        light_cone_id: 光锥种类 ID。
        level: 原始等级。
        promotion: 原始突破阶段。
        superimposition: 叠影等级。
    """

    instance_id: str
    light_cone_id: str
    level: int
    promotion: int
    superimposition: int


@dataclass(frozen=True)
class RelicInstance:
    """带实例标识的五星 +15 遗器。

    Attributes:
        instance_id: 可分配的装备实例 ID。
        relic: 遗器种类、部位、主副词条及最终数值。
    """

    instance_id: str
    relic: Relic


@dataclass(frozen=True)
class CharacterConfig:
    """账号持有角色的原始配置与装备引用。

    Attributes:
        character_id: 常规角色 ID。
        level: 原始等级。
        promotion: 原始突破阶段。
        eidolons: 星魂数。
        traces: 原始基础行迹等级与已解锁节点。
        light_cone_instance_id: 所装备的光锥实例 ID。
        relic_instance_ids: 六件已装备遗器的实例 ID，按部位 1～6 排列。
    """

    character_id: str
    level: int
    promotion: int
    eidolons: int
    traces: TraceConfig
    light_cone_instance_id: str
    relic_instance_ids: tuple[str, ...]


@dataclass(frozen=True)
class GeneratedCharacter:
    """完整角色配置、装备实例、资料版本及遗器生成统计。

    Attributes:
        config_id: 调用方分配的配置标识。
        character: 原始角色及装备引用。
        light_cone: 本次生成的光锥实例。
        relics: 按部位 1～6 排列的六件遗器实例。
        piece_scores: 六件遗器各自的有效词条数，与 relics 顺序相同。
        attempts: 六件遗器生成共用的实际搜索次数。
        total_effective_rolls: 各件得分之和，通过属性读取。
        mean_effective_rolls: 每件平均有效词条数，通过属性读取。
        relic_score_upper: 此配装条件的理论得分上限，包含未指定条数时的四词条。
        game_version: 所用游戏资料版本。
        data_snapshot: 自选资料提供的快照标识，可为 None。
        schema_version: 当前输出字段版本。
        generator_version: 当前完整角色生成规则版本。
    """

    config_id: str
    character: CharacterConfig
    light_cone: LightConeInstance
    relics: tuple[RelicInstance, ...]
    piece_scores: tuple[float, ...]
    attempts: int
    relic_score_upper: float
    game_version: str
    data_snapshot: str | None
    schema_version: int = field(default=2, init=False)
    generator_version: str = field(default="character-v2", init=False)

    @property
    def total_effective_rolls(self) -> float:
        """返回六件遗器的有效词条总数。"""
        return sum(self.piece_scores)

    @property
    def mean_effective_rolls(self) -> float:
        """返回每件遗器的平均有效词条数。"""
        return self.total_effective_rolls / len(self.relics)
