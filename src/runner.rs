use crate::db;
use rusqlite::Connection;
use std::process::Command;

pub trait SessionDeleter {
    fn delete(&mut self, id: &str) -> Result<(), String>;
}

pub struct CliDeleter;

impl SessionDeleter for CliDeleter {
    fn delete(&mut self, id: &str) -> Result<(), String> {
        let status = Command::new("opencode")
            .args(["session", "delete", id])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("opencode session delete 退出状态 {}", status))
        }
    }
}

pub struct DbDeleter<'a> {
    pub conn: &'a mut Connection,
}

impl SessionDeleter for DbDeleter<'_> {
    fn delete(&mut self, id: &str) -> Result<(), String> {
        db::delete_sessions(self.conn, std::slice::from_ref(&id.to_string()))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

pub fn opencode_running(detect: &dyn Fn() -> bool) -> bool {
    detect()
}

pub fn pgrep_detect() -> bool {
    let output = Command::new("pgrep").args(["-f", "opencode"]).output();
    let out = match output {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    let self_pid = std::process::id();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let pid: u32 = match line.trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == self_pid {
            continue;
        }
        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) {
            if cmdline_has_opencode(cmdline.replace('\0', " ").as_str()) {
                return true;
            }
        }
    }
    false
}

pub fn cmdline_has_opencode(cmdline: &str) -> bool {
    cmdline
        .split_whitespace()
        .flat_map(|arg| arg.split('/'))
        .any(|token| token == "opencode")
}

pub struct DeleteProgress {
    pub total: usize,
    pub done: usize,
    pub failures: Vec<(String, String)>,
}

pub fn run_deletes(
    deleter: &mut dyn SessionDeleter,
    ids: &[String],
    mut on_progress: impl FnMut(&DeleteProgress),
) -> DeleteProgress {
    let total = ids.len();
    let mut p = DeleteProgress {
        total,
        done: 0,
        failures: Vec::new(),
    };
    for id in ids {
        if let Err(err) = deleter.delete(id) {
            p.failures.push((id.clone(), err));
        }
        p.done += 1;
        on_progress(&p);
    }
    p
}
