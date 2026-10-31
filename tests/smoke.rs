use opencode_session_manager::db;
use opencode_session_manager::runner;
use std::path::Path;
use std::process::Command;

#[test]
#[ignore]
fn smoke_readonly_load_real_db() {
    let out = Command::new("opencode")
        .args(["db", "path"])
        .output()
        .expect("opencode 应在 PATH 中");
    assert!(out.status.success(), "opencode db path 失败");
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!path.is_empty(), "opencode db path 输出为空");

    let conn = db::open_readonly(Path::new(&path)).expect("以只读打开真实库");
    let projects = db::load_project_counts(&conn).expect("读取工程+计数");
    let shared = db::load_shared_ids(&conn).expect("读取共享 id");
    let mut total = 0usize;
    for p in &projects {
        total += db::load_sessions(&conn, &p.id).expect("读取会话").len();
    }

    let running = runner::opencode_running(&runner::pgrep_detect);
    println!(
        "smoke: 工程={} 会话={} 已分享={} 运行中={} db={}",
        projects.len(),
        total,
        shared.len(),
        running,
        path
    );
    assert!(
        projects.len() >= 10,
        "预期 >=10 工程，实际 {}",
        projects.len()
    );
    assert!(total >= 100, "预期 >=100 会话，实际 {}", total);
}
