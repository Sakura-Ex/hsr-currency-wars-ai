# Python 模拟绑定使用说明

`sim_native` 供训练和评估调用 Rust 模拟，包含战斗接口 `sim_native.battle` 和整局接口 `sim_native.run`。模块位置和依赖见[项目目录与模块职责](../../development/project-layout.md#python-模拟绑定)。

当前处于规划阶段。以下命令待相应绑定实现后执行，构建、导入和打包结果待验证。

Rust战斗核心已提供[分步接口](../battle-simulation/stepping.md)：推进到决策点，返回观测与合法操作，
再提交外部选择。后续Python战斗绑定复用该接口，让推理与训练使用相同的状态推进。

## 本地安装

安装 Maturin 和 Rust 工具链，激活用于训练或评估的 Python 虚拟环境。在仓库的 `research/sim_native/` 下运行：

```powershell
maturin develop --locked
```

Maturin 读取当前目录的 `pyproject.toml` 和 `Cargo.toml`，通过根 Cargo workspace 的锁文件构建绑定及其 Rust 依赖，并将扩展安装到当前虚拟环境。

## 构建 wheel

在同一 `research/sim_native/` 目录运行：

```powershell
maturin build --locked --release
```

## 接入后的验证

1. 本地安装后执行 `python -c "from sim_native import battle"`，确认可以导入战斗接口。局面模拟接入后，再执行 `python -c "from sim_native import run"`。
2. 用 Rust CLI 和 Python 战斗接口运行同一份战斗样例，按共用的输入输出约定核对字段、单位和结果。
3. 将发布用 wheel 安装到干净的 Python 虚拟环境，重复对应的导入检查。
4. 在仓库根目录的 Hatch wheel 配置中排除 `research/sim_native/`，检查账号生成包的 wheel 仍包含 `research/account_generation/`，且排除规则生效。
