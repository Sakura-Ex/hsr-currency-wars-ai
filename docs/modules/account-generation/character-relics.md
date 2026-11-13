# 角色六件遗器生成

状态：已实现。按角色配装需求生成六件五星 +15 遗器，同时满足单件和整套的有效词条上下限。单件接口见[单件遗器生成与评分](relic-generation.md)，属性与套装 ID 见[五星遗器数据](../../game-data/relic-data.md)。

## 在 IDE 中调用

按单件接口页初始化 Python 环境后，打开 [character_relics.py](../../../examples/account_generation/character_relics.py)，修改部位配置和角色权重，在 `result` 处查看结果。

```python
import random

from research.account_generation import (
    CharacterRelicsRequest, RelicSpec, generate_character_relics,
)

request = CharacterRelicsRequest(
    pieces=(
        RelicSpec(slot=4, mainstat="SPD"),
        RelicSpec(slot=5, mainstat=("HP_", "DEF_")),
    ),
    effective_stat_weights={"HP_": 1, "DEF_": 1, "SPD": 1, "Effect RES_": 0.5},
    min_total_effective_rolls=30,
    max_total_effective_rolls=36,
)
result = generate_character_relics(request, rng=random.Random(42))
```

这个请求约束脚部速度主词条、球的生命%或防御%主词条，其余部位自动补齐。每个部位都可以单独提供条件，完整传入六件时按全部指定条件生成。

## 输入参数

| 参数 | 含义 | 默认值 |
| --- | --- | --- |
| `pieces` | 需要约束的互异部位配置；缺少部位随机补齐 | `()`，生成全部六件 |
| `effective_stat_weights` | 角色有效副词条及权重，六件共用 | 必填 |
| `min_total_effective_rolls` | 六件合计有效词条下限 | `None` |
| `max_total_effective_rolls` | 六件合计有效词条上限 | `None` |
| `tier_weights` | 低／中／高档的有限非负排序权重，至少一档为正，零权重档位排除 | `(1, 1, 1)` |
| `max_attempts` | 六件共用的搜索尝试预算 | `100000` |

每个 `RelicSpec` 包含：

| 字段 | 含义 | 默认值 |
| --- | --- | --- |
| `slot` | 部位数字 ID，取值 1～6 | 必填 |
| `set_id` | 套装 ID；未设置时从该部位适用套装中随机选择 | `None` |
| `mainstat` | 一个主词条键、候选键元组，或 `None` 表示不限 | `None` |
| `initial_substat_count` | 3 或 4，分别对应八／九次总量；`None` 时以 5∶2 抽取优先候选 | `None` |
| `required_substats` | 该件必含的副词条 | `()` |
| `min_effective_rolls` | 该件有效词条下限 | `None` |
| `max_effective_rolls` | 该件有效词条上限 | `None` |

上下限分别设置：两端都传时形成闭区间，任意一端为 `None` 时不设该端限制，两端均为空时按其他条件生成。零是有效边界。例如每件可设置 `min_effective_rolls=4, max_effective_rolls=7`，整套再设置合计上下限 30 和 36。

套装按部位分别指定，可表达四件套、两件套和散搭。随机套装按[套装 ID 与名称](../../game-data/relic-data.md#套装-id-与名称)采样，外圈部位使用外圈套装，球与绳使用位面饰品。角色 ID 由上层关联，角色需求由配装配置及权重表达。

## 未指定的条件怎样生成

自动补齐的部位随机选择适用套装，主词条从合法候选中搜索。初始条数留空时，按三∶四 = 5∶2 抽取优先候选，两种起始条件都可以参与整套目标搜索；高分目标可使最终结果中四词条起始的占比提高。显式传入的 `RelicSpec` 采用相同规则，指定 3 或 4 时沿用指定值。套装为 `None` 时随机选择。

主词条省略或传 `None` 时，使用该部位全部合法候选；候选列表如 `("HP_", "DEF_")` 表示生命%或防御%。搜索会避开与必含副词条冲突的主词条，并共同满足副词条得分目标。结果中的套装与主词条均为实际选中的一个 ID 或属性键。

输入部位按原顺序返回；补齐部位按头、手、躯干、脚、球、绳的顺序追加。后续模块按遗器的 `slot` 读取对应部位。

## 怎样满足目标

单件生成直接枚举四种副词条、各属性的获得次数及低／中／高档数量，求和得到最终数值。单件以初始词条数确定八／九次总量，搜索过程中使用可达到的分数范围排除无法满足目标的分配。

六件套先处理范围较窄的部位，根据已分配得分和剩余部位范围，收紧当前部位的目标。后续部位无法凑出合法组合时，搜索返回前一部位尝试其他分配。设置合计目标时，各件得分可以有明显差异，各件上下限可进一步控制质量分布。

搜索预算统计考察的主副词条组合及数值分配选项，六件和回溯共用一份预算。范围冲突、穷尽合法组合或预算耗尽时，抛出 `GenerationTargetNotReached` 并说明原因。预算耗尽表示本次搜索尚未找到结果。

生成约束副词条的有效词条数。角色总速度、能量恢复等属性阈值随配置转换接通后加入。

## 返回结果

| 字段 | 含义 |
| --- | --- |
| `result.relics` | 六件最终遗器，按输入部位及补齐部位的顺序排列 |
| `result.piece_scores` | 各件有效词条数，与遗器顺序对应 |
| `result.total_effective_rolls` | 六件得分之和 |
| `result.mean_effective_rolls` | 合计得分除以六 |
| `result.attempts` | 实际消耗的搜索尝试次数 |

合计与平均值由各件得分计算。`maximum_character_relic_score(request)` 计算配装条件下的理论上限，供[完整角色生成](character-generation.md)换算五档目标。完整角色生成同时分配装备实例 ID；调用方保存生成参数和随机种子。

## 验证

执行 `uv run --locked python -m unittest discover -s tests -v`。测试覆盖整套四词条上限、三／四词条混合目标、部分条件与全部随机补齐、套装和主词条选择、单件及整套上下限、零边界、合法档位分解、固定种子和搜索预算。
