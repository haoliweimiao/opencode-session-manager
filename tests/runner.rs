use opencode_session_manager::runner::{
    SessionDeleter, cmdline_has_opencode, opencode_running, run_deletes,
};

struct MockDeleter<'a> {
    fail: &'a [&'a str],
}

impl SessionDeleter for MockDeleter<'_> {
    fn delete(&mut self, id: &str) -> Result<(), String> {
        if self.fail.contains(&id) {
            Err(format!("boom {id}"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn opencode_running_detects() {
    assert!(opencode_running(&|| true));
    assert!(!opencode_running(&|| false));
}

#[test]
fn cmdline_token_matcher_finds_opencode_not_self() {
    assert!(cmdline_has_opencode("node /x/opencode/dist/index.js foo"));
    assert!(cmdline_has_opencode("opencode run hello world"));
    assert!(cmdline_has_opencode("/usr/bin/opencode"));
    assert!(!cmdline_has_opencode("/x/opencode-session-manager --help"));
    assert!(!cmdline_has_opencode("/x/opencode_session_manager foo"));
}

#[test]
fn run_deletes_collects_failures() {
    let mut calls = 0;
    let mut deleter = MockDeleter { fail: &["s2"] };
    let ids = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
    let p = run_deletes(&mut deleter, &ids, |_| calls += 1);
    assert_eq!(p.total, 3);
    assert_eq!(p.done, 3);
    assert_eq!(p.failures.len(), 1);
    assert_eq!(p.failures[0].0, "s2");
    assert_eq!(calls, 3);
}

#[test]
fn run_deletes_all_success_clears_failures() {
    let mut deleter = MockDeleter { fail: &[] };
    let ids = vec!["s1".to_string(), "s2".to_string()];
    let p = run_deletes(&mut deleter, &ids, |_| {});
    assert!(p.failures.is_empty());
    assert_eq!(p.done, 2);
}
