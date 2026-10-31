pub mod state;

use crate::db;
use crate::runner;
use iced::widget::{
    button, checkbox, column, container, progress_bar, row, scrollable, text, text_input,
};
use iced::{Background, Center, Color, Element, Fill, FillPortion, Shrink, Size, Task, Theme};
use state::{AppState, RunState, SessionState};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

type Elem<'a> = Element<'a, Message>;

#[derive(Debug, Clone)]
pub enum Message {
    SelectProject(String),
    ToggleSession(String),
    ToggleProjectAll(String),
    SelectAll,
    SelectNone,
    FilterChanged(String),
    ToggleBackup,
    RequestDelete,
    CancelDelete,
    ConfirmDelete,
    DeleteNext,
    RetryFailed,
    CloseFinished,
    Refresh,
}

struct DeleteRun {
    ids: Vec<String>,
    next: usize,
    total: usize,
    done: usize,
    failures: Vec<(String, String)>,
    backup: bool,
}

impl DeleteRun {
    fn finished(&self) -> Finish {
        Finish {
            requested: self.total,
            deleted: self.done.saturating_sub(self.failures.len()),
            failed: self.failures.clone(),
        }
    }
}

struct Finish {
    requested: usize,
    deleted: usize,
    failed: Vec<(String, String)>,
}

pub struct App {
    state: AppState,
    db_path: PathBuf,
    error: Option<String>,
    dialog: Option<bool>,
    delete: Option<DeleteRun>,
    finished: Option<Finish>,
    backup: bool,
}

pub fn run() -> iced::Result {
    iced::application(boot, update, view)
        .theme(Theme::Dark)
        .window_size(Size::new(1120.0, 760.0))
        .run()
}

fn boot() -> (App, Task<Message>) {
    let (db_path, error) = match resolve_db_path() {
        Ok(p) => (p, None),
        Err(e) => (PathBuf::from("."), Some(e)),
    };
    let mut app = App {
        state: AppState::new(
            Vec::new(),
            HashMap::new(),
            HashSet::new(),
            RunState::NotRunning,
        ),
        db_path,
        error,
        dialog: None,
        delete: None,
        finished: None,
        backup: false,
    };
    app.load();
    (app, Task::none())
}

fn resolve_db_path() -> Result<PathBuf, String> {
    if let Ok(out) = Command::new("opencode").args(["db", "path"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(PathBuf::from(s));
            }
        }
    }
    if let Some(dirs) = directories::ProjectDirs::from("", "", "opencode") {
        return Ok(dirs.data_local_dir().join("opencode.db"));
    }
    Err("无法定位 opencode 数据库路径".into())
}

fn fetch(
    db_path: &Path,
) -> Result<
    (
        Vec<db::ProjectCount>,
        HashMap<String, Vec<db::SessionRow>>,
        HashSet<String>,
        RunState,
    ),
    String,
> {
    let conn = db::open_readonly(db_path).map_err(|e| e.to_string())?;
    let projects = db::load_project_counts(&conn).map_err(|e| e.to_string())?;
    let shared = db::load_shared_ids(&conn).map_err(|e| e.to_string())?;
    let mut sessions = HashMap::new();
    for p in &projects {
        let rows = db::load_sessions(&conn, &p.id).map_err(|e| e.to_string())?;
        sessions.insert(p.id.clone(), rows);
    }
    let running = runner::opencode_running(&runner::pgrep_detect);
    let mode = if running {
        RunState::Running
    } else {
        RunState::NotRunning
    };
    Ok((projects, sessions, shared, mode))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn rel_time(ms: i64) -> String {
    let secs = (now_ms() - ms).max(0) / 1000;
    if secs < 60 {
        "刚刚".into()
    } else if secs < 3600 {
        format!("{} 分钟前", secs / 60)
    } else if secs < 86400 {
        format!("{} 小时前", secs / 3600)
    } else {
        format!("{} 天前", secs / 86400)
    }
}

fn fmt_model(s: &str) -> String {
    if let Some(i) = s.find("\"id\":\"") {
        let start = i + 6;
        if let Some(j) = s[start..].find('"') {
            return s[start..start + j].to_string();
        }
    }
    if s.is_empty() {
        "—".into()
    } else {
        s.to_string()
    }
}

fn short_path(p: &str) -> String {
    if p.len() <= 42 {
        p.to_string()
    } else {
        let tail: Vec<&str> = p.rsplitn(3, '/').collect();
        let parts: Vec<&str> = tail.iter().rev().skip(1).take(2).cloned().collect();
        format!("…/{}", parts.join("/"))
    }
}

fn marker(st: SessionState) -> &'static str {
    match st {
        SessionState::Normal => "",
        SessionState::Shared => "[已分享]",
        SessionState::Active => "[进行中]",
        SessionState::Archived => "[已归档]",
    }
}

pub fn finish_report(requested: usize, deleted: usize, failed: usize) -> String {
    if failed == 0 {
        format!("已删除 {deleted}/{requested} 个会话")
    } else {
        format!("已删除 {deleted}/{requested}，失败 {failed} 个：")
    }
}

impl App {
    fn load(&mut self) {
        let (projects, sessions, shared, mode) = match fetch(&self.db_path) {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let current = self
            .state
            .current_project
            .take()
            .filter(|c| projects.iter().any(|p| &p.id == c));
        self.state = AppState::new(projects, sessions, shared, mode);
        self.state.current_project =
            current.or_else(|| self.state.projects.first().map(|p| p.id.clone()));
        self.error = None;
    }

    fn reload(&mut self) {
        self.load();
    }

    fn backup_dir() -> PathBuf {
        let base = directories::ProjectDirs::from("", "", "opencode-session-manager")
            .map(|d| d.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("backups").join(now_ms().to_string())
    }

    fn cli_delete(&self, id: &str) -> Result<(), String> {
        let st = Command::new("opencode")
            .args(["session", "delete", id])
            .status()
            .map_err(|e| e.to_string())?;
        if st.success() {
            Ok(())
        } else {
            Err(format!("opencode session delete 退出 {st}"))
        }
    }

    fn export_backup(&self, id: &str, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let out = Command::new("opencode")
            .args(["export", id])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("opencode export 失败：退出 {}", out.status));
        }
        let data = String::from_utf8_lossy(&out.stdout);
        if data.trim().is_empty() {
            return Err("opencode export 输出为空（会话可能已不存在）".into());
        }
        std::fs::write(dir.join(format!("{id}.json")), data.as_bytes()).map_err(|e| e.to_string())
    }

    fn run_direct(&mut self, ids: Vec<String>) {
        let backup = self.backup;
        let requested = ids.len();
        let mut failures: Vec<(String, String)> = Vec::new();
        let to_delete: Vec<String> = if backup {
            let dir = App::backup_dir();
            ids.iter()
                .filter(|id| match self.export_backup(id, &dir) {
                    Ok(()) => true,
                    Err(e) => {
                        failures.push(((*id).clone(), e));
                        false
                    }
                })
                .cloned()
                .collect()
        } else {
            ids
        };
        let deleted = match db::open_writable(&self.db_path)
            .and_then(|mut conn| db::delete_sessions(&mut conn, &to_delete))
        {
            Ok(n) => n,
            Err(e) => {
                failures.push(("<数据库>".into(), e.to_string()));
                0
            }
        };
        self.state.selected.clear();
        self.finished = Some(Finish {
            requested,
            deleted,
            failed: failures,
        });
        self.reload();
    }

    fn tick_cli(&mut self) -> Task<Message> {
        let done_res = {
            let r = self.delete.as_ref();
            if let Some(r) = r {
                if r.next >= r.total {
                    Some(r.finished())
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(f) = done_res {
            self.delete = None;
            self.finished = Some(f);
            self.reload();
            return Task::none();
        }
        let (id, backup) = {
            let r = self.delete.as_mut().unwrap();
            (r.ids[r.next].clone(), r.backup)
        };
        let res = if backup {
            let dir = App::backup_dir();
            match self.export_backup(&id, &dir) {
                Ok(()) => self.cli_delete(&id),
                Err(e) => Err(e),
            }
        } else {
            self.cli_delete(&id)
        };
        let finish = {
            let r = self.delete.as_mut().unwrap();
            r.next += 1;
            r.done += 1;
            if let Err(e) = res {
                r.failures.push((id, e));
            }
            if r.next >= r.total {
                Some(r.finished())
            } else {
                None
            }
        };
        match finish {
            Some(f) => {
                self.delete = None;
                self.finished = Some(f);
                self.reload();
                Task::none()
            }
            None => Task::done(Message::DeleteNext),
        }
    }
}

fn update(app: &mut App, msg: Message) -> Task<Message> {
    match msg {
        Message::SelectProject(id) => {
            app.state.current_project = Some(id);
            Task::none()
        }
        Message::ToggleSession(id) => {
            app.state.toggle_session(&id);
            Task::none()
        }
        Message::ToggleProjectAll(pid) => {
            app.state.toggle_project_all(&pid);
            Task::none()
        }
        Message::SelectAll => {
            app.state.select_all();
            Task::none()
        }
        Message::SelectNone => {
            app.state.select_none();
            Task::none()
        }
        Message::FilterChanged(s) => {
            app.state.filter = s;
            Task::none()
        }
        Message::ToggleBackup => {
            app.backup = !app.backup;
            Task::none()
        }
        Message::RequestDelete => {
            let (count, _) = app.state.confirm_summary();
            if count > 0 {
                app.dialog = Some(true);
            }
            Task::none()
        }
        Message::CancelDelete => {
            app.dialog = None;
            Task::none()
        }
        Message::ConfirmDelete => {
            app.dialog = None;
            let ids = app.state.selected_ids_in_order();
            if ids.is_empty() {
                return Task::none();
            }
            if app.state.mode == RunState::Running {
                let total = ids.len();
                app.delete = Some(DeleteRun {
                    ids,
                    next: 0,
                    total,
                    done: 0,
                    failures: Vec::new(),
                    backup: app.backup,
                });
                Task::done(Message::DeleteNext)
            } else {
                app.run_direct(ids);
                Task::none()
            }
        }
        Message::DeleteNext => app.tick_cli(),
        Message::RetryFailed => {
            let failed: Vec<String> = app
                .finished
                .as_ref()
                .map(|f| f.failed.iter().map(|(id, _)| id.clone()).collect())
                .unwrap_or_default();
            if failed.is_empty() {
                return Task::none();
            }
            app.finished = None;
            if app.state.mode == RunState::Running {
                let total = failed.len();
                app.delete = Some(DeleteRun {
                    ids: failed,
                    next: 0,
                    total,
                    done: 0,
                    failures: Vec::new(),
                    backup: app.backup,
                });
                Task::done(Message::DeleteNext)
            } else {
                app.run_direct(failed);
                Task::none()
            }
        }
        Message::CloseFinished => {
            app.finished = None;
            Task::none()
        }
        Message::Refresh => {
            app.reload();
            Task::none()
        }
    }
}

fn view<'a>(app: &'a App) -> Element<'a, Message> {
    let banner = if app.state.mode == RunState::Running {
        container(
            text("opencode 正在运行：删除将改为逐条 CLI 执行（较慢），列表可能不同步")
                .color(Color::from_rgb(1.0, 0.75, 0.3)),
        )
        .padding(6)
        .width(Fill)
    } else {
        container(
            text("opencode 未运行：删除将单事务直删（秒级）").color(Color::from_rgb(0.6, 0.9, 0.6)),
        )
        .padding(6)
        .width(Fill)
    };

    let selected = app.state.selected.len();
    let toolbar = row![
        text_input("筛选标题", &app.state.filter)
            .on_input(Message::FilterChanged)
            .width(Fill)
            .padding(6),
        button(text("全选")).on_press(Message::SelectAll),
        button(text("清空")).on_press(Message::SelectNone),
        button(text("刷新")).on_press(Message::Refresh),
        button(text(format!("删除所选 ({selected})"))).on_press_maybe(if selected > 0 {
            Some(Message::RequestDelete)
        } else {
            None
        }),
    ]
    .spacing(8)
    .align_y(Center)
    .width(Fill);

    let content = row![
        container(project_sidebar(app)).width(FillPortion(1)),
        container(session_panel(app)).width(FillPortion(3)),
    ]
    .spacing(10)
    .width(Fill)
    .height(Fill);

    let mut root = column![banner, toolbar, content]
        .spacing(10)
        .padding(12)
        .width(Fill)
        .height(Fill);

    if let Some(err) = &app.error {
        root = root.push(
            container(text(format!("错误：{err}")).color(Color::from_rgb(1.0, 0.5, 0.5)))
                .width(Fill)
                .padding(6),
        );
    }

    let mut overlays: Vec<Elem> = Vec::new();
    if let Some(run) = &app.delete {
        overlays.push(progress_panel(run));
    }
    if let Some(f) = &app.finished {
        overlays.push(finished_panel(f));
    }
    if app.dialog.is_some() {
        overlays.push(confirm_dialog(app));
    }

    let mut layers = vec![root.into()];
    layers.extend(overlays);
    layered_stack(layers)
}

fn project_sidebar<'a>(app: &'a App) -> Element<'a, Message> {
    let mut rows: Vec<Elem> = Vec::new();
    for p in &app.state.projects {
        let pid = p.id.clone();
        let visible = app.state.filtered_sessions(&p.id);
        let all_selected =
            !visible.is_empty() && visible.iter().all(|r| app.state.selected.contains(&r.id));
        let is_current = app.state.current_project.as_deref() == Some(p.id.as_str());
        let label = if is_current {
            format!("▶ {}", short_path(&p.worktree))
        } else {
            short_path(&p.worktree)
        };
        rows.push(
            row![
                checkbox(all_selected).on_toggle(move |_| Message::ToggleProjectAll(pid.clone())),
                button(text(label))
                    .on_press(Message::SelectProject(p.id.clone()))
                    .padding([4, 8]),
                text(p.sessions.to_string()).color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(8)
            .align_y(Center)
            .padding(2)
            .into(),
        );
    }
    if app.state.projects.is_empty() {
        return text(if app.error.is_some() {
            "无数据"
        } else {
            "暂无工程"
        })
        .into();
    }
    scrollable(column(rows).spacing(2).padding(4))
        .height(Fill)
        .into()
}

fn session_panel<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(pid) = app.state.current_project.as_ref() else {
        return text("请在左侧选择一个工程").into();
    };
    let rows = app.state.filtered_sessions(pid);
    let header = row![
        text("已选"),
        text("标题").width(Fill),
        text("更新时间").width(Shrink),
        text("模型").width(Shrink),
        text("成本").width(Shrink),
    ]
    .spacing(8)
    .width(Fill);

    let mut items: Vec<Elem> = Vec::new();
    for s in rows {
        let id = s.id.clone();
        let st = app.state.state_of(&id);
        let checked = app.state.selected.contains(&id);
        let title = if st == SessionState::Normal {
            s.title.clone()
        } else {
            format!("{} {}", s.title, marker(st))
        };
        items.push(
            row![
                checkbox(checked).on_toggle(move |_| Message::ToggleSession(id.clone())),
                text(title).width(Fill),
                text(rel_time(s.time_updated)).width(Shrink),
                text(fmt_model(&s.model)).width(Shrink),
                text(format!("{:.2}", s.cost)).width(Shrink),
            ]
            .spacing(10)
            .align_y(Center)
            .padding(4)
            .into(),
        );
    }
    let list: Vec<Elem> = if items.is_empty() {
        vec![text("无匹配会话").into()]
    } else {
        items
    };
    column![header, scrollable(column(list).spacing(2)).height(Fill)]
        .spacing(6)
        .width(Fill)
        .height(Fill)
        .into()
}

fn dim_background(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
        ..Default::default()
    }
}

fn overlay_box<'a>(content: Elem<'a>) -> Elem<'a> {
    container(content)
        .padding(20)
        .style(container::bordered_box)
        .max_width(720)
        .max_height(560)
        .into()
}

fn progress_panel(run: &DeleteRun) -> Elem<'_> {
    let frac = if run.total == 0 {
        0.0
    } else {
        run.done as f32 / run.total as f32
    };
    let body = column![
        text("正在删除…").size(20),
        row![
            progress_bar(0.0..=1.0, frac),
            text(format!("{}/{}", run.done, run.total)),
        ]
        .spacing(10)
        .align_y(Center),
        text(format!("失败 {} 个", run.failures.len())).color(Color::from_rgb(1.0, 0.6, 0.6))
    ]
    .spacing(14);
    overlay_box(body.into()).into()
}

fn finished_panel(f: &Finish) -> Elem<'_> {
    let mut body = column![text("删除完成").size(20)].spacing(10);
    if f.failed.is_empty() {
        body = body.push(
            text(finish_report(f.requested, f.deleted, 0)).color(Color::from_rgb(0.6, 0.9, 0.6)),
        );
    } else {
        body = body.push(
            text(finish_report(f.requested, f.deleted, f.failed.len()))
                .color(Color::from_rgb(1.0, 0.6, 0.6)),
        );
        let fail_rows: Vec<Elem> = f
            .failed
            .iter()
            .map(|(id, e)| row![text(id), text(e)].spacing(8).into())
            .collect();
        body = body.push(scrollable(column(fail_rows).spacing(4)).height(200));
    }
    let mut footer = row![button(text("关闭")).on_press(Message::CloseFinished)];
    if !f.failed.is_empty() {
        footer = footer.push(
            button(text("重试失败项"))
                .on_press(Message::RetryFailed)
                .padding([6, 12]),
        );
    }
    body = body.push(container(footer).align_right(Shrink));
    overlay_box(body.into()).into()
}

fn confirm_dialog(app: &App) -> Elem<'_> {
    let (count, projects) = app.state.confirm_summary();
    let ids = app.state.selected_ids_in_order();
    let mut special: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<Elem> = Vec::new();
    for id in &ids {
        let st = app.state.state_of(id);
        if !matches!(st, SessionState::Normal) {
            special.push((id.clone(), marker(st).to_string()));
        }
        let (title, worktree) = app.state.lookup(id).unwrap_or((id.clone(), String::new()));
        let idc = id.clone();
        entries.push(
            row![
                checkbox(app.state.selected.contains(idc.as_str()))
                    .on_toggle(move |_| Message::ToggleSession(idc.clone())),
                text(format!("{title}  —  {worktree} {m}", m = marker(st))).width(Fill),
            ]
            .spacing(8)
            .padding(2)
            .into(),
        );
    }

    let mut body = column![
        text("确认删除").size(22),
        text(format!(
            "将删除 {count} 个会话，涉及 {} 个工程。此操作不可恢复。",
            projects.len()
        ))
        .color(Color::from_rgb(0.9, 0.9, 0.9)),
    ]
    .spacing(12);

    if !special.is_empty() {
        let warn: String = special
            .iter()
            .map(|(id, m)| format!("{id} {m}"))
            .collect::<Vec<_>>()
            .join("；");
        body = body.push(
            text(format!("⚠ 含特殊会话：{warn}（已分享的删除后链接失效）"))
                .color(Color::from_rgb(1.0, 0.7, 0.4)),
        );
    }

    body = body.push(
        checkbox(app.backup)
            .label("删除前导出备份到默认目录（opencode export）")
            .on_toggle(|_| Message::ToggleBackup),
    );

    body = body.push(scrollable(column(entries).spacing(2)).height(240));

    let footer = row![
        button(text("取消")).on_press(Message::CancelDelete),
        button(text(format!("确认删除 {count} 个"))).on_press(Message::ConfirmDelete),
    ]
    .spacing(10);
    body = body.push(container(footer).align_right(Shrink));
    overlay_box(body.into()).into()
}

fn layered_stack(layers: Vec<Elem>) -> Element<Message> {
    let mut s = iced::widget::Stack::new();
    for (i, layer) in layers.into_iter().enumerate() {
        if i == 0 {
            s = s.push(layer);
        } else {
            s = s.push(
                container(layer)
                    .center_x(Fill)
                    .center_y(Fill)
                    .width(Fill)
                    .height(Fill)
                    .style(dim_background),
            );
        }
    }
    s.into()
}
