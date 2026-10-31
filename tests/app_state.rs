use opencode_session_manager::app::state::{AppState, RunState, SessionState};
use opencode_session_manager::db::{ProjectCount, SessionRow};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn project(id: &str, sessions: usize) -> ProjectCount {
    ProjectCount {
        id: id.to_string(),
        worktree: format!("/{}", id),
        sessions,
    }
}

fn row(id: &str, title: &str, updated: i64) -> SessionRow {
    row_arch(id, title, updated, false)
}

fn row_arch(id: &str, title: &str, updated: i64, archived: bool) -> SessionRow {
    SessionRow {
        id: id.to_string(),
        title: title.to_string(),
        time_created: 0,
        time_updated: updated,
        cost: 0.0,
        archived,
        model: "m".to_string(),
    }
}

fn base_state(running: bool) -> AppState {
    let projects = vec![project("p1", 2), project("p2", 1)];
    let mut sessions = HashMap::new();
    sessions.insert(
        "p1".to_string(),
        vec![row("s1", "alpha", 100), row("s2", "beta", 90)],
    );
    sessions.insert("p2".to_string(), vec![row("s3", "gamma", 80)]);
    AppState::new(
        projects,
        sessions,
        HashSet::new(),
        if running {
            RunState::Running
        } else {
            RunState::NotRunning
        },
    )
}

#[test]
fn toggle_session_updates_selection() {
    let mut s = base_state(false);
    s.toggle_session("s1");
    assert!(s.selected.contains("s1"));
    assert_eq!(s.selected.len(), 1);
    s.toggle_session("s1");
    assert!(!s.selected.contains("s1"));
    assert!(s.selected.is_empty());
}

#[test]
fn toggle_project_all_selects_and_deselects() {
    let mut s = base_state(false);
    s.toggle_project_all("p1");
    assert_eq!(
        s.selected,
        HashSet::from(["s1".to_string(), "s2".to_string()])
    );
    s.toggle_session("s3");
    s.toggle_project_all("p1");
    assert!(!s.selected.contains("s1"));
    assert!(!s.selected.contains("s2"));
    assert!(s.selected.contains("s3"));
}

#[test]
fn select_all_cross_projects() {
    let mut s = base_state(false);
    s.select_all();
    assert_eq!(
        s.selected,
        HashSet::from(["s1".to_string(), "s2".to_string(), "s3".to_string()])
    );
    s.select_none();
    assert!(s.selected.is_empty());
}

#[test]
fn select_all_respects_filter() {
    let mut s = base_state(false);
    s.filter = "AL".to_string();
    s.select_all();
    assert_eq!(s.selected, HashSet::from(["s1".to_string()]));
}

#[test]
fn filtered_sessions_matches_title() {
    let mut s = base_state(false);
    assert_eq!(s.filtered_sessions("p1").len(), 2);
    s.filter = "beta".to_string();
    let f = s.filtered_sessions("p1");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].id, "s2");
    assert!(s.filtered_sessions("p2").is_empty());
}

#[test]
fn confirm_summary_aggregates_projects() {
    let mut s = base_state(false);
    s.toggle_session("s1");
    s.toggle_session("s3");
    let (count, projects) = s.confirm_summary();
    assert_eq!(count, 2);
    assert_eq!(projects, vec!["p1".to_string(), "p2".to_string()]);
}

#[test]
fn state_of_marks_shared_active_archived() {
    let now = now_ms();
    let mut projects = vec![project("p1", 4)];
    projects[0].sessions = 4;
    let mut sessions = HashMap::new();
    sessions.insert(
        "p1".to_string(),
        vec![
            row_arch("s_shared_active", "ss", now, false),
            row_arch("s_active", "a", now, false),
            row_arch("s_arch", "ar", 100, true),
            row("s_normal", "n", 100),
        ],
    );
    let mut shared = HashSet::new();
    shared.insert("s_shared_active".to_string());
    shared.insert("s_shared_only".to_string());
    let s = AppState::new(projects, sessions, shared, RunState::Running);
    assert_eq!(s.state_of("s_shared_active"), SessionState::Shared);
    assert_eq!(s.state_of("s_active"), SessionState::Active);
    assert_eq!(s.state_of("s_arch"), SessionState::Archived);
    assert_eq!(s.state_of("s_normal"), SessionState::Normal);
    assert_eq!(s.state_of("s_shared_only"), SessionState::Shared);
}

#[test]
fn selected_ids_in_order_follows_project_and_row_order() {
    let mut s = base_state(false);
    s.toggle_session("s3");
    s.toggle_session("s1");
    let ids = s.selected_ids_in_order();
    assert_eq!(ids, vec!["s1".to_string(), "s3".to_string()]);
}

#[test]
fn lookup_finds_session_title_and_project() {
    let s = base_state(false);
    let (title, worktree) = s.lookup("s2").unwrap();
    assert_eq!(title, "beta");
    assert_eq!(worktree, "/p1");
    assert!(s.lookup("nope").is_none());
}
