use opencode_session_manager::app::finish_report;

#[test]
fn report_all_backup_failed_does_not_underflow() {
    let s = finish_report(5, 0, 5);
    assert!(s.contains("0/5"));
    assert!(s.contains("失败 5"));
}

#[test]
fn report_partial_failure_counts() {
    let s = finish_report(5, 3, 2);
    assert!(s.contains("3/5"));
    assert!(s.contains("失败 2"));
}

#[test]
fn report_success_omits_failure() {
    let s = finish_report(5, 5, 0);
    assert_eq!(s, "已删除 5/5 个会话");
}
