# Git 协作规范

每个 PR（拉取请求）包含一项完整改动。随着代码量和参与人数增加，调整协作流程。

## 任务与评审流程

每个任务写明目标、输入输出、验收条件和负责人。划分任务时，确保每项任务都能独立演示和评审。

建议按以下流程协作：

1. 领取任务，确认负责人和验收条件。
2. 创建短期分支，实现改动并完成相关验证。耗时较长的任务可提前建立草稿 PR，让同伴了解接口和进度。
3. 提交 PR，说明结果、主要改动和验证证据。
4. 请同伴评审；涉及共享接口时，请接口使用方参与。
5. 处理评审意见，有重要变化时再次请求评审。建议至少获得一位同伴批准，并通过相关检查后合并。

## PR 内容与合并要求

PR 正文使用以下结构：

```markdown
## 目标与结果
说明具体场景和实现后的行为，关联任务。

## 主要改动
说明评审需要关注的模块、规则或接口变化。

## 验证
列出实际执行的命令、使用的样本和结果，说明验证覆盖的范围。
```

评审时检查需求是否满足、规则是否正确、代码是否易读、验证是否充分，以及文档是否与改动一致。

建议通过 PR 向主分支合并改动，使用 squash merge 将一个 PR 中的提交合并为一次提交。在 GitHub 中配置相应的评审和检查要求，必需检查的名称应与仓库中实际运行的检查任务一致。

## 分支命名

手动创建的分支建议命名为 `feat/<主题>`、`fix/<主题>`、`docs/<主题>`。Codex 创建的分支使用 `codex/<主题>`。

## 提交信息

提交信息使用英文，标题采用以下格式：

```text
type(scope): description

feat(sim): add crystal settlement for reward nodes
fix(battle): apply freeze removal in the observed order
docs(plan): clarify acceptance criteria for initial battle samples
test(replay): add a case for reproduced positioning differences
refactor(state): consolidate resource update logic
ci(checks): add battle module tests
```

type 表示改动类型，使用 `feat`、`fix`、`docs`、`test`、`refactor`、`perf`、`build`、`ci`、`chore`；scope 表示涉及的模块，使用实际模块名称。正文按需补充改动原因、行为变化和关联任务。

PR 标题同样使用英文和上述格式，合并后用作主分支上的提交标题。每个提交包含一项完整改动，便于理解、定位和撤销。
