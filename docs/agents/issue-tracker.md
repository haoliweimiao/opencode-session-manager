# 问题跟踪器：本地 Markdown

本仓库的 issue 与规格以 markdown 文件存放在 `.scratch/` 下。

## 约定

- 一个特性一个目录：`.scratch/<feature-slug>/`
- 规格为 `.scratch/<feature-slug>/spec.md`
- 实现 issue 每个 ticket 一个文件，位于 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 开始编号，绝不用合并的单一大文件
- triage 状态写在每个 issue 文件靠顶部的 `Status:` 行（角色字符串见 `triage-labels.md`）
- 评论与对话历史追加到文件末尾的 `## Comments` 标题下

## 当技能要求「发布到 issue 跟踪器」

在 `.scratch/<feature-slug>/` 下新建文件（必要时先创建目录）。

## 当技能要求「获取相关 ticket」

读取所引用路径下的文件。用户通常会直接给出路径或 issue 编号。

## 路径规划操作（wayfinding）

由 `/wayfinder` 使用。**map** 是一个文件，每个 ticket 一个**子文件**。

- **map**：`.scratch/<effort>/map.md`（Notes / Decisions-so-far / Fog 主体）。
- **子 ticket**：`.scratch/<effort>/issues/NN-<slug>.md`，从 `01` 开始编号，正文为该问题。`Type:` 行记录 ticket 类型（`research`/`prototype`/`grilling`/`task`）；`Status:` 行记录 `claimed`/`resolved`。
- **阻塞**：靠顶部的 `Blocked by: NN, NN` 行。当它列出的每个文件均为 `resolved` 时，该 ticket 解除阻塞。
- **前沿**：扫描 `.scratch/<effort>/issues/` 中处于打开、未阻塞、未认领状态的文件；编号小者优先。
- **认领**：动工前将 `Status` 设为 `claimed` 并保存。
- **解决**：在 `## Answer` 标题下追加答案，将 `Status` 设为 `resolved`，然后把上下文指针（要点 + 链接）追加到 `map.md` 的 Decisions-so-far。
