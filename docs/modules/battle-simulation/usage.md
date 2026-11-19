# 战斗核心与 CLI 使用手册

`battle-core` 是崩铁货币战争 AI 系统的独立战斗模拟核心，提供纯 Rust 编写的确定性战斗规则演算、蒙特卡洛批次评估以及命令行工具 `hcwa-battle`。

默认构建接收已组装的 Rust `Scenario`，执行单场战斗或批次评估。启用 `diagnostics` 特性后，可读取开发用 JSON、生成输入指纹并导出回放。

系统设计、分步 API 和验证入口见[战斗模拟模块文档](README.md)。

---

## 1. 命令行工具 (`hcwa-battle`)

命令行工具 [`hcwa-battle`](../../../core/battle/src/main.rs) 要求启用 `diagnostics` 特性，用于解析开发场景、重放单局和批次评估。

### 1.1 子命令手册

从仓库根目录执行（统一使用锁定工具链）：

```powershell
# 1. 解析开发场景并输出场景指纹、来源与随机算法
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- inspect <SCENARIO.json>

# 2. 单局确定性模拟（参数：场景文件 随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- simulate <SCENARIO.json> <SEED_U64>

# 3. 蒙特卡洛批次评估（参数：场景文件 试验场次 主随机种子）
cargo run -p battle-core --features diagnostics --bin hcwa-battle -- batch <SCENARIO.json> <TRIALS> <MASTER_SEED_U64>
```

### 1.2 退出码与错误输出

- **退出码 0**：执行成功，标准输出（stdout）打印格式化的 JSON 结果。
- **退出码 2**：文件读取、结构解析、参数、模拟执行或输出出错，错误详情以 JSON 写入标准错误（stderr）。
- 建议将大量批次运行结果重定向至本地忽略目录，如 `runs/result.json`。

---

## 2. 场景输入与输出格式规范

### 2.1 核心输入结构 (`Scenario`)

使用 [`duel.json`](../../../core/battle/examples/duel.json) 作为可运行的输入模板。正式字段由 [`Scenario`](../../../core/battle/src/model/scenario.rs) 定义，包含 `max_av`、`max_actions`、双方共享战技点、我方单位、敌方波次和 `mechanics`。开发文件中的 `schema_version`、`ruleset`（样例标为 `battle_v2`）等元数据由 [`ScenarioFile`](../../../core/battle/src/diagnostics/mod.rs) 在构造 `Scenario` 前移除。组装方负责保证静态数值和规则引用有效，`inspect` 检查 JSON 和字段结构。

### 2.2 核心机制字段 (`mechanics`) 说明

- `cycles`：各轮次（Cycle）的 AV 额度列表。组装方保证额度之和等于 `max_av`，列表为空时使用单轮 `max_av`。
- `ally_damage_factor` 是我方直接伤害的可选乘数。Rust `Mechanics::default()` 与省略此字段的开发 JSON 均取 `1.0`。填入有限非负数，`0.0` 表示该次直接伤害量为零。我方直接伤害命中存活目标时，在护盾和生命结算前乘算一次。敌方伤害使用 `1.0`。用于命中的值为负数或非有限值时返回 `ally_damage_factor must be finite and non-negative`。
- `queue_capacity`：有限敌人队列的在场槽位数。启用时，组装方将所有敌人按入场顺序放入唯一一波 `waves[0]`。敌人阵亡并结束当前 Action 后，引擎依序补足空槽。
- `triggers`: 战局全局或单位级事件监听器（`BattleStart`、`WaveStart`、`Hit`、`Break`、`Death`、`TurnEnd` 等）。

### 2.3 输出结果结构 (`BattleResult` 与 `BattleReport`)

- **运行时原生结果 (`BattleResult`)**：
  纯内存核心直接产出的对局结果，包含：
  - `outcome`：`Victory`（胜利）、`Defeat`（失败）、`Draw`（双方全灭）或 `Incomplete`（未完成）。
  - `termination`：具体终止原因，包括敌方全灭、我方全灭、双方全灭、轮次耗尽、达到动作上限和调用方取消。
  - `elapsed_av` 与 `remaining_av`: 实际消耗 AV 与剩余总预算。
  - `actions`: 本场已完成的完整动作总数。
  - `units`：已入场单位的终态快照，保留阵亡记录及生命、护盾、能量、剩余行动距离、速度和状态等字段。

- **CLI 诊断报告 (`BattleReport`，需 `diagnostics` 特性)**：
  CLI 工具 `hcwa-battle` 导出的 JSON 诊断包装，在 `BattleResult` 的基础上增加审计字段：
  - `provenance` 与 `scenario_sha256`：输入来源及对重新序列化的完整 JSON 计算的 SHA-256 指纹。
  - `seed`: 64 位无符号整型种子的十进制无损字符串。
  - `events` 与 `trace_truncated`：CLI 的 `simulate` 记录有序事件，并标明记录是否达到上限。
  - `settlement`：[那刻夏开发样例](anaxa-scenario.md)使用的临时诊断结果。

---

## 3. Rust 库调用示例

### 3.1 同步快速模拟

默认构建直接运行组装好的场景：

```rust
use battle_core::engine::simulate;
use battle_core::model::Scenario;

fn run_single_battle(scenario: &Scenario, seed: u64) -> Result<(), String> {
    // 纯内存核心直接执行单局模拟
    let result = simulate(scenario, seed)?;

    println!("对局结果: {:?}, 动作数: {}", result.outcome, result.actions);
    println!("消耗 AV: {:.2}", result.elapsed_av);
    Ok(())
}
```

启用 `diagnostics` 特性时，支持从 JSON 文件读取并记录逐事件回放（trace）：

```rust
use battle_core::diagnostics::{simulate as diag_simulate, ScenarioFile};

fn run_diagnostic_battle(scenario_json: &str, seed: u64) -> Result<(), String> {
    let file = ScenarioFile::from_slice(scenario_json.as_bytes())?;
    // trace = true 记录详细事件流
    let result = diag_simulate(&file.battle, seed, true)?;
    let report = file.report(result, seed);

    println!("对局结果: {:?}, 终止原因: {:?}", report.result.outcome, report.result.termination);
    Ok(())
}
```

### 3.2 外部策略分步交互 (`BattleSession`)

对于需要逐步观察战局并执行策略的场景，使用 [`BattleSession`](stepping.md)：

```rust
use battle_core::engine::{BattleSession, BattleStep};
use battle_core::model::Scenario;

fn run_interactive_battle(scenario: &Scenario, seed: u64) -> Result<(), String> {
    // 初始化会话（默认纯内存模式只需传入场景引用与种子）
    let mut session = BattleSession::new(scenario, seed)?;

    loop {
        let (id, choice) = match session.advance()? {
            BattleStep::Decision(decision) => (decision.id, decision.legal.first().cloned()),
            BattleStep::Finished(result) => {
                println!("战斗结束，胜负状态: {:?}", result.outcome);
                break;
            }
        };
        session.submit(id, choice)?;
    }

    Ok(())
}
```

---

## 4. 示例场景资源

场景文件位于 [`core/battle/examples/`](../../../core/battle/examples/)：

- [`duel.json`](../../../core/battle/examples/duel.json)：人工单挑场景，展示基础伤害、暴击和行动顺序。
- [`team-waves.json`](../../../core/battle/examples/team-waves.json)：多角色队伍对战两波敌人，展示波次衔接与战技点共享。
- [`revival-budget.json`](../../../core/battle/examples/revival-budget.json)：展示复活时扣减当前轮次额度。
- [`anaxa-s2-full-battle.json`](../../../core/battle/examples/anaxa-s2-full-battle.json)：那刻夏 S2 的部分技能链与有限队列开发样例，见[样例说明](anaxa-scenario.md)。
