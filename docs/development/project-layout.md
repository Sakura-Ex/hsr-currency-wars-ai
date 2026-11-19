# 项目目录与模块职责

按业务职责设置模块目录。`core/` 放研究工具和玩家应用共用的实现，`research/` 放账号生成、Python 绑定、训练和评估，`apps/` 放面向玩家的应用。模块通过公开接口调用，规则和状态由对应模块维护。数据流见[账号数据流](../modules/account-generation/data-flow.md#处理流程)和[系统设计](../project/architecture.md#架构与模块交互)。

## 模块目录

下图约定各模块的位置，表格说明职责与依赖。模块的实现状态、内部结构、接口和验证方式集中在各自的模块文档，通过[模块文档入口](../modules/README.md)查阅。

```text
hsr-currency-wars-ai/
├── core/
│   ├── account/
│   ├── battle/
│   ├── run/
│   └── decision/
├── research/
│   ├── account_generation/
│   ├── sim_native/
│   ├── training/
│   └── evaluation/
└── apps/
    └── player/
```

| 目录 | 职责与依赖 |
| --- | --- |
| `research/account_generation/` | 根据生成条件和随机源产出原始账号配置。模块内部结构见[账号与遗器数据](../modules/account-generation/README.md)。 |
| `core/account/` | 维护共用账号类型和来源导入，将原始账号转换为静态配置和可用资源。接口与验证见[账号模块](../modules/account/README.md)。 |
| `core/battle/` | `battle-core` 接收已组装的战斗输入，结算行动顺序、技能、伤害和状态，返回战斗结果与事件。CLI 调用同一战斗库，供调试、按种子回放和批量验证使用。 |
| `core/run/` | 依据 `core/account/` 提供的配置维护整局状态，提供可见局面和合法操作。校验操作后通过战斗输入组装步骤准备参数，调用 `core/battle/`，并在满足条件时结算结果。 |
| `core/decision/` | 用可见局面和可用资源构造模型输入，通过 `core/run/` 评估候选操作，输出运营或战斗建议。 |
| `research/sim_native/` | 用 PyO3 提供训练和评估共用的 Python 模拟接口，依赖 Rust 模拟核心。 |
| `research/training/`、`research/evaluation/` | 通过 `sim_native` 调用模拟，采集轨迹、训练模型、回放对局和比较策略。 |
| `apps/player/` | 接收玩家账号数据和当前局面，调用核心接口，展示建议并记录玩家反馈。 |

候选操作在整局状态副本上试算。实际对局达到正式结算条件时，由 `core/run/` 将结果写入当前整局状态一次。

## Python 模拟绑定

`research/sim_native/` 维护 `sim-native` crate 的 `Cargo.toml` 和 Python 扩展 `sim_native` 的 `pyproject.toml`。扩展负责转换参数、结果和错误，模拟规则和状态由 Rust 核心维护。

- `sim_native.battle` 暴露战斗接口，通过 `../../core/battle` 路径依赖 `battle-core`。
- `sim_native.run` 暴露整局接口，通过 `../../core/run` 路径依赖整局核心。局面观测、合法操作、状态推进和结果结算由 `core/run/` 提供。

两个 Python 接口共用一个 Maturin wheel。构建、安装和验证步骤见[Python 模拟绑定使用说明](../modules/sim-native/README.md)。

## 构建配置

Rust crate 在各自目录维护 `Cargo.toml`，并加入仓库根 `Cargo.toml` 的 workspace。共用包配置和依赖版本在根清单维护，Rust 依赖统一由根 `Cargo.lock` 锁定。

当前根 workspace 包含已实现的 `core/account` 和 `core/battle`。模块共用根清单的依赖；`account` 使用 edition 2024，`battle` 保持 edition 2021，根 `Cargo.lock` 和 `rust-toolchain.toml` 统一锁定依赖与工具链；从根目录或模块目录均可构建和运行。`core/run`、`research/sim_native` 等规划中模块按各自实现状态接入。战斗核心的使用方法和验证入口见[战斗模拟模块文档](../modules/battle-simulation/README.md)。

Python 研究项目在根目录维护 `pyproject.toml` 和 `uv.lock`。Python 模拟扩展的构建配置由 `research/sim_native/pyproject.toml` 维护。各模块在使用说明中提供安装、运行和验证命令。

## 配套目录

| 位置 | 职责 |
| --- | --- |
| `tests/<模块>/`、`examples/<模块>/` | Python 模块的自动测试和调用示例，目录名与模块对应。 |
| Rust crate 内的 `tests/`、`tests/fixtures/` 和 `examples/` | 对应 crate 的集成测试、测试样例和可运行示例。单元测试放在源码旁。 |
| `docs/project/`、`docs/game-data/`、`docs/development/`、`docs/modules/<模块>/` | 分别存放项目架构与规则、共用游戏数值、开发规范、模块设计与使用说明。 |
| `local-data/`、`runs/`、`checkpoints/` | 本地输入、运行记录和模型权重等产物，跨仓库资料按 [AGENTS.md](../../AGENTS.md) 的分工维护。 |

各模块的示例和测试通过对应模块的公开接口调用实现。
