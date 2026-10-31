# opencode-session-manager 实现计划

> 过程文档（P 层），未入库。设计已与用户逐轮确认（grilling 三轮），下文即验收基线。

## 一、背景与目标

用 Rust + iced 做一个桌面 GUI 工具，展示本机 opencode 的工程与会话清单，支持单选/跨工程全选，删除所选会话。

当前机器数据规模：13 个工程、325 个会话、0 个已归档会话。

## 二、事实依据（已取证）

- 数据全部在 `~/.local/share/opencode/opencode.db`（SQLite，WAL 模式，当前约 10GB）。
- `project`：`id` / `worktree`（目录路径，实际工程标识）/ `name`（当前多为 NULL）。
- `session`：`id` / `project_id`(FK)→`project` 级联 / `title` / `model` / `agent` / `cost` / `tokens_*` / `time_created` / `time_updated` / `time_archived`。
- `session_id` 上的级联表共 7 张：`session_message` / `message` / `part` / `todo` / `session_input` / `session_context_epoch` / `session_share`。
- `session.parent_id`：子会话（`task` 派生的 subagent）指向其主会话。**该列无外键、不随父删除自动级联**；实测 2026-10 数据 322 会话中 199 为子会话、深度≤1、子与父同工程同目录。
- 官方 CLI：`opencode session list`（仅 title/updated，无工程分组）、`opencode session delete <id>`（单条）、`opencode db [query]`、`opencode db path`（输出 DB 路径）。
- 关键注意：SQLite 默认 `PRAGMA foreign_keys=OFF`，直删连接**必须显式开启**，否则级联不生效、留下孤儿行。

## 三、已确认设计决策

| 项 | 决策 |
|---|---|
| GUI 框架 | iced（跨平台，最新稳定版，构建时定版） |
| 数据读取 | rusqlite 只读连接（`mode=ro`）直读 DB；DB 路径先 `opencode db path`，失败则 XDG 全局数据目录兜底 |
| 删除执行 | 混合策略：opencode 未运行 → 单事务直删；运行中 → 逐条 `opencode session delete <id>`（降级执行） |
| 运行中检测 | `pgrep -f opencode`，运行时置直删不可用并显示全局横幅 |
| 界面 | 两级视图：左工程列表(含会话数) / 右当前工程会话(复选框)；顶栏跨工程全选/反选 |
| 会话列 | 勾选 \| title \| 更新时间 \| model \| cost |
| 筛选 | title 关键字过滤；时间范围过滤二期 |
| 特殊会话 | 已归档独立分组、已共享加⚠️标记、活动会话(运行中且近5分钟更新)标「进行中」；全部显示默认不隐藏 |
| 确认弹窗 | 摘要(N会话/M工程/不可逆) + 滚动勾选清单 + 警示点；可改勾选 |
| 备份 | 可选「导出备份到目录」开关，默认关；勾选则逐条 `opencode export <id>` 后再删 |
| 空间回收 | 默认不回收；删除后出现「回收空间」手动按钮 → 触发 VACUUM，执行前做磁盘空间预检 |
| 刷新 | 启动加载 + 每次删除后局部刷新 + 手动刷新按钮；无轮询 |
| 工程级删除 | 不做一键删工程；用工程复选框全选统一走流程 |
| 进度(降级CLI) | 进度条「已删 x/n」+ 失败项红色单列 + 重试失败项 |
| 主/子会话 | 默认仅显示主会话（`parent_id` 为空），主会话行标「子×N」；「显示子会话」开关默认关，开启后子会话可见且可单独勾选删除 |
| 子树删除 | 勾选主会话即并入其整棵子树（子默认全选，确认弹窗内可改）；删除顺序子先父后；确认弹窗对「父勾选但子被取消」显示孤儿警示 |

## 四、技术栈与依赖（最小集）

- Rust（稳定版 toolchain），edition 2021+。
- 依赖候选：`iced`、`rusqlite`(feature `bundled`)、`directories`(XDG 兜底)、`anyhow`。pgrep/`opencode` 走 `std::process::Command`，不引额外库。
- 不引 serde（直接用 rusqlite 行映射即可）。构建时按 iced 当前最新稳定版定 pin。

## 五、模块划分（iced 应用）

```
src/
├── main.rs          # iced 应用入口 / 消息循环
├── db.rs            # 数据层：只读查询 + 直删事务（打开即 foreign_keys=ON、busy_timeout）
├── runner.rs        # 删除执行器：判定 opencode 运行态，分发直删/降级 CLI，产出进度与失败项
├── app.rs           # GUI 状态与 Update：两级视图、勾选集、筛选、弹窗、进度
└── ui/
    ├── sidebar.rs   # 工程列表（含会话数、全选本工程）
    ├── session_list.rs # 会话行、列渲染、特殊标记
    └── dialogs.rs   # 确认弹窗、进度面板、失败重试
```

### 数据层查询

- 工程+计数：`SELECT p.id, p.worktree FROM project p` + `SELECT project_id, COUNT(*) FROM session GROUP BY project_id`。
- 会话列表：`SELECT id, title, time_created, time_updated, cost, time_archived, model FROM session WHERE project_id = ? ORDER BY time_updated DESC`。
- 共享标记：`SELECT session_id FROM session_share`（一次性载入集合）。
- 已归档分组：`time_archived IS NOT NULL` 的会话归入「已归档」组。
- 直删事务：`PRAGMA foreign_keys=ON; DELETE FROM session WHERE id IN (...);` 单事务提交。

### 删除执行器

- `pgrep -f opencode` 有输出 → 降级 CLI：逐条 `opencode session delete <id>`，`Command::status()` 判定成败；失败收集。
- 无输出 → 直删：上一节事务。两模式共用确认弹窗与勾选集。

## 六、测试计划

- 数据层单元测试：查询 SQL 针对空库/样例库跑通（用内存 SQLite 建最小 schema 副本）。
- 直删级联验证：样例库插入 project/session/message，直删后断言 7 张关联表无孤儿行（**验证 foreign_keys=ON 生效**）。
- 运行态判定：mock pgrep 返回值两种分支。
- 手动冒烟：对真实 DB 仅跑只读查询不删；删除操作先在本机选 1 个测试会话人工核对后放开。

## 七、验收标准

1. 启动即正确列出全部 13 工程与其会话数，无遗漏、无报错。
2. 两级视图、全选/单选勾选、关键字段展示、特殊标记符合第三节表。
3. 未运行态直删所选会话秒级完成；运行态横幅提示并逐个 CLI 删除且有进度。
4. 确认弹窗含摘要、滚动清单、警示点，可就地改勾选。
5. 删除后列表局部刷新、计数更新。
6. 不自动 VACUUM；「回收空间」按钮触发并有磁盘预检。
7. `cargo build` 无警告（warning-free），`cargo test` 全绿。
8. 勾选主会话自动并入其全部子会话；确认弹窗内可单独取消子会话并显示孤儿警示。
9. 默认列表不含子会话；「显示子会话」开关开启后子会话可见且可单独删除（不动主会话）。
10. 删除顺序子先父后：任意模式中途退出均不产生孤儿（`parent_id` 悬挂）。

## 八、风险与备注

- 删除不可逆：仅靠可选备份兜底；第一批只小批量(≤5)验证。
- 降级 CLI 325 个会话约 2~3 分钟：进度条弱化等待感。
- VACUUM 需约 2× 磁盘空间：按钮前预检 `Statvfs` 剩余空间并提示。
- iced 具体控件能力（列表虚拟滚动等）以构建时最新版实测为准，不足则用简单 `Scrollable` 兜底。

## Task 1: 项目骨架

**Context**: 空仓库，`git init -b main` 已完成。建可测试的 cargo 库工程（lib + bin），引入依赖。

**Steps**:
1. `cargo init --name opencode-session-manager` — **Expected**: 生成 `Cargo.toml`、`src/main.rs`、`.gitignore`。
2. 改写 `src/main.rs` 为薄壳，新增 `src/lib.rs` 声明模块 `db`、`runner`、`app`、`ui`（各模块为占位 pub 定义，先不实现逻辑）。— **Expected**: `cargo build` 编译通过无错误。
3. `Cargo.toml` 依赖：`iced`、`rusqlite`（feature `bundled`）、`directories`、`anyhow`（版本取 crates.io 当前稳定，构建时定 pin）。— **Expected**: `cargo build` 下载并编译通过。
4. 提交。— **Expected**: git 有首个提交。

**Completion contract**: `cargo build` 无报错；模块骨架成立。

**Test command**: `cargo build`

## Task 2: 数据层 db.rs

**Context**: 用户已确认直读 DB、hybrid 删除。数据库在 `~/.local/share/opencode/opencode.db`（WAL）。本节 TDD 于内存/临时 SQLite，用最小 schema 副本。

**Produces**:
- `pub struct ProjectCount { id: String, worktree: String, sessions: usize }`
- `pub struct SessionRow { id: String, title: String, time_created: i64, time_updated: i64, cost: f64, archived: bool, model: String }`
- `pub struct SharedIds { shared: HashSet<String> }`
- `pub fn open_readonly(path: &Path) -> Result<Connection>`
- `pub fn load_project_counts(conn: &Connection) -> Result<Vec<ProjectCount>>`
- `pub fn load_sessions(conn: &Connection, project_id: &str) -> Result<Vec<SessionRow>>`（按 `time_updated DESC`）
- `pub fn load_shared_ids(conn: &Connection) -> Result<HashSet<String>>`
- `pub fn delete_sessions(conn: &mut Connection, ids: &[String]) -> Result<usize>`（单事务，`PRAGMA foreign_keys=ON` 于连接打开时即设）

**Tests**（`tests/db.rs`，用临时文件/内存建最小 schema：project、session、message、part、session_share）:
1. `load_project_counts_aggregates` — 插 2 工程、3 会话，断言计数正确。
2. `load_sessions_ordered_desc` — 断言按 time_updated 降序。
3. `load_shared_ids_set` — 断言共享会话 id 集合。
4. `delete_sessions_cascades_children` — 会话带 message+part，删除后断言子表无孤儿（**关键：验证 foreign_keys=ON 生效**）。
5. `open_readonly_rejects_writes` — 对临时文件开 `mode=ro`，写操作应为 `DatabaseBusy`/只读错误。

**Steps（RED→GREEN）**: 逐测试先写后跑，见 fail，再实现；全部过一遍后 `cargo test` 全绿。

**Test command**: `cargo test --test db`

## Task 3: 执行器 runner.rs

**Context**: 删除按运行态分流。pgrep 检测 + 直删/降级 CLI。

**Produces**:
- `pub trait SessionDeleter { fn delete(&self, id: &str) -> Result<(), String>; }`（可注入，便于测试；实现 `CliDeleter` 调 `opencode session delete <id>`）
- `pub struct DbDeleter<'a> { conn: &'a mut Connection }`（实现 SessionDeleter，调 db::delete_sessions 单条）
- `pub fn opencode_running(detect: &dyn Fn() -> bool) -> bool`（默认用 pgrep 实现 `pgrep -f opencode`，有输出即运行中）
- `pub struct DeleteProgress { total: usize, done: usize, failures: Vec<(String, String)> }`
- `pub fn run_deletes(deleter: &dyn SessionDeleter, ids: &[String], on_progress: impl FnMut(&DeleteProgress)) -> DeleteProgress`（逐条调用、收集失败、回调进度）

**Tests**（`tests/runner.rs`）:
1. `opencode_running_detects` — 注入返回 true/false 的检测器，断言状态。
2. `run_deletes_collects_failures` — 注入 deleter 对部分 id 失败，断言失败集合、done 计数、进度回调次数。
3. `run_deletes_all_success_clears_failures` — 全成功断言无失败。

**Steps（RED→GREEN）**: 同上逐测试。

**Test command**: `cargo test --test runner`

## Task 4: GUI 应用 app.rs + ui/

**Context**: iced 双栏两级视图。为可测试，把纯状态逻辑抽到 `app::state`（不依赖 iced）。

**Produces**:
- `pub struct AppState { projects, sessions_by_project, shared, selected: HashSet<String>, filter: String, current_project: Option<String>, mode: RunState }`
- 状态方法：`toggle_session`、`toggle_project_all`、`select_all`、`select_none`、`filtered_sessions(project)`、`confirm_summary() -> (count, projects)`（跨工程聚合）、`state_of(id) -> SessionState {Normal|Shared|Active|Archived}`
- iced widget 层（`app.rs`/`ui/`）：左侧工程列表 + 计数 + 全选本工程；右侧会话行（勾选|title|updated|model|cost）；顶栏跨工程全选/反选；筛选框；确认弹窗（摘要+滚动清单+警示点，可就地改勾选）；进度面板 + 失败列表 + 重试按钮；运行态横幅。

**Tests**（`tests/app_state.rs`，纯状态）:
1. `toggle_session_updates_selection`
2. `toggle_project_all_selects_and_deselects`
3. `select_all_cross_projects` / `select_none`
4. `filtered_sessions_matches_title`
5. `confirm_summary_aggregates_projects`
6. `state_of_marks_shared_active_archived`

**Steps（RED→GREEN）**: 先状态层 TDD 全绿；widget 层薄封装（无法单测的部分标注手动冒烟）。

**Test command**: `cargo test --test app_state`

## Task 5: 集成验收

**Context**: 全链路联调 + 真实环境只读冒烟。

**Steps**:
1. `cargo build` — **Expected**: 零警告零错误。`RUSTFLAGS="-D warnings"` 复查可选。
2. `cargo test` — **Expected**: 全部绿灯。
3. 真实 DB 只读冒烟（不删任何数据）：临时命令/单元测试设置 `OPCODE_DB_PATH` 指向真实库并仅跑只读加载路径，断言 13 工程、325 会话全载入。— **Expected**: 输出与实际一致。
4. `pgrep -f opencode` 实测本机返回空（当前 opencode 未运行），UI 应显示「未运行」态。— **Expected**: 与运行时行为一致。
5. 提交。

**Test command**: `cargo test`
