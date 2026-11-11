use crate::api::{ProjectCount, SessionRow};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
    Normal,
    Active,
}

pub struct AppState {
    pub projects: Vec<ProjectCount>,
    pub sessions: HashMap<String, Vec<SessionRow>>,
    pub active: HashSet<String>,
    pub selected: HashSet<String>,
    pub filter: String,
    pub current_project: Option<String>,
    pub show_children: bool,
}

impl AppState {
    pub fn new(
        projects: Vec<ProjectCount>,
        sessions: HashMap<String, Vec<SessionRow>>,
        active: HashSet<String>,
    ) -> Self {
        AppState {
            projects,
            sessions,
            active,
            selected: HashSet::new(),
            filter: String::new(),
            current_project: None,
            show_children: false,
        }
    }

    pub fn is_child(&self, id: &str) -> bool {
        self.row_of(id).map(|r| r.parent_id.is_some()).unwrap_or(false)
    }

    pub fn is_main(&self, id: &str) -> bool {
        self.row_of(id).map(|r| r.parent_id.is_none()).unwrap_or(false)
    }

    fn row_of(&self, id: &str) -> Option<&SessionRow> {
        self.sessions
            .values()
            .flatten()
            .find(|r| r.id == id)
    }

    fn project_of(&self, id: &str) -> Option<String> {
        for p in &self.projects {
            if self
                .sessions
                .get(&p.id)
                .map(|rows| rows.iter().any(|r| r.id == id))
                .unwrap_or(false)
            {
                return Some(p.id.clone());
            }
        }
        None
    }

    fn children_of(&self, project_id: &str, id: &str) -> Vec<String> {
        self.sessions
            .get(project_id)
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.parent_id.as_deref() == Some(id))
                    .map(|r| r.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn child_count(&self, project_id: &str, id: &str) -> usize {
        self.children_of(project_id, id).len()
    }

    fn add_subtree(&mut self, project_id: &str, id: &str) {
        self.selected.insert(id.to_string());
        for c in self.children_of(project_id, id) {
            self.add_subtree(project_id, &c);
        }
    }

    fn remove_subtree(&mut self, project_id: &str, id: &str) {
        self.selected.remove(id);
        for c in self.children_of(project_id, id) {
            self.remove_subtree(project_id, &c);
        }
    }

    fn filter_matches(&self, r: &SessionRow) -> bool {
        self.filter.is_empty() || r.title.to_lowercase().contains(&self.filter.to_lowercase())
    }

    fn set_selected(&mut self, project_id: &str, id: &str, selected: bool) {
        if selected {
            if self.is_child(id) {
                self.selected.insert(id.to_string());
            } else {
                self.add_subtree(project_id, id);
            }
        } else if self.is_child(id) {
            self.selected.remove(id);
        } else {
            self.remove_subtree(project_id, id);
        }
    }

    pub fn toggle_session(&mut self, id: &str) {
        if let Some(pid) = self.project_of(id) {
            let selected = !self.selected.contains(id);
            self.set_selected(&pid, id, selected);
        }
    }

    pub fn toggle_project_all(&mut self, project_id: &str) {
        let visible: Vec<String> = self
            .filtered_sessions(project_id)
            .into_iter()
            .map(|r| r.id.clone())
            .collect();
        let all_selected =
            !visible.is_empty() && visible.iter().all(|id| self.selected.contains(id));
        for id in visible {
            self.set_selected(project_id, &id, !all_selected);
        }
    }

    pub fn select_all(&mut self) {
        self.selected.clear();
        let mut mains: Vec<(String, String)> = Vec::new();
        let mut main_ids: HashSet<String> = HashSet::new();
        for (pid, rows) in &self.sessions {
            for r in rows {
                if !self.filter_matches(r) || self.is_child(&r.id) {
                    continue;
                }
                main_ids.insert(r.id.clone());
                mains.push((pid.clone(), r.id.clone()));
            }
        }
        for (_, rows) in &self.sessions {
            for r in rows {
                if !self.filter_matches(r) {
                    continue;
                }
                if let Some(p) = r.parent_id.as_deref() {
                    if !main_ids.contains(p) {
                        self.selected.insert(r.id.clone());
                    }
                }
            }
        }
        for (pid, id) in mains {
            self.add_subtree(&pid, &id);
        }
    }

    pub fn select_none(&mut self) {
        self.selected.clear();
    }

    pub fn filtered_sessions(&self, project_id: &str) -> Vec<&SessionRow> {
        self.sessions
            .get(project_id)
            .map(|rows| {
                rows.iter()
                    .filter(|r| (self.show_children || self.is_main(&r.id)) && self.filter_matches(r))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn lookup(&self, id: &str) -> Option<(String, String)> {
        for p in &self.projects {
            if let Some(rows) = self.sessions.get(&p.id) {
                if let Some(r) = rows.iter().find(|r| r.id == id) {
                    return Some((r.title.clone(), p.worktree.clone()));
                }
            }
        }
        None
    }

    pub fn selected_ids_in_order(&self) -> Vec<String> {
        let mut out = Vec::new();
        for p in &self.projects {
            if let Some(rows) = self.sessions.get(&p.id) {
                for r in rows {
                    if self.selected.contains(&r.id) {
                        out.push(r.id.clone());
                    }
                }
            }
        }
        out
    }

    pub fn delete_order(&self) -> Vec<String> {
        let mut pending = self.selected_ids_in_order();
        let mut out = Vec::new();
        while let Some(pos) = pending.iter().position(|id| {
            pending
                .iter()
                .all(|other| self.parent_of(other).as_deref() != Some(id.as_str()))
        }) {
            out.push(pending.remove(pos));
        }
        out.extend(pending);
        out
    }

    fn parent_of(&self, id: &str) -> Option<String> {
        self.row_of(id).and_then(|r| r.parent_id.clone())
    }

    pub fn confirm_summary(&self) -> (usize, Vec<String>) {
        let mut projects = Vec::new();
        for p in &self.projects {
            if self
                .sessions
                .get(&p.id)
                .map(|rows| rows.iter().any(|r| self.selected.contains(&r.id)))
                .unwrap_or(false)
            {
                projects.push(p.id.clone());
            }
        }
        (self.selected.len(), projects)
    }

    pub fn orphan_warning_count(&self) -> usize {
        let mut count = 0;
        for p in &self.projects {
            let mains: Vec<String> = self
                .sessions
                .get(&p.id)
                .map(|rows| {
                    rows.iter()
                        .filter(|r| self.is_main(&r.id) && self.selected.contains(&r.id))
                        .map(|r| r.id.clone())
                        .collect()
                })
                .unwrap_or_default();
            for main in mains {
                for c in self.children_of(&p.id, &main) {
                    if !self.selected.contains(&c) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    pub fn reload_into(&mut self, other: AppState, current_project: Option<String>) {
        let show_children = self.show_children;
        let filter = std::mem::take(&mut self.filter);
        *self = other;
        self.show_children = show_children;
        self.filter = filter;
        self.current_project = current_project;
    }

    pub fn state_of(&self, id: &str) -> SessionState {
        if self.active.contains(id) {
            SessionState::Active
        } else {
            SessionState::Normal
        }
    }
}
