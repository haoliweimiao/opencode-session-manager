use opencode_session_manager::api;

#[test]
#[ignore]
fn smoke_readonly_load_real_api() {
    let (projects, sessions, active) = api::load(&api::run_opencode_api, 5000)
        .expect("真实环境 api load 失败");
    let total: usize = sessions.values().map(|v| v.len()).sum();
    let mains: usize = sessions
        .values()
        .flat_map(|v| v.iter())
        .filter(|r| r.parent_id.is_none())
        .count();
    println!(
        "smoke: 工程={} 会话={} 顶层={} 活动={}",
        projects.len(),
        total,
        mains,
        active.len()
    );
    assert!(
        projects.len() >= 10,
        "预期 >=10 工程，实际 {}",
        projects.len()
    );
    assert!(total >= 10, "预期 >=10 会话，实际 {total}");
    assert_eq!(
        mains,
        projects.iter().map(|p| p.sessions).sum::<usize>(),
        "顶层会话计数与工程统计不一致"
    );
}
