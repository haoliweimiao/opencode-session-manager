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
