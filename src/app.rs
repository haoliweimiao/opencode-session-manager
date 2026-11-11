pub mod state;

use crate::api;
use iced::widget::{
    button, checkbox, column, container, progress_bar, row, scrollable, text, text_input,
};
use iced::{Background, Center, Color, Element, Fill, FillPortion, Shrink, Size, Task, Theme};
use state::{AppState, SessionState};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

type Elem<'a> = Element<'a, Message>;

/// 会话列表截断上限（v2 `/api/session` 一次返回全部，此值仅作容积保护）。
const SESSION_LIMIT: usize = 5000;

#[derive(Debug, Clone)]
pub enum Message {
    SelectProject(String),
    ToggleSession(String),
    ToggleProjectAll(String),
    SelectAll,
    SelectNone,
    FilterChanged(String),
    ToggleBackup,
    ToggleShowChildren,
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
    let mut app = App {
        state: AppState::new(Vec::new(), HashMap::new(), HashSet::new()),
        error: None,
        dialog: None,
        delete: None,
        finished: None,
        backup: false,
    };
    app.load();
    (app, Task::none())
}

fn fetch() -> Result<
    (
        Vec<api::ProjectCount>,
        HashMap<String, Vec<api::SessionRow>>,
        HashSet<String>,
    ),
    String,
> {
    api::load(&api::run_opencode_api, SESSION_LIMIT).map_err(|e| e.to_string())
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
        let tail: Vec<&str> = p.rsplitn(3, ['/', '\\']).collect();
        let parts: Vec<&str> = tail.iter().rev().skip(1).take(2).cloned().collect();
        format!("…/{}", parts.join("/"))
    }
}

fn marker(st: SessionState) -> &'static str {
    match st {
        SessionState::Normal => "",
        SessionState::Active => "[进行中]",
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
        let (projects, sessions, active) = match fetch() {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let current = self
            .state
            .current_project
            .clone()
            .filter(|c| projects.iter().any(|p| &p.id == c));
        let current = current.or_else(|| projects.first().map(|p| p.id.clone()));
        let fresh = AppState::new(projects, sessions, active);
        self.state.reload_into(fresh, current);
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
            .args(["session", "export", id])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("opencode session export 失败：退出 {}", out.status));
        }
        if out.stdout.is_empty() {
            return Err("opencode session export 输出为空（会话可能已不存在）".into());
        }
        std::fs::write(dir.join(format!("{id}.json")), &out.stdout).map_err(|e| e.to_string())
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
        Message::ToggleShowChildren => {
            app.state.show_children = !app.state.show_children;
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
            let ids = app.state.delete_order();
            if ids.is_empty() {
                return Task::none();
            }
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
    let banner = container(
        text(format!(
            "opencode 数据源：v2 API · 活动会话 {} 个",
            app.state.active.len()
        ))
        .color(Color::from_rgb(0.6, 0.85, 1.0)),
    )
    .padding(6)
    .width(Fill);

    let selected = app.state.selected.len();
    let toolbar = row![
        text_input("筛选标题", &app.state.filter)
            .on_input(Message::FilterChanged)
            .width(Fill)
            .padding(6),
        checkbox(app.state.show_children)
            .label("显示子会话")
            .on_toggle(|_| Message::ToggleShowChildren),
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
        let title = if !app.state.is_child(&id) {
            let n = app.state.child_count(pid, &id);
            if n == 0 {
                title
            } else {
                format!("{title}（子×{n}）")
            }
        } else {
            format!("└ {}", title)
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
        let prefix = if app.state.is_child(id) { "(子) " } else { "" };
        let idc = id.clone();
        entries.push(
            row![
                checkbox(app.state.selected.contains(idc.as_str()))
                    .on_toggle(move |_| Message::ToggleSession(idc.clone())),
                text(format!("{prefix}{title}  —  {worktree} {m}", m = marker(st))).width(Fill),
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
            text(format!("⚠ 含进行中会话：{warn}（活动会话删除需谨慎）"))
                .color(Color::from_rgb(1.0, 0.7, 0.4)),
        );
    }

    let orphan = app.state.orphan_warning_count();
    if orphan > 0 {
        body = body.push(
            text(format!(
                "⚠ 仍有 {orphan} 个子会话未勾选：删除父会话时 v2 将级联删除这些子会话。"
            ))
            .color(Color::from_rgb(1.0, 0.5, 0.5)),
        );
    }

    body = body.push(
        checkbox(app.backup)
            .label("删除前导出备份到默认目录（opencode session export）")
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
