# 那刻夏 S2 开发场景

[`anaxa-s2-full-battle.json`](../../../core/battle/examples/anaxa-s2-full-battle.json) 使用人工面板和部分 4.7 技能资料，演示那刻夏 S2 的部分技能链、有限队列和战斗终止。

---

## 1. 运行与场景生成

### 1.1 使用现有场景直接运行

从仓库根目录执行以下命令：

```powershell
# 1. 解析开发场景并获取场景指纹
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- inspect core/battle/examples/anaxa-s2-full-battle.json

# 2. 单局模拟（使用固定参考种子 42）
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/anaxa-s2-full-battle.json 42

# 3. 蒙特卡洛批次模拟（8 场，主种子 42）
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- batch core/battle/examples/anaxa-s2-full-battle.json 8 42
```

### 1.2 重新构建场景配置

场景可通过 Python 构建器脚本基于参数化配置生成：

```powershell
cd core/battle
python -m tools.scenarios.anaxa examples/anaxa-s2-full-battle.json
```

构建器仅依赖 Python 标准库，生成确定性的 JSON 场景文件。

---

## 2. 场景机制实现规范

本场景配置了角色 `140502`（那刻夏 S2）与 6 名敌人：

| 机制部分 | 场景内的实现规则 | 边界与设计约定 |
| --- | --- | --- |
| **行动策略** | 默认策略选择首个合法操作。常规动作按战技、普攻顺序配置，主目标取生命比例最低的敌人。 | 终结技在合法插入窗口排队，进入回合时扣除能量。 |
| **普攻** | 单体风属性伤害，造成 1.0 倍攻击力伤害，回复 1 点战技点与 30 点能量。 | 样例另设回合开始回能，条件为场上无揭秘目标。 |
| **战技** | 主目标与 4 次随机存活敌人的风属性弹射，每段造成 0.7 倍攻击力伤害，消耗 1 点战技点，合计回复 30 点能量。 | 弹射目标在存活敌人中均匀随机抽取。 |
| **终结技** | 消耗 140 能量切入 Turn，施加揭秘和七属性弱点，再造成 1.6 倍攻击力的全体风伤，动作完成回复 5 点能量。 | 回能量按动作配置。 |
| **揭秘状态** | 目标回合开始时到期。那刻夏对揭秘目标使用普攻或战技前，登记一次免战技点的单段追加动作。 | 追加动作执行时重新选择主目标。 |
| **S2 追击** | 每两次战技（含追加战技）累计一层标记。在这些动作的 `after_attack` 事件中，每满五层标记触发一段追击。 | 每段重新随机选择存活敌人和伤害元素。 |
| **伤害乘区** | 应用通用多乘区公式，显式指定我方伤害系数 `ally_damage_factor = 0.3`。 | 各乘区独立展开计算，优先消耗护盾再扣除生命值。 |

---

## 3. 有限敌人队列

场景在 `waves[0]` 中列出 6 名敌人，`mechanics.queue_capacity` 为 2。开战时前两名入场。敌人阵亡后，在当前 Action 结束后的调度边界依序补位。补入的单位可在后续排队动作中被选中。清场要求场上敌人和后备队列均已清空。

---

## 4. 临时诊断结果

开发工具从 JSON 中取出 `mechanics.settlement`，在战斗结束后用该场景的人工权重、门槛和扣血值生成诊断报告。成功应用时标记为 `applied_provisional`，战斗未完成时标记为 `not_applied_incomplete`。正式节点结算由整局环境负责，见[项目规则](../../project/game-rules.md#战斗与节点结算)。

---

参考输出与验证记录见[战斗模拟验证](validation.md#那刻夏-s2-样例)。
