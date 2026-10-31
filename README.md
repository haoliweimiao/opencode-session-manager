# opencode-session-manager

为 opencode 会话管理提供的 Rust + iced 图形界面小工具：列出本机 opencode 的工程与会话，勾选并删除会话（支持级联删除与备份导出）。

## 功能概览

- 双栏两级视图：左栏工程列表（含会话数），右栏会话列表（含创建时间、模型、状态等）。
- 取消勾选 / 全选 / 反选 / 关键字筛选。
- 删除前确认弹窗：汇总、勾选清单、模式提示，可就地改勾选。
- 会话删除支持**双模式**：
  - **直删模式**（opencode 未运行时）：对所选会话执行单事务 `DELETE`，并级联清理子表（`PRAGMA foreign_keys=ON`）。
  - **降级执行**（opencode 运行中，自动检测）：逐条调用 `opencode session delete <id>`，带进度与失败重试。
- 可选**备份导出**（默认关闭）：删除前将会话与消息 JSON 导出到 `~/.local/share/opencode-session-manager/backups/<时间戳>/`。

## 前置条件

- Linux（或其他有图形环境的桌面系统）；工具是 iced GUI，需要可用的显示环境。
- 已安装 opencode，且其 SQLite 数据库存在（默认路径：`~/.local/share/opencode/opencode.db`）。若 opencode 尚未初始化过，数据库不存在，启动时会被跳过。
- 支持 Rust 工具链（`cargo`）。

## 编译

```bash
cargo build --release
```

产物位于 `target/release/opencode-session-manager`。调试构建用 `cargo build`。

## 运行

```bash
cargo run --release
# 或直接执行编译产物
./target/release/opencode-session-manager
```

启动后窗口会自动加载全部工程与会话。选择的删除操作为**真实数据删除，不可恢复**，务必先确认勾选范围；不放心可先在确认弹窗中开启「导出备份」。

### GPU 环境说明

程序默认以 `WGPU_POWER_PREF=low` 运行（偏集显，省电且兼容性好）。在混合显卡（如 AMD 核显 + NVIDIA 独显）笔记本上，若 wgpu 默认选到 NVIDIA 独显，`Surface::configure` 会因无法呈现报 `Invalid surface` 直接崩溃；设 `low` 可规避。如需改回独显运行，显式设置该环境变量即可覆盖：

```bash
WGPU_POWER_PREF=high ./target/release/opencode-session-manager
```

### 测试

```bash
cargo test          # 单元测试（21 个）
cargo test -- --ignored   # 冒烟测试：真实数据库只读验证（工程/会话数/运行态检测）
```

冒烟测试出于安全默认忽略（`#[ignore]`），只做只读读取，不修改任何数据。

## 数据与风险说明

- 数据来源为 opencode 自身的 `opencode.db`（SQLite，默认约 10GB，WAL 模式）；本工具只读打开进行查询，不写入该库。
- 删除结果直接作用于该库：会同时级联清空会话关联的子表数据，删除后不可恢复。
- 本工具只管理会话（session），不提供工程级删除，也不提供空间回收（VACUUM）按钮；确需回收磁盘空间时请另行处理。
