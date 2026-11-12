# 完整角色配置生成

状态：已实现。输入角色参考资料、原始练度和随机源，输出账号持有角色、光锥实例、六件遗器实例及装备引用。遗器支持推荐配装、自定义条件和五档练度。批量持有角色、金数分配、统一五星配装和集中程度通过[账号生成接口](account-generation.md)控制。

## 在 IDE 中调用

按[单件接口页](relic-generation.md#在-ide-中调用)配置项目解释器，打开 [character.py](../../../examples/account_generation/character.py)，设置 `DATA_DIR`、角色 ID 和请求参数，在 `result` 处查看结果。

`DATA_DIR` 默认为 `local-data/account-reference/`，请设为包含 `character/`、`lightcone.json` 和 `source-manifest.json` 的本地资料目录。帕姆王的玩法配置由局内模块处理。

下面的示例从项目根目录运行：

```python
import random
from pathlib import Path

from research.account_generation import (
    CharacterRequest,
    LightConeSpec,
    generate_character,
    load_character_reference,
)

reference = load_character_reference(
    Path("local-data/account-reference"),
    "1004",
)
request = CharacterRequest(
    config_id="welt-001",
    eidolons=2,
    light_cone=LightConeSpec(light_cone_id="23004", superimposition=1),
    relic_quality_tier=4,
)
result = generate_character(request, reference=reference, rng=random.Random(42))
```

参考资料可在加载后重复使用。生成函数接收已经准备好的参考对象，相同请求、资料与随机源初始状态产生相同结果。调用方记录请求参数和随机种子，用 `dataclasses.asdict(result)` 转成普通字典。

## 角色请求

| 字段 | 含义 | 默认值 |
| --- | --- | --- |
| `config_id` | 调用方分配的配置 ID，也是装备实例 ID 的前缀 | 必填 |
| `level` | 角色原始等级 | 80 |
| `promotion` | 原始突破阶段，取 0～6 | 6 |
| `eidolons` | 星魂数，取 0～6 | 0 |
| `traces` | 完整原始行迹配置 `TraceConfig` | `None`，按当前等级与突破条件升满可用行迹 |
| `light_cone` | 光锥选择与练度 `LightConeSpec` | 同命途推荐光锥随机选择、80 级、突破 6、叠影 1 |
| `relics` | 现有 `CharacterRelicsRequest`，控制部位、权重、得分区间和搜索预算 | `None`，采用推荐配装 |
| `relic_quality_tier` | 五档遗器练度，取 1～5 | `None`，按显式得分条件生成 |

角色与光锥的原始等级范围随突破阶段变化：0～6 阶段分别为 1～20、20～30、30～40、40～50、50～60、60～70、70～80。低练度的原始配置保持原值，后续配置转换应用游戏内保底映射。

生成的角色条目表示账号持有该角色。账号未持有的角色保持缺席，后续实际配置阶段根据角色 ID 补齐试用配置。货币战争形态由局内流程处理。

### 行迹

`TraceConfig` 保存两类字段：

| 字段 | 含义 |
| --- | --- |
| `skill_levels` | 行迹节点 ID 到基础等级的字典；节点关联的多个技能共用等级 |
| `unlocked_nodes` | 已解锁额外能力和属性节点的 ID 元组 |

节点与技能关联、升级条件可从 `reference.traces` 查阅。默认生成覆盖普攻、战技、终结技、天赋、秘技、忆灵技能和欢愉技；解锁时同时检查角色等级、突破阶段和前置节点。

例如瓦尔特的普攻节点为 `1004001`，战技节点为 `1004002`。星魂 6 的角色仍保存普攻基础等级 6、战技基础等级 10，星魂附加等级由配置转换计算。

传入 `traces` 时，生成器原样使用这份完整配置，便于调用方直接控制各项基础等级和节点解锁情况。

### 光锥

| `LightConeSpec` 字段 | 含义 | 默认值 |
| --- | --- | --- |
| `light_cone_id` | 光锥种类 ID，字符串；显式指定时直接采用 | `None`，从同命途推荐列表等概率选择 |
| `level` | 原始等级 | 80 |
| `promotion` | 原始突破阶段 | 6 |
| `superimposition` | 叠影等级，取 1～5 | 1 |

光锥留空时，从同命途推荐列表中等概率选择。显式指定 ID 时直接采用该光锥，支持调用方自定义配装。每次生成一个光锥实例。[账号生成](account-generation.md)统一维护库存和占用关系，按[光锥持有约定](design.md#光锥持有与装备实例)复用活动、模拟宇宙光锥的唯一实例。

## 推荐配装与覆盖

默认从推荐外圈列表选择一个套装，用于部位 1～4；从推荐位面列表选择一个套装，用于部位 5～6。两个列表各自等概率选择。主词条采用各部位的推荐候选，副词条按[推荐权重](design.md#推荐副词条权重)计分。

传入 `relics` 时，其权重、上下限、档位权重和搜索预算直接参与生成；其中明确提供的部位完整覆盖对应推荐，其余部位沿用推荐。已提供部位里的 `None` 字段沿用遗器接口的随机含义。

例如替换球的主词条候选，其他部位保持推荐：

```python
from dataclasses import replace
from research.account_generation import RelicSpec

request = replace(
    request,
    relics=replace(
        reference.relic_request,
        pieces=(RelicSpec(slot=5, mainstat=("HP_", "DEF_")),),
    ),
)
result = generate_character(request, reference=reference, rng=random.Random(42))
```

这里球的套装留空，会从全部合法位面套装中随机选择。指定 `set_id` 可以固定该部位的套装。

`relic_quality_tier` 按[五档练度](relic-quality.md)计算整套有效词条区间，与显式上下限取交集。理论上限 `U` 按主词条、必含副词条、允许数值档位和初始条数限制计算。初始条数留空时包含四词条成品。单件和整套的目标分数用于筛选成品，`U` 保留施加分数目标之前的最高合法得分。

前四档的右端归入下一档，生成时扣除两倍得分比较容差。第五档包含理论上限。采用五档时，角色权重需要允许获得正分。

## 返回结果

| 字段 | 含义 |
| --- | --- |
| `config_id` | 本次配置 ID |
| `character` | 角色 ID、等级、突破、星魂、行迹和装备实例引用 |
| `light_cone` | 光锥实例 ID、种类 ID、等级、突破和叠影 |
| `relics` | 六件遗器实例，含实例 ID 及已有 `Relic` 成品数据 |
| `piece_scores` | 与六件实例对应的有效词条得分 |
| `total_effective_rolls` | 各件得分之和，通过属性读取 |
| `mean_effective_rolls` | 每件平均有效词条数，通过属性读取 |
| `relic_score_upper` | 当前配装的理论有效词条总数上限 |
| `attempts` | 六件遗器共用的实际搜索次数 |
| `game_version`、`data_snapshot` | 所用资料版本与快照标识 |
| `schema_version`、`generator_version` | 输出字段版本与完整角色生成规则版本 |

六件遗器及得分始终按部位 1～6 排列。光锥实例 ID 为 `<config_id>:light-cone:1`，遗器实例 ID 为 `<config_id>:relic:<slot>`。批量生成时，调用方为每份配置分配不同的 `config_id`，账号组装再处理全局库存和共享装备。

当前输出字段版本为 2，完整角色生成规则版本为 `character-v2`。角色通过实例 ID 引用装备，每份装备数值在实例中保存一次。后续[配置转换](data-flow.md)负责补齐试用、应用保底映射和星魂附加等级，并按角色与装备资料计算生效属性、技能数值及机制引用。

## 验证

执行 `uv run --locked python -m unittest discover -s tests -v`。自动测试使用最小本地资料样例，覆盖推荐字段适配、第五项权重、光锥命途筛选、五档目标、四词条理论上限、行迹基础等级、低等级节点解锁、显式条件覆盖、固定种子和装备实例引用。

已完成 60 个角色 × 5 档共 300 组配置的生成检查，各组均满足对应得分范围。游戏内数值核对和配置映射联调按[首个样例要求](design.md#首个样例与完成条件)推进。
