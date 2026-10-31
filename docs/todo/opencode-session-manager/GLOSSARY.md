# 术语表

| 术语 | 定义 |
|---|---|
| 工程 (project) | opencode 数据模型中的 `project`，以 worktree（目录路径）为唯一标识。本工具的一级分组单位。 |
| 会话 (session) | `session` 表中的一条记录，隶属某个工程，包含聊天内容与 token/cost 统计。删除不可逆。 |
| 级联删除 (cascade delete) | 由外键 `ON DELETE CASCADE` 触发，删除会话时连同清除 `session_message`/`message`/`part`/`todo`/`session_input`/`session_context_epoch`/`session_share` 共 7 张关联表的数据。仅在 `PRAGMA foreign_keys=ON` 时生效。 |
| 已归档会话 (archived) | `time_archived` 非空的会话，界面归入独立「已归档」分组。 |
| 已共享会话 (shared) | `session_share` 表中有对应记录的会话，界面加「已分享」提示，删除会连带失效分享链接。 |
| 活动会话 (active) | opencode 正在运行且 `time_updated` 在最近 5 分钟内的会话，界面标「进行中」警示。 |
| 直删模式 (direct) | opencode 未运行时，以单事务批量 SQL `DELETE` 删除所选会话的执行模式；秒级完成。 |
| 降级执行 (fallback CLI) | opencode 运行中时，删除改走官方 CLI `opencode session delete <id>` 逐条执行的模式；有进度反馈。 |
| 回收空间 (space reclamation) | 手动执行 `VACUUM` 压缩数据库文件的操作，默认不自动执行。 |
| 主会话 (main session) | `parent_id` 为空的会话，即未被任务派生的顶层会话；界面默认只显示主会话。 |
| 子会话 (child session) | `parent_id` 指向某主会话的会话，由 `task` 工具派生的 subagent；不入默认列表，仅在「显示子会话」开关开启时可见，可单独勾选删除。 |
| 子树删除 (subtree deletion) | 删除主会话时将其全部子会话并入删除集、整棵删除的行为。因 `parent_id` 无外键、不会自动级联，须由本工具显式枚举子会话；删除顺序子先父后，避免产生孤儿。 |
| 孤儿会话 (orphan session) | `parent_id` 指向的父会话已被删除而自身存留的会话；无外键保护，删除父会话后子会话默认残留。工具以子树删除规避继承产生。 |
