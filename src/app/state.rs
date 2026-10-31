use crate::db::{ProjectCount, SessionRow};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

const ACTIVE_WINDOW_MS: i64 = 5 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunState {
    Running,
    NotRunning,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
    Normal,
    Shared,
    Active,
    Archived,
}

pub struct AppState {
    pub projects: Vec<ProjectCount>,
    pub sessions: HashMap<String, Vec<SessionRow>>,
    pub shared: HashSet<String>,
    pub selected: HashSet<String>,
    pub filter: String,
    pub current_project: Option<String>,
    pub mode: RunState,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl AppState {
    pub fn new(
        projects: Vec<ProjectCount>,
        sessions: HashMap<String, Vec<SessionRow>>,
        shared: HashSet<String>,
        mode: RunState,
    ) -> Self {
        let mut id_to_project = HashMap::new();
        for (pid, rows) in &sessions {
            for r in rows {
                id_to_project.insert(r.id.clone(), pid.clone());
            }
        }
        AppState {
            projects,
            sessions,
            shared,
            selected: HashSet::new(),
            filter: String::new(),
            current_project: None,
            mode,
        }
    }

    pub fn toggle_session(&mut self, id: &str) {
        if !self.selected.remove(id) {
            self.selected.insert(id.to_string());
        }
    }

    pub fn toggle_project_all(&mut self, project_id: &str) {
        let ids: Vec<String> = self
            .filtered_sessions(project_id)
            .into_iter()
            .map(|r| r.id.clone())
            .collect();
        let all_selected = !ids.is_empty() && ids.iter().all(|id| self.selected.contains(id));
        if all_selected {
            for id in &ids {
                self.selected.remove(id);
            }
        } else {
            for id in ids {
                self.selected.insert(id);
            }
        }
    }

    fn all_current_ids(&self) -> HashSet<String> {
        let filter = self.filter.to_lowercase();
        let mut ids = HashSet::new();
        for rows in self.sessions.values() {
            for r in rows {
                if filter.is_empty() || r.title.to_lowercase().contains(&filter) {
                    ids.insert(r.id.clone());
                }
            }
        }
        ids
    }

    pub fn select_all(&mut self) {
        self.selected = self.all_current_ids();
    }

    pub fn select_none(&mut self) {
        self.selected.clear();
    }

    pub fn filtered_sessions(&self, project_id: &str) -> Vec<&SessionRow> {
        let filter = self.filter.to_lowercase();
        self.sessions
            .get(project_id)
            .map(|rows| {
                rows.iter()
                    .filter(|r| filter.is_empty() || r.title.to_lowercase().contains(&filter))
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

    pub fn state_of(&self, id: &str) -> SessionState {
        if self.shared.contains(id) {
            return SessionState::Shared;
        }
        for rows in self.sessions.values() {
            if let Some(row) = rows.iter().find(|r| r.id == id) {
                if self.mode == RunState::Running && now_ms() - row.time_updated <= ACTIVE_WINDOW_MS
                {
                    return SessionState::Active;
                }
                if row.archived {
                    return SessionState::Archived;
                }
                return SessionState::Normal;
            }
        }
        SessionState::Normal
    }
}
