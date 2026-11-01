//! Branch lookup: `git -C <dir> symbolic-ref --quiet --short HEAD`, as the shell guard, with a
//! timeout.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The current branch of `dir`, or `None` when git fails, times out (1 s) or HEAD is detached.
#[must_use]
pub fn git_branch(dir: &str) -> Option<String> {
    let mut child = Command::new("git")
        .args(["-C", dir, "symbolic-ref", "--quiet", "--short", "HEAD"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            None if start.elapsed() > Duration::from_secs(1) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    let b = out.trim_end_matches('\n').to_string();
    if b.is_empty() { None } else { Some(b) }
}
