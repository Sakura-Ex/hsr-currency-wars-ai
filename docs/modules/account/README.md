# Rust 账号数据与配置

本模块提供 `account-core` crate，供训练、评估和玩家应用共用。包括原始账号模型、reliquary-archiver v4 导入、账号静态配置转换和静态面板计算。

[reliquary-archiver 导入](reliquary-import.md)将来源文件转换为统一原始账号；[账号静态配置](playable-configuration.md)说明转换入口、保底规则和面板结构。整体流程见[账号数据流](../account-generation/data-flow.md)。

## 目录与入口

```text
core/account/
├── Cargo.toml
├── data/
│   ├── reliquary-traces-v4.json  # 已固定版本的行迹绑定
│   └── s2-v4.7/                # 实际游玩配置所需的文档数值与模板
├── src/
│   ├── lib.rs          # 公开类型
│   ├── raw/            # 原始账号与实际持有资源
│   │   ├── mod.rs      # 账号标识、版本、完整库存及公开类型
│   │   ├── character.rs  # 角色、行迹与装备引用
│   │   ├── equipment.rs  # 光锥、遗器实例和部位
│   │   └── stats.rs    # 遗器主副词条键
│   ├── playable/       # 实际配置与静态计算
│   │   ├── mod.rs      # 生效配置、面板及共用规则常量
│   │   ├── selection.rs  # 内部资源检查与本体索引
│   │   ├── attributes.rs  # 静态属性键与单位
│   │   ├── traces.rs   # 按试用基础行迹补齐
│   │   ├── conversion.rs  # 完整配置转换入口
│   │   └── panel.rs    # 共用静态属性计算
│   ├── reference/      # 文档数据类型及 JSON 载入
│   └── import/reliquary/  # 来源字段、行迹与装备转换
├── examples/           # 统一格式读取与实际文件导入
└── tests/
    ├── raw_account.rs
    ├── reliquary_import.rs
    ├── playable_data.rs
    ├── playable_conversion.rs
    ├── trace_floor.rs
    └── fixtures/       # 人工构造的最小样例
```

所有 Rust crate 在根 `Cargo.toml` 中组织为 workspace，当前成员包含 `core/account` 与 `core/battle`。后续训练绑定放在 `research/sim_native/`，通过路径依赖使用账号核心。

原始账号类型继续通过 `account_core::RawAccount` 等根入口使用。实际配置类型位于 `account_core::playable`，转换入口位于 `account_core::playable::conversion`，计算资料位于 `account_core::reference`。

## 统一配置

| 类型 | 保存内容 |
| --- | --- |
| `RawAccount` | 内部格式版本、配置 ID、可选游戏版本与快照、角色列表、光锥和遗器完整库存 |
| `CharacterConfig` | 常规角色 ID、原始等级、突破、星魂、可选能力版本、行迹和装备引用 |
| `TraceConfig` | 真实行迹节点 ID 对应的基础等级，以及一次性节点的解锁状态 |
| `LightConeInstance` | 实例 ID、光锥种类 ID、原始等级、突破和叠影 |
| `RelicInstance` | 实例 ID、套装、部位、实际品质、强化等级、主词条与当前副词条 |
| `RelicSlot` | 头、手、躯干、脚、球、绳，序列化为数字 1～6 |
| `RelicMainStat`、`RelicSubStat` | 与现有 Python 生成器一致的属性键 |

`schema_version` 当前为 `RAW_ACCOUNT_SCHEMA_VERSION = 1`，表示项目统一格式。来源的导出格式版本由后续适配器单独记录。`config_id` 由调用方分配；游戏版本和快照未知时保存 `None`，序列化为 `null`。

角色和库存使用列表，保留来源顺序和重复条目，供后续接入接口报告重复 ID。每件装备使用实例 ID，光锥种类 ID 可以重复。装备归属集中在角色上维护：光锥引用为 `None` 表示空装，遗器按部位映射，缺少的部位表示未装备；库存中未被引用的装备保持闲置。

角色形态按常规角色 ID 分别保存。多形态之间的共享资源关系和使用限制由局面模拟检查。

## 行迹与单位

`TraceConfig.skill_levels` 使用真实行迹节点 ID，记录星魂加级前的基础等级。普攻、战技、终结技、天赋、秘技、忆灵技能、欢愉技等按资料关联到相应节点。`node_unlocks` 使用节点 ID 到布尔值的映射，明确区分已解锁、未解锁与信息未知：

- `true` 表示已解锁，`false` 表示未解锁。
- 缺少某个节点键表示该节点的信息未知。
- 所需节点是否齐全，由接入接口结合资料快照检查。

`ability_version` 保留来源提供的能力配置版本标识；`Some(0)` 与未知的 `None` 分别表达。具体机制由对应版本资料解释。

百分比副词条使用比例值，例如 `0.0324` 表示 3.24%；固定生命、攻击、防御和速度使用实际值，保留小数精度。主词条保存已经区分固定值与百分比的键，最终数值在配置转换中按品质、强化等级和快照查询。属性键约定见[遗器数据](../../game-data/relic-data.md#字段与单位)。

reliquary-archiver v4 适配器已完成主词条歧义处理、百分比换算和行迹绑定。来源技能等级直接取账号技能树，保留基础等级。当前副词条由 `substats` 表达；锁定、弃置、预览与重掷等字段保留在原始导出文件中。字段依据与支持范围见[导入说明](reliquary-import.md)。

## 接入与转换

统一数据类型表达字段与单位。Serde 检查统一格式的字段类型、已知枚举和必需字段；统一结构体拒绝未知字段。来源适配器忽略非计算字段，并报告尚待适配的扩展字段路径。

reliquary-archiver 导入入口已检查重复 ID、装备引用及部位冲突、等级与突破范围、当前词条和行迹绑定。缺失的行迹信息保持未知并写入报告。直接构造或反序列化 `RawAccount` 时仍需由对应入口检查；转换入口核对游戏版本和资源引用，局面模拟检查实际阵容的玩法约束。

导入报告在原始账号之外保存错误、缺失字段和未知编码。生成器在自己的结果中保存生成参数、随机种子、投入评分与搜索统计。`convert_playable_account` 接收 `RawAccount`，完成试用补齐、练度保底、星魂技能加级和静态面板计算。

## 运行与验证

安装 Rust 工具链后，在仓库根目录运行：

```powershell
cargo run --locked -p account-core --example raw_account
cargo run --locked -p account-core --example convert_playable -- <导出.json> <游玩配置.json>
cargo test --locked -p account-core
cargo fmt --all -- --check
cargo clippy --locked -p account-core --all-targets -- -D warnings
```

测试样例为人工构造数据，覆盖空装、部分配装、闲置装备、同种光锥多个实例、多形态、不同遗器品质和强化等级，以及未知行迹信息。reliquary-archiver 实际文件导入结果见[导入验证](reliquary-import.md#验证结果)。
