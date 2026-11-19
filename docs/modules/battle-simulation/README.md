# 基础战斗模拟 (Battle Simulation)

`core/battle` 维护确定性、显式输入的离线战斗模拟核心库 `battle-core` 以及命令行工具 `hcwa-battle`。

本模块接收已组装的有效面板、动作定义与规则参数，按种子模拟战斗，返回单场结果与单位快照。调用方可通过分步会话读取局面与合法操作。开发工具可额外导出事件回放。开战输入组装和正式节点结算见[项目目录与模块职责](../../development/project-layout.md)。

---

## 目录导航

| 文档 | 核心内容 |
| --- | --- |
| [系统架构与逻辑设计](design.md) | 模块分层架构、核心运行时与诊断分离设计、时序状态机、伤害管线与类型系统 |
| [分步会话与外部决策](stepping.md) | `BattleSession` 接口、推进阶段、合法操作列表与时序边界 |
| [测试与回归验证](validation.md) | 自动化测试、样例参考结果和实机核对 |
| [战斗核心与 CLI 使用手册](usage.md) | `battle-core` 特性、`hcwa-battle` 子命令、开发场景字段与样例 |
| [那刻夏 S2 开发场景](anaxa-scenario.md) | 样例的技能配置、队列和运行方式 |

---

## 核心特性与能力矩阵

| 维度 | 能力说明 |
| --- | --- |
| **运行时与诊断** | 默认构建直接运行内存中的战斗配置。可选的 `diagnostics` 特性提供开发 JSON 解析与回放。 |
| **行动时序调度** | 基于“剩余行动距离 / 当前速度”推进时间；单位常规行动（Turn）结束后重排；速度变化保持当前剩余距离；Cycle 倒计时拥有独立 AV 额度，支持多轮不同额度配置。 |
| **动作与资源机制** | 支持队伍共享战技点、单位能量、常规回合、终结技、额外回合与追加攻击。终结技在实际进入回合时扣除能量，插入动作本身不消耗 AV。 |
| **效果与数值管线** | 支持七元素面板、直接伤害乘区、逐命中暴击、护盾吸收、韧性扣减与击破事件。 |
| **状态与变量作用域** | 状态（Modifier）支持按来源独立叠层、刷新及到期结算；支持弱点附加与自然弱点动态合并；支持单位级持久变量及 Ability / TaskTemplate 临时调用帧栈。 |
| **波次与有限队列** | 支持多波次战斗，或从 `waves[0]` 按顺序补入有限敌人槽位。 |
| **外部决策交互** | `BattleSession` 同步推进到下一个选招点，返回观测与合法操作列表。默认策略与外部策略使用同一列表。 |
| **批次评估与分析** | 支持基于主种子的确定性多局批次模拟；独立分离目标、暴击与元素三条随机流；支持胜率频率统计及 95% Wilson 置信区间评估；任意单局可通过对应种子精确重放。 |

---

## 核心规则契约

1. **两类独立的终局触发**：
   - **完全淘汰**：我方全灭、敌方全部阵亡或双方全灭。敌方清场覆盖所有波次及待入场队列。
   - **轮次倒计时耗尽**：总 AV 预算归零且最终 Cycle 到期。当 AV 额度归零时，已进入的单位回合（Turn）仍会完整收尾，随后执行到期 Cycle 结算；若未达成清场则判为时限耗尽。
2. **安全停止与未完成状态**：
   - 支持动作数上限（`max_actions`）与调用方提供的取消检查。
   - 触发保护或取消时，战斗标记为 `Incomplete`，保留当时局面快照，不作为战斗失败计入通关分母。

---

## 快速上手

从仓库根目录执行以下命令（Rust 工具链与依赖由根目录 `rust-toolchain.toml` 及 `Cargo.lock` 统一锁定）：

```powershell
# 解析场景并输出场景指纹
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- inspect core/battle/examples/duel.json

# 单局模拟（参数：场景文件 随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- simulate core/battle/examples/team-waves.json 42

# 批次模拟（参数：场景文件 运行场次 主随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle --release --locked -- batch core/battle/examples/duel.json 10000 20261114

# 分步交互示例
cargo run -p battle-core --features diagnostics --example step_battle --locked -- core/battle/examples/duel.json 42
```

样例列表见[战斗核心与 CLI 使用手册](usage.md#4-示例场景资源)。
