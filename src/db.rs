use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::path::Path;

pub struct ProjectCount {
    pub id: String,
    pub worktree: String,
    pub sessions: usize,
}

pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub time_created: i64,
    pub time_updated: i64,
    pub cost: f64,
    pub archived: bool,
    pub model: String,
    pub parent_id: Option<String>,
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("enable foreign_keys")?;
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .context("set busy_timeout")?;
    Ok(())
}

pub fn open_readonly(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open db read-only: {}", path.display()))?;
    configure(&conn)?;
    Ok(conn)
}

pub fn open_writable(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("open db: {}", path.display()))?;
    configure(&conn)?;
    Ok(conn)
}

pub fn load_project_counts(conn: &Connection) -> Result<Vec<ProjectCount>> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.worktree, COUNT(s.id)
             FROM project p
             LEFT JOIN session s ON s.project_id = p.id AND s.parent_id IS NULL
             GROUP BY p.id, p.worktree
             ORDER BY p.worktree",
        )
        .context("prepare project counts")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ProjectCount {
                id: r.get(0)?,
                worktree: r.get(1)?,
                sessions: r.get::<_, i64>(2)? as usize,
            })
        })
        .context("query project counts")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("map project count")?);
    }
    Ok(out)
}

pub fn load_sessions(conn: &Connection, project_id: &str) -> Result<Vec<SessionRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, time_created, time_updated, cost, time_archived, model, parent_id
             FROM session
             WHERE project_id = ?1
             ORDER BY time_updated DESC",
        )
        .context("prepare sessions")?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                title: r.get(1)?,
                time_created: r.get(2)?,
                time_updated: r.get(3)?,
                cost: r.get(4)?,
                archived: r.get::<_, Option<i64>>(5).map(|t| t.is_some())?,
                model: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                parent_id: r.get::<_, Option<String>>(7)?,
            })
        })
        .context("query sessions")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("map session row")?);
    }
    Ok(out)
}

pub fn load_shared_ids(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn
        .prepare("SELECT session_id FROM session_share")
        .context("prepare shared ids")?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .context("query shared ids")?;
    let mut out = HashSet::new();
    for row in rows {
        out.insert(row.context("map shared id")?);
    }
    Ok(out)
}

pub fn delete_sessions(conn: &mut Connection, ids: &[String]) -> Result<usize> {
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("enable foreign_keys for delete")?;
    let tx = conn.transaction().context("begin delete transaction")?;
    let mut removed = 0;
    for id in ids {
        removed += tx
            .execute("DELETE FROM session WHERE id = ?1", [id])
            .with_context(|| format!("delete session {id}"))?;
    }
    tx.commit().context("commit delete transaction")?;
    Ok(removed)
}
