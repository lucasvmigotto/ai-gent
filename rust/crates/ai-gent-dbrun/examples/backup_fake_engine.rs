//! Test-only container-engine stand-in; built before backup integration tests run.
//! Controls are synthetic per-child environment values. Password input stays on stdin.
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn main() -> io::Result<()> {
    match std::env::var("AI_GENT_BACKUP_FAKE_MODE").as_deref() {
        Ok("echo") => {
            let path = fixture_path()?;
            let args = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
            std::fs::write(path, args)?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            writeln!(
                io::stdout().lock(),
                "got:{}",
                line.trim_end_matches(['\r', '\n'])
            )?;
        }
        Ok("hang") => std::thread::sleep(Duration::from_secs(30)),
        Ok("flood") => {
            let mut out = io::stdout().lock();
            loop {
                out.write_all(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n")?;
            }
        }
        Ok("mark") => {
            std::fs::write(fixture_path()?, b"")?;
            io::copy(&mut io::stdin().lock(), &mut io::sink())?;
        }
        _ => return Err(io::Error::other("unknown backup fixture mode")),
    }
    Ok(())
}

fn fixture_path() -> io::Result<std::ffi::OsString> {
    std::env::var_os("AI_GENT_BACKUP_FAKE_PATH")
        .ok_or_else(|| io::Error::other("missing backup fixture path"))
}
