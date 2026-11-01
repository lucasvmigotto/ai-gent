//! The container engine as a subprocess. Every listing is filtered by this project's label or
//! compose project, every removal goes by ID, nothing prunes (`shared/containers.md` section 9a).
use super::{COMPOSE_LABEL, Failed, LABEL_PROJECT, LABEL_WORKTREE, Res, project, tail};
use crate::tokens::pyjson::{J, parse};
use std::path::Path;
use std::process::Command;

/// Whether an executable named `name` is on `PATH`.
#[must_use]
pub fn which(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let names: Vec<String> = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&path).any(|d| {
        names.iter().any(|n| {
            let p = d.join(n);
            p.is_file() && executable(&p)
        })
    })
}

#[cfg(unix)]
fn executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    true
}

/// `$CONTAINER_ENGINE`, else `podman`, else `docker` (`containers.md` section 1).
#[must_use]
pub fn engine() -> Option<String> {
    #[cfg(feature = "test-support")]
    if std::env::var_os("AI_GENT_WT_NO_ENGINE").is_some() {
        return None;
    }
    if let Some(e) = std::env::var("CONTAINER_ENGINE")
        .ok()
        .filter(|e| !e.is_empty())
    {
        return Some(e);
    }
    ["podman", "docker"]
        .into_iter()
        .find(|c| which(c))
        .map(str::to_string)
}

/// Run the engine; a failure is an error unless `ok`, in which case it is the empty string.
pub fn ctr(e: &str, args: &[&str], ok: bool) -> Res<String> {
    let label = || {
        let name = Path::new(e)
            .file_name()
            .map_or(e.to_string(), |n| n.to_string_lossy().to_string());
        format!(
            "{name} {}",
            args.iter().take(3).copied().collect::<Vec<_>>().join(" ")
        )
    };
    let out = Command::new(e)
        .args(args)
        .output()
        .map_err(|x| Failed(format!("{}: {x}", label())))?;
    if !out.status.success() {
        if ok {
            return Ok(String::new());
        }
        return Err(Failed(format!(
            "{}: {}",
            label(),
            tail(String::from_utf8_lossy(&out.stderr).trim(), 300)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Whitespace-separated IDs of a listing, without repeats.
pub fn ids(e: &str, args: &[&str]) -> Res<Vec<String>> {
    Ok(dedupe(
        ctr(e, args, false)?
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    ))
}

fn dedupe(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// A track's containers and volumes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Owned {
    /// Container IDs.
    pub containers: Vec<String>,
    /// Volume names.
    pub volumes: Vec<String>,
}

/// A track's containers and volumes: its worktree label, or its compose project.
pub fn owned(e: &str, root: &str, track: &str) -> Res<Owned> {
    let p = project(root);
    let mut found = Owned::default();
    for f in [
        format!("label={LABEL_WORKTREE}={p}/{track}"),
        format!("label={COMPOSE_LABEL}={p}-{track}"),
    ] {
        found
            .containers
            .extend(ids(e, &["ps", "-aq", "--filter", &f])?);
        found
            .volumes
            .extend(ids(e, &["volume", "ls", "-q", "--filter", &f])?);
    }
    Ok(Owned {
        containers: dedupe(found.containers),
        volumes: dedupe(found.volumes),
    })
}

/// Remove a track's labelled containers, then its volumes, by ID; verify by recount.
pub fn release(e: &str, root: &str, track: &str) -> Res<Vec<String>> {
    let found = owned(e, root, track)?;
    for c in &found.containers {
        ctr(e, &["rm", "-f", c], true)?;
    }
    for v in &found.volumes {
        ctr(e, &["volume", "rm", v], true)?;
    }
    let left = owned(e, root, track)?;
    let mut removed = Vec::new();
    removed.extend(
        found
            .containers
            .iter()
            .filter(|x| !left.containers.contains(x))
            .map(|x| format!("container {x}")),
    );
    removed.extend(
        found
            .volumes
            .iter()
            .filter(|x| !left.volumes.contains(x))
            .map(|x| format!("volume {x}")),
    );
    if !left.containers.is_empty() || !left.volumes.is_empty() {
        let names: Vec<String> = left
            .containers
            .iter()
            .chain(left.volumes.iter())
            .cloned()
            .collect();
        return Err(Failed(format!(
            "still there after removal: {} (in use? another store? containers.md \u{a7}9); the worktree is kept, fix and re-run",
            names.join(", ")
        )));
    }
    Ok(removed)
}

/// A container the engine does not manage (a buildah working container).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct External {
    /// Container ID.
    pub id: String,
    /// Name.
    pub name: String,
}

/// Containers in the store the engine doesn't manage: buildah working containers (Podman only).
pub fn external(e: &str) -> Res<Vec<External>> {
    let out = ctr(
        e,
        &[
            "ps",
            "-a",
            "--external",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Status}}",
        ],
        true,
    )?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let p: Vec<&str> = line.split('\t').collect();
            (p.len() == 3 && p[2].to_lowercase().contains("storage")).then(|| External {
                id: p[0].to_string(),
                name: p[1].to_string(),
            })
        })
        .collect())
}

/// This project's dangling images.
pub fn dangling(e: &str, root: &str) -> Res<Vec<String>> {
    ids(
        e,
        &[
            "images",
            "-q",
            "--filter",
            "dangling=true",
            "--filter",
            &format!("label={LABEL_PROJECT}={}", project(root)),
        ],
    )
}

/// A volume carrying this project's labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// Volume name.
    pub name: String,
    /// The track it belongs to.
    pub track: String,
}

/// This project's volumes (own label, or a compose project named `<project>-...`).
pub fn labelled_volumes(e: &str, root: &str) -> Res<Vec<Volume>> {
    let p = project(root);
    let mut names = ids(
        e,
        &[
            "volume",
            "ls",
            "-q",
            "--filter",
            &format!("label={LABEL_PROJECT}={p}"),
        ],
    )?;
    for n in ids(
        e,
        &[
            "volume",
            "ls",
            "-q",
            "--filter",
            &format!("label={COMPOSE_LABEL}"),
        ],
    )? {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["volume", "inspect"];
    args.extend(names.iter().map(String::as_str));
    let text = ctr(e, &args, false)?;
    let Some(J::Arr(items)) = parse(if text.is_empty() { "[]" } else { &text }) else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for v in &items {
        let label = |k: &str| -> String {
            v.get("Labels")
                .and_then(|l| l.get(k))
                .and_then(J::as_str)
                .unwrap_or("")
                .to_string()
        };
        let compose = label(COMPOSE_LABEL);
        if label(LABEL_PROJECT) != p && !compose.starts_with(&format!("{p}-")) {
            continue; // another project's compose volume
        }
        let wl = label(LABEL_WORKTREE);
        let mut track = wl
            .split_once('/')
            .map(|(_, t)| t.to_string())
            .unwrap_or_default();
        if track.is_empty() {
            track = compose.chars().skip(p.chars().count() + 1).collect();
        }
        if track.is_empty() {
            track = "main".into();
        }
        rows.push(Volume {
            name: v.get("Name").and_then(J::as_str).unwrap_or("").to_string(),
            track,
        });
    }
    Ok(rows)
}
