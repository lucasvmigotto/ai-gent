//! The disk under the engine's store: usage, the floor, the preflight of `add`.
use super::engine::ctr;
use super::{Failed, GB, Out, Res, Settings, g};
use std::process::Command;

/// `(total, free, path)` in bytes of the filesystem holding the engine's storage.
pub fn store_usage(e: &str) -> Res<(u64, u64, String)> {
    #[cfg(feature = "test-support")]
    if let Ok(v) = std::env::var("AI_GENT_WT_FAKE_USAGE") {
        let p: Vec<&str> = v.splitn(3, ',').collect();
        if let [t, f, path] = p[..] {
            if let (Ok(t), Ok(f)) = (t.parse(), f.parse()) {
                return Ok((t, f, path.to_string()));
            }
        }
    }
    let mut path = ctr(e, &["info", "--format", "{{.Store.GraphRoot}}"], true)?;
    if path.is_empty() {
        path = ctr(e, &["info", "--format", "{{.DockerRootDir}}"], true)?;
    }
    if path.is_empty() {
        path = root_path();
    }
    if let Some((t, f)) = disk_usage(&path) {
        return Ok((t, f, path));
    }
    let path = root_path();
    disk_usage(&path)
        .map(|(t, f)| (t, f, path))
        .ok_or_else(|| Failed("cannot read the disk usage".into()))
}

fn root_path() -> String {
    if cfg!(windows) {
        std::env::var("SystemDrive").map_or("C:\\".into(), |d| format!("{d}\\"))
    } else {
        "/".into()
    }
}

#[cfg(unix)]
fn disk_usage(path: &str) -> Option<(u64, u64)> {
    // `df -Pk`: POSIX output, 1024-byte blocks; the fourth column is what an unprivileged user may use.
    let out = Command::new("df").args(["-Pk", path]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let f: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
    Some((
        f.get(1)?.parse::<u64>().ok()? * 1024,
        f.get(3)?.parse::<u64>().ok()? * 1024,
    ))
}

#[cfg(windows)]
fn disk_usage(path: &str) -> Option<(u64, u64)> {
    let drive = path.chars().next().filter(char::is_ascii_alphabetic)?;
    let script = format!("$d = Get-PSDrive -Name {drive}; \"$($d.Used + $d.Free) $($d.Free)\"");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut it = text.split_whitespace();
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// The floor in bytes: the flag, else `disk_floor_gb`, else `max(20 GB, 10% of the store)`.
#[must_use]
pub fn floor(s: &Settings, total: u64, flag: Option<f64>) -> f64 {
    if let Some(f) = flag {
        return f * GB;
    }
    s.get("disk_floor_gb")
        .map_or_else(|| (20.0 * GB).max((total / 10) as f64), |f| f * GB)
}

/// Refuse a new track when the container store can't hold one more volume per live track.
pub fn preflight(
    root: &str,
    e: &str,
    live: usize,
    min_free_gb: Option<f64>,
    out: &mut Out,
) -> Res<()> {
    let s = super::settings(root);
    let cap = s.volume_cap();
    if live as i64 >= cap {
        return Err(Failed(format!(
            "{live} track(s) already hold a volume each; the cap is {cap} (max_volumes in .ai-gent/parallel.yaml). Land or remove one first"
        )));
    }
    let (total, free, path) = store_usage(e)?;
    let fl = floor(&s, total, min_free_gb);
    let est = s.volume_estimate_gb();
    let need = (live + 1) as f64 * est * GB;
    if (free as f64) - need < fl {
        out.eline(ctr(e, &["system", "df"], true)?);
        return Err(Failed(format!(
            "{:.1} GB free on {path}; {} track(s) at {} GB each would leave less than the {:.0} GB floor. `wt.py resources` lists this project's leftovers; never clean another project's to make room. Start fewer tracks, or ask (--min-free-gb overrides the floor)",
            free as f64 / GB,
            live + 1,
            g(est),
            fl / GB
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_floor_prefers_the_flag_then_the_setting_then_ten_percent() {
        let s = Settings {
            values: vec![("disk_floor_gb".into(), 7.0)],
        };
        assert!((floor(&s, 500_000_000_000, Some(0.0))).abs() < 1.0);
        assert!((floor(&s, 500_000_000_000, None) - 7.0 * GB).abs() < 1.0);
        let none = Settings { values: vec![] };
        assert!((floor(&none, 500_000_000_000, None) - 50.0 * GB).abs() < 1.0);
        assert!((floor(&none, 100_000_000_000, None) - 20.0 * GB).abs() < 1.0);
    }
}
