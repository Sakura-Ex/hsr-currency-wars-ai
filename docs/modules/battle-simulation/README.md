# 基础战斗模拟

`crates/battle-core`提供独立于窗口的Rust库和`hcwa-battle`命令行入口。
输入已经合成的有效面板、动作和规则参数，返回战斗结果、事件记录或多次模拟统计。
Tauri可直接调用同一Rust库；账号转换与整局环境后续从各自模块接入。

## 当前能力

| 能力 | 已实现范围 |
| --- | --- |
| 行动时序 | 按速度推进AV，同刻稳定排序，行动提前/延后，多波共享预算 |
| 动作与资源 | 显式动作优先级、自动终结技、双方独立技能点、能量消耗与恢复 |
| 直接效果 | 伤害乘区、逐命中暴击、嘲讽加权目标、治疗、护盾与持续时间 |
| 终止与复活 | 死亡、可选显式复活策略、独立罚时账本、胜败与行动保护、取消 |
| 批次评估 | 独立种子、逐局记录、胜败频率、95% Wilson区间、按种子重放 |

当前支持的规则契约为`basic_v1`，详细执行顺序见
[核心说明](../../../crates/battle-core/README.md#architecture-and-execution-contract)。
它不会从角色名称自动补技能，也不推断后台机制、追击、召唤物、持续伤害、
韧性击破状态变化或装备羁绊。已知未支持的机制放入`unsupported_mechanics`，
入口会拒绝。完整账号配置到战斗输入的转换仍待实现。

整局HP结算由整局环境维护，本库的`run_hp_loss`为未知，不填0，也不将角色剩余
HP当作玩家整局HP。预算耗尽按当前显式规则判败；程序行动保护和取消为
`incomplete`，不会混入胜败统计分母。

## 安装与运行

安装Rust工具链后，在仓库根目录运行。`rust-toolchain.toml`固定本模块验证使用
的Rust版本，Cargo.lock固定依赖。

```powershell
cargo test --locked -p hcwa-battle-core
cargo run --locked -p hcwa-battle-core --bin hcwa-battle -- validate crates/battle-core/examples/duel.json
cargo run --locked -p hcwa-battle-core --bin hcwa-battle -- simulate crates/battle-core/examples/team-waves.json 17
cargo run --release --locked -p hcwa-battle-core --bin hcwa-battle -- batch crates/battle-core/examples/duel.json 10000 20261114
```

输出为JSON，错误写入stderr并返回退出码2。可将结果重定向到已忽略的`runs/`
目录。示例全部使用人工单位与参数；`duel.json`是理论胜率50%的暴击对决，
`team-waves.json`展示双波队伍，`revival-budget.json`展示声明的复活罚时和预算。
这些结果用于验证基础内核，不能当作真实角色的游戏胜率。

## 输入和结果

输入包含版本和来源描述、双方有效属性、动作、初始资源、波次、AV预算与行动上限。
`model::validate`校验完整输入并生成不可变`ValidatedScenario`；输入指纹随结果返回。
`engine::simulate`执行一场，`engine::simulate_cancellable`增加取消检查，
`batch::run_batch`调用同一引擎执行独立试验。复用接口不读取外部文件，不改玩家真实状态。

批次的`completed = wins + losses + draws`，仅completed进入频率分母；
`incomplete`、已启动后取消与未启动分别记录。没有完成样本时频率和区间为null。
种子在Rust中是u64，在JSON中使用十进制字符串，TypeScript应原样保留，
不能先转为Number再回传。单局重放使用ledger中该局的seed。

## 验证与后续接入

测试覆盖手算公式、状态转换、边界预算、资源合法性、未知与错误输入、取消、
批次分母和完整种子重放；另外用固定上游表达式生成的18个合成向量核对公式迁移。
来源与许可记录见[第三方说明](../../../crates/battle-core/THIRD_PARTY_NOTICES.md)。
CI在Windows和Linux运行格式、静态检查、测试及合成10000次模拟。

后续接入完整角色时，先完成账号/试用及模式参数的有效配置合成，逐项实现所需
机制并添加回归，再用可核验游戏记录对齐动作和结算。真实规则确认与软件测试
分别记录；只支持动作片段时不得标为整个角色已支持。
