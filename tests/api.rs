use opencode_session_manager::api;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

const PROJECTS_JSON: &str = r#"[
  {"id":"p1","canonical":"C:/alpha","vcs":"git","time":{"created":1,"updated":1},"sandboxes":[]},
  {"id":"p2","canonical":"C:/beta"}
]"#;

const SESSIONS_JSON: &str = r#"{
  "data": [
    {"id":"s1","projectID":"p1","model":{"id":"1","providerID":"firmior","variant":"max"},"cost":0.5,"time":{"created":100,"updated":200},"title":"主会话一"},
    {"id":"s2","projectID":"p1","parentID":"s1","model":{"id":"1","providerID":"firmior"},"time":{"created":300,"updated":400}},
    {"id":"s3","projectID":"p2","time":{"created":500,"updated":600},"title":"p2 会话"},
    {"id":"s4","projectID":"p1","title":null,"time":{"created":700,"updated":800}}
  ],
  "cursor": {"next": null}
}"#;

const ACTIVE_JSON: &str = r#"{"data":{"s1":{"type":"foreground"},"s9":{"type":"background"}}}"#;

struct FakeApi {
    calls: RefCell<Vec<String>>,
    fail: bool,
    bad_json: bool,
}

impl FakeApi {
    fn new() -> Self {
        FakeApi {
            calls: RefCell::new(Vec::new()),
            fail: false,
            bad_json: false,
        }
    }

    fn call(&self, args: &[String]) -> Result<Vec<u8>, String> {
        self.calls.borrow_mut().push(args.join(" "));
        if self.fail {
            return Err("命令执行失败".into());
        }
        let path = args.get(1).map(|s| s.as_str()).unwrap_or("");
        let body = match path {
            "/api/project" => PROJECTS_JSON.to_string(),
            "/api/session/active" => ACTIVE_JSON.to_string(),
            "/api/session" => SESSIONS_JSON.to_string(),
            _ => return Err(format!("unexpected path {path}")),
        };
        if self.bad_json && path == "/api/session" {
            return Ok(b"not json{{{\n".to_vec());
        }
        Ok(body.into_bytes())
    }

    fn runner(&self) -> impl Fn(&[String]) -> Result<Vec<u8>, String> + '_ {
        move |args| self.call(args)
    }
}

#[test]
fn fetch_projects_maps_id_and_canonical() {
    let fake = FakeApi::new();
    let projects = api::fetch_projects(&fake.runner()).expect("解析工程列表");
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].id, "p1");
    assert_eq!(projects[0].worktree, "C:/alpha");
    assert_eq!(projects[1].id, "p2");
    assert_eq!(projects[1].worktree, "C:/beta");
}

#[test]
fn load_groups_by_project_and_counts_main_only() {
    let fake = FakeApi::new();
    let (projects, sessions, active) = api::load(&fake.runner(), 5000).expect("load");
    let by_id: HashMap<_, _> = projects.into_iter().map(|p| (p.id.clone(), p)).collect();
    assert_eq!(by_id["p1"].sessions, 2, "p1 应计 2 个顶层会话（s1,s4）");
    assert_eq!(by_id["p2"].sessions, 1, "p2 应计 1 个顶层会话（s3）");
    assert_eq!(sessions["p1"].len(), 3, "p1 含子会话共 3 条");
    assert_eq!(sessions["p2"].len(), 1);
    assert_eq!(active, HashSet::from(["s1".to_string(), "s9".to_string()]));
}

#[test]
fn model_display_formats_per_spec() {
    let fake = FakeApi::new();
    let (_, sessions, _) = api::load(&fake.runner(), 5000).expect("load");
    let by_id: HashMap<_, _> = sessions
        .values()
        .flatten()
        .map(|s| (s.id.clone(), s.model.clone()))
        .collect();
    assert_eq!(by_id["s1"], "firmior:1/max", "带 variant 显示 provider: id/variant");
    assert_eq!(by_id["s2"], "firmior:1", "无 variant 显示 provider:id");
    assert_eq!(by_id["s3"], "", "无模型字段显示空");
}

#[test]
fn null_title_falls_back_to_empty() {
    let fake = FakeApi::new();
    let (_, sessions, _) = api::load(&fake.runner(), 5000).expect("load");
    let by_id: HashMap<_, _> = sessions
        .values()
        .flatten()
        .map(|s| (s.id.clone(), s.title.clone()))
        .collect();
    assert_eq!(by_id["s2"], "", "缺 title 字段兜底为空串");
    assert_eq!(by_id["s4"], "", "title:null 兜底为空串");
}

#[test]
fn fetch_sessions_single_request_returns_all() {
    let fake = FakeApi::new();
    let rows = api::fetch_sessions(&fake.runner(), 5000).expect("sessions");
    let ids: Vec<&str> = rows.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["s1", "s2", "s3", "s4"], "一次请求返回全部会话");
    let session_calls = fake
        .calls
        .borrow()
        .iter()
        .filter(|c| c.contains("/api/session "))
        .count();
    assert_eq!(session_calls, 1, "会话列表应只请求一次");
}

#[test]
fn fetch_sessions_respects_max() {
    let fake = FakeApi::new();
    let rows = api::fetch_sessions(&fake.runner(), 2).expect("sessions");
    assert_eq!(rows.len(), 2, "达到 max 即截断");
}

#[test]
fn runner_error_propagates() {
    let mut fake = FakeApi::new();
    fake.fail = true;
    let err = api::fetch_projects(&fake.runner()).expect_err("应返回错误");
    assert!(err.to_string().contains("命令执行失败"));
}

#[test]
fn invalid_json_reports_error() {
    let mut fake = FakeApi::new();
    fake.bad_json = true;
    assert!(api::fetch_sessions(&fake.runner(), 100).is_err(), "非法 JSON 应报错");
}

#[test]
#[ignore]
fn smoke_readonly_real_env() {
    let (projects, sessions, active) = api::load(&api::run_opencode_api, 5000).expect("真实环境 load");
    println!(
        "smoke api: 工程={} 会话={} 活动={}",
        projects.len(),
        sessions.len(),
        active.len()
    );
    for p in &projects {
        println!("  {} {} ({} 顶层)", p.id, p.worktree, p.sessions);
    }
    for (pid, rows) in &sessions {
        let mains = rows.iter().filter(|r| r.parent_id.is_none()).count();
        assert_eq!(
            mains,
            projects.iter().find(|p| &p.id == pid).map(|p| p.sessions).unwrap_or(0),
            "工程 {pid} 顶层会话数与统计不一致"
        );
    }
    assert!(!projects.is_empty(), "真实环境应有工程");
}
