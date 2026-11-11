use opencode_session_manager::api::{ProjectCount, SessionRow};
use opencode_session_manager::app::state::{AppState, SessionState};
use std::collections::{HashMap, HashSet};

fn project(id: &str, sessions: usize) -> ProjectCount {
    ProjectCount {
        id: id.to_string(),
        worktree: format!("/{id}"),
        sessions,
    }
}

fn row_for(pid: &str, id: &str, title: &str, updated: i64) -> SessionRow {
    SessionRow {
        id: id.to_string(),
        project_id: pid.to_string(),
        title: title.to_string(),
        time_created: 0,
        time_updated: updated,
        cost: 0.0,
        model: "m".to_string(),
        parent_id: None,
    }
}

fn row(pid: &str, id: &str, title: &str, updated: i64) -> SessionRow {
    row_for(pid, id, title, updated)
}

fn row_child(pid: &str, id: &str, title: &str, updated: i64, parent: &str) -> SessionRow {
    let mut r = row_for(pid, id, title, updated);
    r.parent_id = Some(parent.to_string());
    r
}

fn base_state() -> AppState {
    let projects = vec![project("p1", 2), project("p2", 1)];
    let mut sessions = HashMap::new();
    sessions.insert(
        "p1".to_string(),
        vec![
            row("p1", "s1", "alpha", 100),
            row("p1", "s2", "beta", 90),
        ],
    );
    sessions.insert("p2".to_string(), vec![row("p2", "s3", "gamma", 80)]);
    AppState::new(projects, sessions, HashSet::new())
}

#[test]
fn toggle_session_updates_selection() {
    let mut s = base_state();
    s.toggle_session("s1");
    assert!(s.selected.contains("s1"));
    assert_eq!(s.selected.len(), 1);
    s.toggle_session("s1");
    assert!(!s.selected.contains("s1"));
    assert!(s.selected.is_empty());
}

#[test]
fn toggle_project_all_selects_and_deselects() {
    let mut s = base_state();
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
    let mut s = base_state();
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
    let mut s = base_state();
    s.filter = "AL".to_string();
    s.select_all();
    assert_eq!(s.selected, HashSet::from(["s1".to_string()]));
}

#[test]
fn filtered_sessions_matches_title() {
    let mut s = base_state();
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
            row("p1", "m1", "主会话一", 100),
            row_child("p1", "c1", "子会话一", 90, "m1"),
            row_child("p1", "c2", "子会话二", 80, "m1"),
            row("p1", "m2", "主会话二", 70),
        ],
    );
    AppState::new(projects, sessions, HashSet::new())
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
    let mut s = base_state();
    s.toggle_session("s1");
    s.toggle_session("s3");
    let (count, projects) = s.confirm_summary();
    assert_eq!(count, 2);
    assert_eq!(projects, vec!["p1".to_string(), "p2".to_string()]);
}

#[test]
fn state_of_marks_active_ids_only() {
    let projects = vec![project("p1", 2)];
    let mut sessions = HashMap::new();
    sessions.insert(
        "p1".to_string(),
        vec![
            row("p1", "s_active", "a", 100),
            row("p1", "s_normal", "n", 100),
        ],
    );
    let mut active = HashSet::new();
    active.insert("s_active".to_string());
    let s = AppState::new(projects, sessions, active);
    assert_eq!(s.state_of("s_active"), SessionState::Active);
    assert_eq!(s.state_of("s_normal"), SessionState::Normal);
    assert_eq!(s.state_of("missing"), SessionState::Normal);
}

#[test]
fn selected_ids_in_order_follows_project_and_row_order() {
    let mut s = base_state();
    s.toggle_session("s3");
    s.toggle_session("s1");
    let ids = s.selected_ids_in_order();
    assert_eq!(ids, vec!["s1".to_string(), "s3".to_string()]);
}

#[test]
fn lookup_finds_session_title_and_project() {
    let s = base_state();
    let (title, worktree) = s.lookup("s2").unwrap();
    assert_eq!(title, "beta");
    assert_eq!(worktree, "/p1");
    assert!(s.lookup("nope").is_none());
}
