# 模块文档

先进入模块总览，查看用途、当前状态和阅读入口，再查阅具体设计。

| 模块 | 当前内容 |
| --- | --- |
| [Python 模拟绑定](sim-native/README.md) | 规划中的战斗与局面模拟绑定，登记构建、安装和验证步骤 |
| [账号与遗器数据](account-generation/README.md) | 账号金数与统一配装、推荐关联和资源分配，角色及遗器生成接口与共用数据流 |
| [Rust 账号数据与配置](account/README.md) | 原始账号与导入、实际游玩配置结构、文档数值 JSON、试用补齐与映射设计 |
| [基础战斗模拟](battle-simulation/README.md) | Rust `battle-core` 确定性战斗规则演算、分步外部决策会话（`BattleSession`）、蒙特卡洛批次评估与 CLI 工具手册 |

模块后续增加的设计、接口和验证文档由各自目录内的入口组织。
