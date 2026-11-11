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
