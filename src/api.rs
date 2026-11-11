use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;

/// 单页请求数量上限（会话列表）。
const PAGE: usize = 1000;

#[derive(Debug)]
pub struct Project {
    pub id: String,
    pub worktree: String,
}

pub struct ProjectCount {
    pub id: String,
    pub worktree: String,
    pub sessions: usize,
}

pub struct SessionRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub time_created: i64,
    pub time_updated: i64,
    pub cost: f64,
    pub model: String,
    pub parent_id: Option<String>,
}

/// 通过子进程执行 `opencode api <args>` 并返回 stdout。
pub fn run_opencode_api(args: &[String]) -> Result<Vec<u8>, String> {
    let out = Command::new("opencode")
        .arg("api")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "opencode api 退出 {}：{}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

#[derive(Debug, Deserialize)]
struct ApiProject {
    id: String,
    canonical: String,
}

#[derive(Debug, Deserialize)]
struct SessionsResponse {
    data: Vec<SessionInfo>,
}

#[derive(Debug, Deserialize)]
struct SessionInfo {
    id: String,
    #[serde(default, rename = "projectID")]
    project_id: String,
    #[serde(default, rename = "parentID")]
    parent_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cost: f64,
    #[serde(default)]
    model: Option<ModelRef>,
    #[serde(default)]
    time: SessionTime,
}

#[derive(Debug, Deserialize, Default)]
struct SessionTime {
    created: i64,
    updated: i64,
}

#[derive(Debug, Deserialize, Default)]
struct ModelRef {
    id: String,
    #[serde(rename = "providerID")]
    provider_id: String,
    variant: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActiveResponse {
    data: HashMap<String, serde_json::Value>,
}

/// 工程列表：map 到 (id, 目录)。
pub fn fetch_projects(runner: &dyn Fn(&[String]) -> Result<Vec<u8>, String>) -> Result<Vec<Project>> {
    let bytes = runner(&["get".into(), "/api/project".into()])
        .map_err(|e| anyhow!("获取工程列表失败：{e}"))?;
    let list: Vec<ApiProject> = serde_json::from_slice(&bytes)
        .context("解析工程列表 JSON")?;
    Ok(list.into_iter().map(|p| Project { id: p.id, worktree: p.canonical }).collect())
}

/// 会话列表：跨全部工程（v2 `/api/session` 一次返回全部，仅按 `max` 截断兜底）。
pub fn fetch_sessions(
    runner: &dyn Fn(&[String]) -> Result<Vec<u8>, String>,
    max: usize,
) -> Result<Vec<SessionRow>> {
    let args = vec![
        "get".into(),
        "/api/session".into(),
        "--param".into(),
        format!("limit={PAGE}"),
    ];
    let bytes = runner(&args).map_err(|e| anyhow!("获取会话列表失败：{e}"))?;
    let resp: SessionsResponse = serde_json::from_slice(&bytes).context("解析会话列表 JSON")?;
    let mut rows: Vec<SessionRow> = resp.data.into_iter().map(into_row).collect();
    rows.truncate(max);
    Ok(rows)
}

/// 活动会话 id 集合。
pub fn fetch_active(
    runner: &dyn Fn(&[String]) -> Result<Vec<u8>, String>,
) -> Result<HashSet<String>> {
    let bytes = runner(&["get".into(), "/api/session/active".into()])
        .map_err(|e| anyhow!("获取活动会话失败：{e}"))?;
    let resp: ActiveResponse = serde_json::from_slice(&bytes).context("解析活动会话 JSON")?;
    Ok(resp.data.into_keys().collect())
}

/// 全量加载：工程（含顶层会话计数）、按工程分组的会话、活动会话集合。
pub fn load(
    runner: &dyn Fn(&[String]) -> Result<Vec<u8>, String>,
    max: usize,
) -> Result<(Vec<ProjectCount>, HashMap<String, Vec<SessionRow>>, HashSet<String>)> {
    let projects = fetch_projects(runner)?;
    let sessions = fetch_sessions(runner, max)?;
    let active = fetch_active(runner)?;

    let mut by_project: HashMap<String, Vec<SessionRow>> = HashMap::new();
    for row in sessions {
        by_project.entry(row.project_id.clone()).or_default().push(row);
    }

    let counts = projects
        .into_iter()
        .map(|p| {
            let sessions = by_project
                .get(&p.id)
                .map(|rows| rows.iter().filter(|r| r.parent_id.is_none()).count())
                .unwrap_or(0);
            ProjectCount {
                id: p.id,
                worktree: p.worktree,
                sessions,
            }
        })
        .collect();
    Ok((counts, by_project, active))
}

fn into_row(info: SessionInfo) -> SessionRow {
    let model = match info.model {
        Some(m) => {
            let base = if m.provider_id.is_empty() {
                m.id
            } else {
                format!("{}:{}", m.provider_id, m.id)
            };
            match m.variant {
                Some(v) if !v.is_empty() => format!("{base}/{v}"),
                _ => base,
            }
        }
        None => String::new(),
    };
    SessionRow {
        id: info.id,
        project_id: info.project_id,
        title: info.title.filter(|t| !t.is_empty()).unwrap_or_default(),
        time_created: info.time.created,
        time_updated: info.time.updated,
        cost: info.cost,
        model,
        parent_id: info.parent_id,
    }
}
