//! Running child processes: finding a program on `PATH`, and a run with a timeout.
use crate::env::Env;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The full path of `name` on the `PATH` of `env`, honouring `PATHEXT` on Windows.
#[must_use]
pub fn which(name: &str, env: &Env) -> Option<PathBuf> {
    let path = env.text("PATH").or_else(|| env.text("Path"))?;
    let exts: Vec<String> = if cfg!(windows) {
        let pe = env
            .text("PATHEXT")
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
        std::iter::once(String::new())
            .chain(
                pe.split(';')
                    .filter(|e| !e.is_empty())
                    .map(str::to_lowercase),
            )
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&OsString::from(path)) {
        for ext in &exts {
            let cand = dir.join(format!("{name}{ext}"));
            if is_executable(&cand) {
                return Some(cand);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// What a finished child printed.
#[derive(Debug)]
pub struct Output {
    /// Exit status code (124 when the run timed out, -1 when killed by a signal).
    pub code: i32,
    /// Whether the run was killed for taking too long.
    pub timed_out: bool,
    /// Standard output bytes.
    pub stdout: Vec<u8>,
    /// Standard error bytes.
    pub stderr: Vec<u8>,
}

/// Run `cmd` with exactly the environment `env`, `input` on stdin (or none), and a timeout.
///
/// # Errors
/// When the program cannot be started.
pub fn run(
    mut cmd: Command,
    env: &Env,
    input: Option<&[u8]>,
    timeout: Duration,
) -> std::io::Result<Output> {
    cmd.env_clear();
    for (k, v) in env.iter() {
        cmd.env(k, v);
    }
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let t_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut v);
        }
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(o) = err.as_mut() {
            let _ = o.read_to_end(&mut v);
        }
        v
    });
    if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
        let data = data.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
    }
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            timed_out = true;
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    Ok(Output {
        code: if timed_out {
            124
        } else {
            status.code().unwrap_or(-1)
        },
        timed_out,
        stdout,
        stderr,
    })
}
