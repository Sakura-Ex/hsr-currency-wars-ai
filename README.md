# HSR Currency Wars AI

English | [简体中文](README.zh-CN.md)

An open-source AI strategy research and assistance project for Currency Wars in Honkai: Star Rail, aimed at Windows players.

## Project goals

- Explore and evaluate team compositions, combat decisions, and economic strategies.
- Adapt strategy recommendations to each player's available characters and equipment.

## Status

| Module | Status |
| --- | --- |
| [Account generator](docs/modules/account-generation/account-generation.md) | Implemented. Uses bundled game data for version 4.7 to generate sample accounts with owned characters, Light Cones, and Relics offline. |
| Account adapter and configuration conversion | In design. Covers importing player accounts, applying trial and fallback configurations, and resolving effective stats. |
| Run environment | Planned. Simulates stages, shops, investments, and resource settlement. |
| Operations decision engine | In design. The current priority is defining observations, legal actions, and evaluation for one stage. Model training is planned. |
| [Combat engine](docs/modules/battle-simulation/README.md) | Implemented as the Rust `basic_v1` explicit-input core and batch CLI. Executes resolved stats/actions with deterministic replay; full 4.7 character kits, automatic account assembly, and run-HP settlement are not supported. |
| Combat planner | Planned. Compares positioning, combat actions, and retries. |
| Team composition research | Planned. Searches team compositions and transitions. |
| Experiment evaluation | Planned. Replays runs and compares strategy results. |
| Windows player application | Planned. Presents recommendations and records player feedback. |

See the [documentation hub](docs/README.md) for project plans, design, and development guides (in Chinese).

## References and acknowledgements

| Repository | Use |
| --- | --- |
| [StarRailRes](https://github.com/Mar-7th/StarRailRes) | Game reference data |
| [TurnBasedGameData](https://github.com/DimbreathBot/TurnBasedGameData) | Game reference data |

## License

This project is licensed under the GNU General Public License version 3.0 only (`GPL-3.0-only`). See [LICENSE](LICENSE) for the full license text.

This is an independent community project and is not affiliated with or endorsed by HoYoverse. Game names and trademarks belong to their respective owners.
