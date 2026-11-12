# 账号数据生成

状态：已实现。日期：2026-11-11。按持有人数、金数、统一五星配装、遗器水平和投入分布生成原始账号。输出持有角色及统一的光锥、遗器库存，供后续实际配置转换使用。

## 在 IDE 中调用

按[单件接口页](relic-generation.md#在-ide-中调用)配置项目解释器，打开 [account.py](../../../examples/account_generation/account.py)，设置 `DATA_DIR` 和请求参数，在 `result` 处查看完整账号。

`DATA_DIR` 默认为 `local-data/account-reference/`，请设为实际资料目录，其中应包含 `source-manifest.json`、`character.json`、`character/` 和 `lightcone.json`。单元测试自带参考资料样例。

```python
import random
from pathlib import Path

from research.account_generation import (
    AccountRequest, generate_account, load_account_reference,
)

reference = load_account_reference(
    Path("local-data/account-reference")
)
request = AccountRequest(
    account_id="account-001",
    character_count=12,
    gold_count=20,
    core_character_ids=("1310", "1225"),
    resource_concentration=0.7,
    teammate_bias=0.7,
    min_mean_effective_rolls=4.5,
    max_mean_effective_rolls=5,
)
result = generate_account(request, reference=reference, rng=random.Random(42))
```

参考资料可重复使用。调用方保存请求参数、随机种子、代码版本与数据快照，相同输入和随机源初始状态产生相同结果。角色缺席表达未持有，试用补齐及生效数值由后续[实际配置转换](data-flow.md)处理。

## 生成条件

| 参数 | 含义 | 默认值 |
| --- | --- | --- |
| `account_id` | 账号 ID，也是装备实例 ID 前缀 | 必填 |
| `character_count` | 持有角色总数 | 必填，允许 0 |
| `gold_count` | 精确金数，支持 0.5 金步长 | `None`，由配装条件中的可达总量随机确定 |
| `five_star_build` | 所有持有五星的统一星魂和专锥叠影，如 `(0, 1)`、`(2, 1)` | `None` |
| `character_pool` | 允许持有的常规角色 ID；数量等于候选数时选取全部 | `None`，使用全部参考角色 |
| `core_character_ids` | 指定核心角色，保证持有 | `()` |
| `core_count` | 未指定核心时，以首先选中的这些角色为核心 | 1，空账号为 0 |
| `character_overrides` | 角色 ID 到 `CharacterSpec` 的完整覆盖，并保证持有 | `{}` |
| `light_cone_pool` | 自动配装可选的光锥 ID，按角色命途筛选 | `None`，使用全部同命途候选 |
| `gold_categories` | 计入预算的资源类别 | `("limited", "standard")` |
| `resource_concentration` | 同时控制金数和遗器的集中程度，取 0～1 | 0 |
| `gold_concentration` | 独立覆盖金数集中程度 | `None`，沿用共同值 |
| `relic_concentration` | 独立覆盖遗器集中程度 | `None`，沿用共同值 |
| `teammate_bias` | 推荐队友关联强度，取 0～1 | 0.7 |
| `light_cone_gold_share` | 配装搜索中光锥金数占角色投入的目标比例 | 0.25 |
| `min_mean_effective_rolls` | 全部已装备遗器的每件平均有效词条下限 | `None` |
| `max_mean_effective_rolls` | 同一范围的平均有效词条上限 | `None` |
| `relic_quality_tier` | 按账号理论总上限换算的五档遗器目标 | `None` |
| `max_attempts` | 角色选择、金数分配和遗器搜索共用的候选考察预算 | 1000000 |

核心和覆盖角色需要在候选范围内，持有人数需要能容纳这些角色。未提供固定练度的角色，在预算内选择星魂 0～6、光锥叠影 1～5；原始等级和突破采用角色生成的 80 级、阶段 6 默认值。

每名持有角色生成一个光锥实例和六件五星 +15 遗器。金数容量由持有人数、星魂上限、光锥叠影及显式覆盖共同决定。

## 金数与统一五星配装

五星角色每个本体及星魂对应一份，五星光锥每级叠影对应一份：

| 类别 | 标识 | 每份折算 | 默认计入预算 |
| --- | --- | ---: | --- |
| 限定 | `limited` | 1 金 | 是 |
| 常驻 | `standard` | 0.5 金 | 是 |
| 免费五星 | `free` | 显式纳入时按 1 金统计 | 否 |
| 四星及以下 | `none` | 0 金 | 否 |

`gold_count` 指定时，输出的实际金数与其相同。限定、常驻和免费的持有份数分别保留，常驻折算为半金。例如常驻角色 2 魂配常驻光锥叠影 1，合计四份，折算 2 金。

默认分类由 `accounts/rules.py` 维护：常驻角色为 1003、1004、1101、1104、1107、1209、1211；常驻光锥为 23000、23002、23003、23004、23005、23012、23013。80xx 主角形态和 24xxx 模拟宇宙五星光锥归免费，其余五星归限定。调用 `load_account_reference` 时，可用 `character_categories`、`light_cone_categories` 覆盖具体条目的类别。

统一配装可独立使用，也可与金数条件共同使用：

```python
from dataclasses import replace

# 全 01：所有持有五星为 0 魂、专锥叠影 1。
all_01 = generate_account(
    replace(request, account_id="all-01", gold_count=None, five_star_build=(0, 1)),
    reference=reference,
    rng=random.Random(42),
)

# 全 21：所有持有五星为 2 魂、专锥叠影 1。
all_21 = generate_account(
    replace(request, account_id="all-21", gold_count=None, five_star_build=(2, 1)),
    reference=reference,
    rng=random.Random(42),
)
```

四星角色按自身条件生成。统一配装优先使用加载资料时指定的 `signature_light_cones={角色ID: 光锥ID}`，默认取角色的同命途推荐光锥首项。缺少覆盖和同命途推荐时，加载报错并给出角色 ID。

统一配装固定的投入先得到满足，集中程度再控制其他可分配投入。金数与统一配装冲突时，生成器报告条件无法同时满足。

## 角色与光锥覆盖

`CharacterSpec` 与[单角色请求](character-generation.md)共用原始练度字段，包括等级、突破、星魂、行迹、光锥、遗器请求和单角色遗器档位。账号级覆盖无需填写单角色 `config_id`，实例 ID 由账号统一分配。

```python
from research.account_generation import CharacterSpec, LightConeSpec

custom = replace(
    request,
    character_overrides={
        "1310": CharacterSpec(
            eidolons=2,
            light_cone=LightConeSpec(light_cone_id="23025", superimposition=1),
            relic_quality_tier=5,
        ),
    },
)
```

覆盖字段优先于统一五星配装，角色的星魂和光锥叠影采用覆盖值。覆盖中光锥 ID 留空时，仍在对应候选范围内自动选择。显式传入光锥 ID 时直接采用，支持不同命途或候选池以外的配装；该 ID 需要存在于光锥资料中，供预算统计查询。

自动配装始终匹配命途，在接近投入目标的组合中优先推荐光锥，必要时采用候选池内其他同命途光锥。活动及模拟宇宙光锥每种至多分配一个实例；普通光锥允许同 ID 多个实例，见[光锥持有约定](design.md#光锥持有与装备实例)。

唯一光锥冲突时，搜索继续尝试其他配装和角色名单。名单搜索对每种持有集合与核心集合的组合考察一次。未指定金数时，随机排列可达金额，逐个尝试满足唯一光锥约束的分配。全部搜索共用 `max_attempts` 预算。

## 推荐队友与资源集中程度

推荐资料按原队伍和位置保留。选中角色后，从其固定推荐队伍中抽取一组，后续选择优先考虑其中尚未满足的位置：首选队友权重为 1，对应替补为 0.5。明确列出的角色 ID 参与抽取，替补分组 ID 保留在参考资料中供后续关联。

`teammate_bias=0` 按可行候选等概率选择，`1` 优先从这些队友中选取；中间值混合两种分布。所有选择同时满足人数、候选范围和金数可达条件。

集中程度为 `c`，持有人数为 `n`，核心人数为 `m`。满足各角色下限后，剩余投入按以下权重分配：

- 普通角色：`(1-c)/n`。
- 核心角色：`(1-c)/n + c/m`。

`c=0` 平均分配可调投入，`c=1` 优先投入核心。核心达到容量后，剩余投入分配给其他角色。固定配装、角色本体所需金数和遗器上限都会影响实际份额，结果提供 `core_gold_share` 与 `core_relic_share` 供比较。

金数分配在接近各角色目标投入的组合中，进一步接近 `light_cone_gold_share`；该参数表达光锥投入倾向。

## 遗器目标与返回结果

平均有效词条以全部持有角色已装备的 `6 × 人数` 件遗器为范围，各件按对应角色权重评分。上下限可独立提供；两端相同时搜索精确目标。五档条件按所有角色理论上限之和换算，再与显式平均范围和角色级条件取交集。

先在可行总分范围内选择目标，再按集中程度分配到各角色。搜索优先检查各角色投入目标半条以内的成品。该范围内无解时，使用剩余预算搜索角色配装和账号平均目标允许的完整范围。精确总分在每轮中先搜索全中档组合，再搜索全部允许的分档。三／四词条按 5∶2 选择优先候选，目标筛选可以提高四词条占比。

| 返回字段 | 含义 |
| --- | --- |
| `characters` | 实际持有角色及其原始练度、装备实例引用 |
| `light_cones`、`relics` | 全账号统一装备库存，每份数值保存一次 |
| `core_character_ids` | 实际采用的核心名单 |
| `investments` | 各角色的持有份数、六件得分与理论上限 |
| `copies_by_category` | 各类别五星持有份数，通过属性读取 |
| `gold_by_category`、`gold_count` | 当前统计口径的分类金数与实际总金数，通过属性读取 |
| `mean_effective_rolls` | 已装备遗器的每件平均得分，通过属性读取 |
| `core_gold_share`、`core_relic_share` | 核心的实际金数与遗器得分占比，通过属性读取 |
| `game_version`、`data_snapshot` | 游戏资料版本与快照 |
| `schema_version`、`generator_version` | 输出字段及账号生成规则版本 |
| `attempts` | 实际消耗的候选考察次数 |

`dataclasses.asdict(result)` 提供角色、库存和生成统计的普通字典，属性统计可单独读取保存。角色引用的装备实例 ID 均能在对应库存中找到，每个实例分配给一个角色。角色不存在时保持缺席，空账号的金数和平均词条均为零。

## 验证

执行 `uv run --locked python -m unittest discover -s tests -v`。测试覆盖常驻半金、全 01／全 21、组合条件冲突、显式覆盖、唯一与多实例光锥、推荐队友、投入集中、精确与单边词条范围、固定种子、空账号及搜索预算。

已验证 60 人全 01、60 人指定金数、12 人五档练度，以及小规模精确平均词条目标。预算耗尽表示本次尚未找到结果。生成检查核对资源条件，对局能力通过后续模拟评价。
