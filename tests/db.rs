use opencode_session_manager::db::{
    delete_sessions, load_project_counts, load_sessions, load_shared_ids, open_readonly,
    open_writable,
};
use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn build_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT NOT NULL);
         CREATE TABLE session (
           id TEXT PRIMARY KEY,
           project_id TEXT NOT NULL,
           title TEXT NOT NULL,
           model TEXT,
           cost REAL DEFAULT 0,
           time_created INTEGER NOT NULL,
           time_updated INTEGER NOT NULL,
           time_archived INTEGER,
           FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE
         );
         CREATE TABLE message (
           id TEXT PRIMARY KEY,
           session_id TEXT NOT NULL,
           FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
         );
         CREATE TABLE part (
           id TEXT PRIMARY KEY,
           message_id TEXT NOT NULL,
           session_id TEXT NOT NULL,
           FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
         );
         CREATE TABLE session_share (
           session_id TEXT PRIMARY KEY,
           FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
         );",
    )
}

fn seed(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "INSERT INTO project (id, worktree) VALUES ('p1', '/a'), ('p2', '/b');
         INSERT INTO session (id, project_id, title, model, time_created, time_updated)
           VALUES ('s1', 'p1', 'alpha', 'm1', 100, 300),
                  ('s2', 'p1', 'beta',  'm2', 100, 200);
         INSERT INTO session (id, project_id, title, time_created, time_updated)
           VALUES ('s3', 'p2', 'gamma', 100, 500);",
    )
}

#[test]
fn load_project_counts_aggregates() {
    let conn = open_writable(Path::new(":memory:")).unwrap();
    build_schema(&conn).unwrap();
    seed(&conn).unwrap();

    let counts = load_project_counts(&conn).unwrap();
    assert_eq!(counts.len(), 2);
    let p1 = counts.iter().find(|c| c.id == "p1").unwrap();
    assert_eq!(p1.worktree, "/a");
    assert_eq!(p1.sessions, 2);
    let p2 = counts.iter().find(|c| c.id == "p2").unwrap();
    assert_eq!(p2.sessions, 1);
}

#[test]
fn load_sessions_ordered_desc() {
    let conn = open_writable(Path::new(":memory:")).unwrap();
    build_schema(&conn).unwrap();
    seed(&conn).unwrap();

    let sessions = load_sessions(&conn, "p1").unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, "s1");
    assert_eq!(sessions[0].title, "alpha");
    assert_eq!(sessions[0].time_updated, 300);
    assert_eq!(sessions[1].id, "s2");

    let empty = load_sessions(&conn, "nope").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn load_shared_ids_set() {
    let conn = open_writable(Path::new(":memory:")).unwrap();
    build_schema(&conn).unwrap();
    seed(&conn).unwrap();
    conn.execute("INSERT INTO session_share (session_id) VALUES ('s1')", [])
        .unwrap();
    conn.execute("INSERT INTO session_share (session_id) VALUES ('s2')", [])
        .unwrap();

    let shared = load_shared_ids(&conn).unwrap();
    assert_eq!(shared, HashSet::from(["s1".to_string(), "s2".to_string()]));
}

#[test]
fn delete_sessions_cascades_children() {
    let mut conn = open_writable(Path::new(":memory:")).unwrap();
    build_schema(&conn).unwrap();
    seed(&conn).unwrap();
    conn.execute(
        "INSERT INTO message (id, session_id) VALUES ('m1', 's1')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO part (id, message_id, session_id) VALUES ('pt1', 'm1', 's1')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message (id, session_id) VALUES ('m2', 's3')",
        [],
    )
    .unwrap();

    let n = delete_sessions(&mut conn, &["s1".to_string(), "s2".to_string()]).unwrap();
    assert_eq!(n, 2);

    let messages: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message WHERE session_id = 's1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(messages, 0);
    let parts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM part WHERE session_id = 's1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(parts, 0);

    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 1);
    let projects: i64 = conn
        .query_row("SELECT COUNT(*) FROM project", [], |r| r.get(0))
        .unwrap();
    assert_eq!(projects, 2);
}

#[test]
fn open_readonly_rejects_writes() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "osm_ro_test_{}.db",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let conn = open_writable(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (id TEXT PRIMARY KEY); INSERT INTO t VALUES ('x');")
            .unwrap();
    }
    let ro = open_readonly(&path).unwrap();
    let err = ro
        .execute("INSERT INTO t VALUES ('y')", [])
        .expect_err("write on readonly connection must fail");
    assert!(
        err.to_string().to_lowercase().contains("readonly")
            || err.to_string().contains("read-only"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_file(&path);
}
