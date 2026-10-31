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

fn row_child(id: &str, title: &str, updated: i64, parent: &str) -> SessionRow {
    let mut r = row_arch(id, title, updated, false);
    r.parent_id = Some(parent.to_string());
    r
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
        parent_id: None,
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

fn tree_state() -> AppState {
    let projects = vec![project("p1", 4)];
    let mut sessions = HashMap::new();
    sessions.insert(
        "p1".to_string(),
        vec![
            row("m1", "主会话一", 100),
            row_child("c1", "子会话一", 90, "m1"),
            row_child("c2", "子会话二", 80, "m1"),
            row("m2", "主会话二", 70),
        ],
    );
    AppState::new(projects, sessions, HashSet::new(), RunState::NotRunning)
}

#[test]
fn children_hidden_by_default_and_badge_counted() {
    let s = tree_state();
    assert!(!s.show_children);
    let visible = s.filtered_sessions("p1");
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().all(|r| r.parent_id.is_none()));
    assert_eq!(s.child_count("p1", "m1"), 2);
    assert_eq!(s.child_count("p1", "m2"), 0);
    assert_eq!(s.child_count("p1", "c1"), 0);
}

#[test]
fn show_children_reveals_children() {
    let mut s = tree_state();
    s.show_children = true;
    assert_eq!(s.filtered_sessions("p1").len(), 4);
}

#[test]
fn toggling_main_selects_whole_subtree() {
    let mut s = tree_state();
    s.toggle_session("m1");
    assert_eq!(
        s.selected,
        HashSet::from(["m1".to_string(), "c1".to_string(), "c2".to_string()])
    );
    s.toggle_session("m1");
    assert!(s.selected.is_empty());
}

#[test]
fn toggling_child_selects_only_child() {
    let mut s = tree_state();
    s.toggle_session("c1");
    assert_eq!(s.selected, HashSet::from(["c1".to_string()]));
}

#[test]
fn toggle_project_all_expands_subtrees() {
    let mut s = tree_state();
    s.toggle_project_all("p1");
    assert_eq!(
        s.selected,
        HashSet::from([
            "m1".to_string(),
            "c1".to_string(),
            "c2".to_string(),
            "m2".to_string()
        ])
    );
    s.toggle_project_all("p1");
    assert!(s.selected.is_empty());
}

#[test]
fn unchecked_child_triggers_orphan_warning() {
    let mut s = tree_state();
    s.toggle_session("m1");
    s.selected.remove("c2");
    assert_eq!(s.orphan_warning_count(), 1);
    s.selected.insert("c2".to_string());
    assert_eq!(s.orphan_warning_count(), 0);
}

#[test]
fn delete_order_is_children_before_parents() {
    let mut s = tree_state();
    s.toggle_session("m1");
    s.toggle_session("m2");
    let order = s.delete_order();
    let pos_m1 = order.iter().position(|i| i == "m1").unwrap();
    let pos_c1 = order.iter().position(|i| i == "c1").unwrap();
    let pos_c2 = order.iter().position(|i| i == "c2").unwrap();
    assert!(pos_c1 < pos_m1);
    assert!(pos_c2 < pos_m1);
    assert_eq!(order.len(), 4);
}

#[test]
fn select_all_with_hidden_children_expands_subtrees() {
    let mut s = tree_state();
    s.select_all();
    assert_eq!(s.selected.len(), 4);
}

#[test]
fn reload_into_preserves_show_children_and_filter_clears_selection() {
    let mut a = tree_state();
    a.show_children = true;
    a.filter = "子会话".to_string();
    a.toggle_session("m1");
    assert_eq!(a.selected.len(), 3);
    let fresh = tree_state();
    a.reload_into(fresh, Some("p1".to_string()));
    assert!(a.show_children);
    assert_eq!(a.filter, "子会话");
    assert!(a.selected.is_empty());
    assert_eq!(a.current_project.as_deref(), Some("p1"));
}

#[test]
fn is_main_mirrors_is_child() {
    let s = tree_state();
    assert!(s.is_main("m1"));
    assert!(!s.is_main("c1"));
    assert!(s.is_child("c1"));
    assert!(!s.is_child("m1"));
}

#[test]
fn select_all_with_children_shown_counts_each_session_once() {
    let mut s = tree_state();
    s.show_children = true;
    s.select_all();
    assert_eq!(s.selected.len(), 4);
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
