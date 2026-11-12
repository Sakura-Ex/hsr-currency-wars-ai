# 单件遗器生成与评分

状态：已实现。生成器固定输出五星 +15 遗器，供[角色六件套](character-relics.md)和账号生成调用。代码位于 `research/account_generation/`，数据流见[账号数据流与模块职责](data-flow.md)。

## 在 IDE 中调用

使用 Python 3.12 或更高版本，在项目根目录执行 `uv sync --locked`，然后在 IDE 中选择项目 `.venv` 解释器。打开 [single_relic.py](../../../examples/account_generation/single_relic.py)，修改请求参数并运行，在 `result` 处查看结果。

```python
import random

from research.account_generation import RelicRequest, generate_relic

request = RelicRequest(
    set_id="115",
    slot=1,
    mainstat="HP",
    effective_stat_weights={"CRIT Rate_": 1, "CRIT DMG_": 1, "SPD": 1},
    min_effective_rolls=5,
    max_effective_rolls=6,
)
result = generate_relic(request, rng=random.Random(42))
```

生成函数通过参数接收请求和随机源。调用方管理批次、种子和结果保存；相同请求与随机源状态产生相同结果。

## 生成参数

| 参数 | 含义 | 默认值 |
| --- | --- | --- |
| `set_id` | 套装 ID；未设置时从该部位适用套装中随机选择 | `None` |
| `slot` | 部位数字 ID，取值 1～6 | 必填 |
| `mainstat` | 一个主词条键、候选键元组，或 `None` 表示不限 | `None` |
| `effective_stat_weights` | 按角色或打法指定有效副词条及权重；未列出属性计零分 | 必填 |
| `initial_substat_count` | 3 或 4；`None` 时以 5∶2 抽取优先候选 | `None` |
| `required_substats` | 成品必含的副词条，至多四种，与主词条相容 | `()` |
| `min_effective_rolls` | 有效词条下限 | `None` |
| `max_effective_rolls` | 有效词条上限 | `None` |
| `tier_weights` | 低／中／高档的有限非负排序权重，至少一档为正，零权重档位排除 | `(1, 1, 1)` |
| `max_attempts` | 搜索尝试预算 | `10000` |

部位 ID 为整数：1 头部、2 手部、3 躯干、4 脚部、5 位面球、6 连结绳。部位与主词条的对应关系由 `relics/rules.py` 维护。完整数值与 ID／名称对照见[五星遗器数据](../../game-data/relic-data.md)。

副词条 `HP`、`ATK`、`DEF` 为固定值，带下划线的属性为百分比，`SPD` 为速度。文档数值表使用百分比展示；代码和代码示例使用比例值，例如 `CRIT Rate_` 的 `3.24%` 在代码中存为 `0.0324`。主副词条使用同一套键，例如 `HP` 为固定生命，`HP_` 为生命百分比。

主词条可传一个键，例如 `mainstat="HP_"`；可传多个候选，例如 `mainstat=("HP_", "DEF_")`；省略或传 `None` 时，使用该部位全部合法主词条。搜索同时选择主词条和相容的副词条，输出包含实际选中的主词条键。

有效词条上下限分别设置，任意一端可留空；两端均为空时按其他条件生成。零是有效边界。随机套装使用[五星遗器数据](../../game-data/relic-data.md#套装-id-与名称)中与该部位对应的套装范围。

初始条数为 `None` 时，按三∶四 = 5∶2 抽取优先搜索的起始条件，两种起始条件均参与目标搜索。当前条件无法满足目标时，继续搜索另一种。显式指定 3 或 4 时沿用指定值。

5∶2 描述候选的基础概率。两种起始条件均能直接满足目标时，结果保持该抽样倾向；高分目标会筛选出更多四词条起始的成品，每套与整个生成批次的数量由抽样和目标共同决定。

## 生成与评分

生成器从与主词条相容的副词条池中选择四种互异属性，直接分配各属性的获得次数及低／中／高档数量，求和得到最终数值。三词条起始的成品共获得八次数值，四词条起始共获得九次；每种副词条至少获得一次。搜索使用剩余次数与可达到的得分范围筛选合法分配。

有效词条按以下公式计算：

```text
有效词条 = Σ(角色权重 × 副词条实际数值 ÷ 该属性单次中档数值)
```

一次速度高档 `2.6`，按中档 `2.3` 折算约为 `1.130435` 条。通过 `score_relic(relic, effective_stat_weights=...)` 可以按另一种角色需求重新评分。评分统计副词条；主词条数值由后续配置转换取得。

随机源控制副词条集合与候选数值的搜索顺序，档位权重影响候选排序。指定有效词条目标后，搜索优先尝试接近区间中点的分配。生成分布用于控制训练样本，游戏自然掉落概率另行维护。

显式套装 ID 需要适用于指定部位，初始词条数接受 `None`、3 或 4；违反这两项约束时抛出 `ValueError`。

目标比较容差为 `1e-6`。搜索预算统计考察的副词条集合与数值分配选项。目标范围冲突、无合法档位组合或预算耗尽时抛出 `GenerationTargetNotReached`，并说明具体原因。

## 结果与验证

- `result.relic`：套装、部位、主词条、固定的五星品质与 +15 等级、四条副词条。
- `result.relic.substats`：四个属性键到最终数值的字典，例如 `{"SPD": 6.9, "HP_": 0.077760004, "DEF": 21.168774, "Effect RES_": 0.077760004}`。
- `result.effective_rolls`：按本次角色权重计算的得分。
- `result.attempts`：实际消耗的搜索尝试次数。

调用方可以通过标准库 `dataclasses.asdict(result)` 取得普通字典。

自动验证命令为 `uv run --locked python -m unittest discover -s tests -v`。测试覆盖固定种子、5∶2 基础抽样、高分与低分目标对起始条数的筛选、三／四词条起始、八／九次合法数值分解、主副词条排斥、档位数值、角色评分、主词条候选与不限主词条、目标求解和预算耗尽。
