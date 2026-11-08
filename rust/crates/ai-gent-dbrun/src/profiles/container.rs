//! Local containers: engine discovery, the proof that a profile is local, local profile discovery and
//! the published-port detection of FR-020 (a published port selects the Rust driver, none selects the
//! container exec path of T014). Ported from `dbrun.py` (`engine_cmd`, `engine_endpoint_local`,
//! `list_containers`, `local_profiles`, `classify`), with every child process bounded.
use super::{
    Class, EXIT_CONNECT, Profile, ProfileError, SecretSource, Source, default_port, inside,
};
use ai_gent_core::env::Env;
use ai_gent_core::passstore::Secret;
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Seconds each container-engine call may take.
pub const ENGINE_TIMEOUT: Duration = Duration::from_secs(30);
/// Most bytes of a container-engine answer taken.
pub const MAX_ENGINE_OUTPUT: usize = 8 * 1024 * 1024;
/// Most containers inspected for discovery.
pub const MAX_CANDIDATES: usize = 64;

/// What a container-engine call returned.
#[derive(Debug, Clone, Default)]
pub struct EngineOut {
    /// Exit code; 124 for a timeout, 127 when the program could not run.
    pub code: i32,
    /// Standard output.
    pub stdout: String,
}

/// A container engine (podman or docker), or a fake in tests.
pub trait ContainerEngine: Send + Sync {
    /// The program's base name (`podman`, `docker`).
    fn program(&self) -> String;
    /// Runs the engine with `args`. Bounded by [`ENGINE_TIMEOUT`].
    fn run(&self, args: &[&str]) -> EngineOut;
}

/// The real engine: a program run with the process environment, a timeout and an output cap.
pub struct CliEngine {
    path: PathBuf,
    env: Env,
}

impl ContainerEngine for CliEngine {
    fn program(&self) -> String {
        let stem = self
            .path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        stem.to_lowercase()
    }

    fn run(&self, args: &[&str]) -> EngineOut {
        let mut cmd = Command::new(&self.path);
        cmd.args(args);
        match ai_gent_core::exec::run(cmd, &self.env, None, ENGINE_TIMEOUT) {
            Ok(o) if o.stdout.len() <= MAX_ENGINE_OUTPUT => EngineOut {
                code: o.code,
                stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            },
            Ok(o) => EngineOut {
                code: if o.code == 0 { 1 } else { o.code },
                stdout: String::new(),
            },
            Err(_) => EngineOut {
                code: 127,
                stdout: String::new(),
            },
        }
    }
}

/// `CONTAINER_ENGINE` when set (and found), else `podman`, else `docker`.
#[must_use]
pub fn detect_engine(env: &Env) -> Option<Arc<dyn ContainerEngine>> {
    let find = |name: &str| -> Option<PathBuf> {
        if name.contains('/') || name.contains('\\') {
            let p = PathBuf::from(name);
            return p.is_file().then_some(p);
        }
        ai_gent_core::exec::which(name, env)
    };
    let path = match env.text("CONTAINER_ENGINE").filter(|v| !v.is_empty()) {
        Some(e) => find(&e)?,
        None => find("podman").or_else(|| find("docker"))?,
    };
    Some(Arc::new(CliEngine {
        path,
        env: env.clone(),
    }))
}

/// Whether a container name is safe to hand to the engine as one argument: the engines' own name rule
/// (`[A-Za-z0-9][A-Za-z0-9_.-]*`), so a name can never be read as an option (`-v`, `--privileged`).
#[must_use]
pub fn valid_container_name(n: &str) -> bool {
    let mut c = n.chars();
    n.len() <= 255
        && c.next().is_some_and(|f| f.is_ascii_alphanumeric())
        && c.all(|x| x.is_ascii_alphanumeric() || matches!(x, '_' | '.' | '-'))
}

fn re_proxy() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)pgbouncer|pgpool|cloud-sql-proxy|cloudsql|socat|haproxy|envoy|proxysql|odyssey|nginx|traefik").unwrap()
    })
}

/// The engine a container image runs; proxies in front of a database are not it.
#[must_use]
pub fn image_engine(image: &str) -> Option<&'static str> {
    static E: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    if re_proxy().is_match(image) {
        return None;
    }
    let e = E.get_or_init(|| {
        [
            ("postgresql", r"(?i)postgres|postgis|timescale"),
            ("mariadb", r"(?i)mariadb"),
            ("mysql", r"(?i)(^|/)mysql|percona"),
            ("sqlserver", r"(?i)mssql|sql-server|azure-sql-edge"),
            ("oracle", r"(?i)oracle|gvenzl"),
        ]
        .into_iter()
        .map(|(n, p)| (n, Regex::new(p).unwrap()))
        .collect()
    });
    e.iter().find(|(_, r)| r.is_match(image)).map(|(n, _)| *n)
}

/// Whether the engine runs on this machine: no `DOCKER_HOST` or `CONTAINER_HOST` other than a `unix://`
/// socket; podman reports a local service; docker's context endpoint is a `unix://` socket.
#[must_use]
pub fn engine_endpoint_local(engine: &dyn ContainerEngine, env: &Env) -> (bool, String) {
    for var in ["DOCKER_HOST", "CONTAINER_HOST"] {
        if let Some(v) = env.text(var).filter(|v| !v.is_empty())
            && !v.starts_with("unix://")
        {
            let scheme = v.split("://").next().unwrap_or("");
            return (false, format!("{var}={scheme}:// is not a local socket"));
        }
    }
    if engine.program() == "podman" {
        let out = engine.run(&["info", "--format", "{{.Host.ServiceIsRemote}}"]);
        if out.code != 0 || out.stdout.trim() != "false" {
            return (
                false,
                "podman reports a remote service (or `podman info` failed)".into(),
            );
        }
        return (true, "podman service is local".into());
    }
    let out = engine.run(&[
        "context",
        "inspect",
        "--format",
        "{{.Endpoints.docker.Host}}",
    ]);
    let host = out.stdout.trim().to_string();
    if out.code != 0 || !host.starts_with("unix://") {
        let shown = if host.is_empty() { "?" } else { host.as_str() };
        return (
            false,
            format!("docker context endpoint {shown} is not a local socket"),
        );
    }
    (true, format!("docker endpoint {host}"))
}

/// How a local profile is reached (FR-020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// The container publishes the engine's port on loopback: use the Rust driver on `host:port`.
    Published {
        /// Loopback address of the mapping (`127.0.0.1` or `::1`).
        host: String,
        /// The host port.
        port: u16,
    },
    /// No usable published port: use the engine's own client inside the container (T014).
    ContainerExec,
}

/// Published-port detection from an `inspect` document: the first mapping of `<container_port>/tcp`
/// whose host address is the wildcard or a loopback address and whose host port is a valid port.
/// Everything else, host networking, a mapping bound to another address, a missing or malformed
/// entry, is [`Route::ContainerExec`]: the driver is never pointed at an address the proof did not cover.
#[must_use]
pub fn detect_route(info: &Value, container_port: u16) -> Route {
    let ports = info.pointer("/NetworkSettings/Ports");
    let Some(list) = ports
        .and_then(|p| p.get(format!("{container_port}/tcp")))
        .and_then(Value::as_array)
    else {
        return Route::ContainerExec;
    };
    for m in list {
        let ip = m.get("HostIp").and_then(Value::as_str).unwrap_or("");
        let port = m
            .get("HostPort")
            .and_then(Value::as_str)
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|p| *p != 0);
        let host = match ip {
            "" | "0.0.0.0" => Some("127.0.0.1".to_string()),
            "::" | "::1" => Some("::1".to_string()),
            other if crate::core::tls::is_loopback_host(other) => Some(other.to_string()),
            _ => None,
        };
        if let (Some(host), Some(port)) = (host, port) {
            return Route::Published { host, port };
        }
    }
    Route::ContainerExec
}

/// A container the engine lists.
#[derive(Debug, Clone)]
struct Listed {
    name: String,
    image: String,
    labels: BTreeMap<String, String>,
}

fn list_containers(engine: &dyn ContainerEngine) -> Vec<Listed> {
    let out = engine.run(&["ps", "--no-trunc", "--format", "json"]);
    let text = out.stdout.trim();
    if out.code != 0 || text.is_empty() {
        return vec![];
    }
    let items: Vec<Value> = if text.starts_with('[') {
        serde_json::from_str(text).unwrap_or_default()
    } else {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    };
    items
        .into_iter()
        .filter_map(|item| {
            let labels = match item.get("Labels") {
                Some(Value::Object(o)) => o
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                    .collect(),
                Some(Value::String(s)) => s
                    .split(',')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                _ => BTreeMap::new(),
            };
            let name = match item.get("Names") {
                Some(Value::Array(a)) => a.first().and_then(Value::as_str),
                Some(Value::String(s)) => Some(s.as_str()),
                _ => None,
            }?
            .to_string();
            Some(Listed {
                name,
                image: item
                    .get("Image")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                labels,
            })
        })
        .collect()
}

fn inspect(engine: &dyn ContainerEngine, container: &str) -> Option<Value> {
    let out = engine.run(&["inspect", "--format", "{{json .}}", container]);
    if out.code != 0 {
        return None;
    }
    let v: Value = serde_json::from_str(out.stdout.trim()).ok()?;
    match v {
        Value::Array(mut a) if !a.is_empty() => Some(a.remove(0)),
        Value::Array(_) => None,
        other => Some(other),
    }
}

fn container_env(info: &Value) -> BTreeMap<String, String> {
    info.pointer("/Config/Env")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(|e| e.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// The credentials a database image takes from its environment (what the container was started with).
#[derive(Debug, Default)]
struct Creds {
    user: Option<String>,
    password: Option<String>,
    database: Option<String>,
    service: Option<String>,
}

fn credentials_from_env(eng: &str, env: &BTreeMap<String, String>) -> Creds {
    let g = |k: &str| env.get(k).cloned();
    let s = |k: &str| env.get(k).cloned().unwrap_or_default();
    match eng {
        "postgresql" => {
            let user = env
                .get("POSTGRES_USER")
                .cloned()
                .unwrap_or_else(|| "postgres".into());
            Creds {
                password: Some(s("POSTGRES_PASSWORD")),
                database: Some(
                    env.get("POSTGRES_DB")
                        .cloned()
                        .unwrap_or_else(|| user.clone()),
                ),
                user: Some(user),
                service: None,
            }
        }
        "mysql" | "mariadb" => {
            let p = if eng == "mariadb" && env.keys().any(|k| k.starts_with("MARIADB_")) {
                "MARIADB_"
            } else {
                "MYSQL_"
            };
            match g(&format!("{p}USER")).filter(|u| !u.is_empty()) {
                Some(u) => Creds {
                    user: Some(u),
                    password: Some(s(&format!("{p}PASSWORD"))),
                    database: Some(s(&format!("{p}DATABASE"))),
                    service: None,
                },
                None => Creds {
                    user: Some("root".into()),
                    password: Some(s(&format!("{p}ROOT_PASSWORD"))),
                    database: Some(s(&format!("{p}DATABASE"))),
                    service: None,
                },
            }
        }
        "sqlserver" => Creds {
            user: Some("sa".into()),
            password: Some(
                g("MSSQL_SA_PASSWORD")
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| s("SA_PASSWORD")),
            ),
            database: Some("master".into()),
            service: None,
        },
        "oracle" => match g("APP_USER").filter(|u| !u.is_empty()) {
            Some(u) => Creds {
                user: Some(u),
                password: Some(s("APP_USER_PASSWORD")),
                service: Some("FREEPDB1".into()),
                database: None,
            },
            None => Creds {
                user: Some("system".into()),
                password: Some(s("ORACLE_PASSWORD")),
                service: Some("FREEPDB1".into()),
                database: None,
            },
        },
        _ => Creds::default(),
    }
}

/// Profiles for the database containers of this repository's compose projects that are running
/// (`local:<service>`). A remote engine's containers are never local; an engine that fails lists none.
#[must_use]
pub fn local_profiles(
    engine: Option<&dyn ContainerEngine>,
    env: &Env,
    root: &Path,
) -> BTreeMap<String, Profile> {
    let mut profiles = BTreeMap::new();
    let Some(engine) = engine else {
        return profiles;
    };
    if !engine_endpoint_local(engine, env).0 {
        return profiles;
    }
    let mut seen = 0;
    for c in list_containers(engine) {
        let wd = c.labels.get("com.docker.compose.project.working_dir");
        let Some(eng) = image_engine(&c.image) else {
            continue;
        };
        let Some(wd) = wd.filter(|w| inside(Path::new(w), root)) else {
            continue;
        };
        if !valid_container_name(&c.name) {
            continue;
        }
        seen += 1;
        if seen > MAX_CANDIDATES {
            break;
        }
        let service = c
            .labels
            .get("com.docker.compose.service")
            .cloned()
            .unwrap_or_else(|| c.name.clone());
        let _ = wd;
        let info = inspect(engine, &c.name);
        let creds = info
            .as_ref()
            .map(|i| credentials_from_env(eng, &container_env(i)))
            .unwrap_or_default();
        let cport = default_port(eng).unwrap_or(0);
        let route = info.as_ref().map(|i| detect_route(i, cport));
        let name = format!("local:{service}");
        let mut p = Profile::blank(&name, Source::RunningContainer);
        p.engine = eng.to_string();
        p.class = Class::Local;
        p.container = Some(c.name.clone());
        p.port = Some(cport.to_string());
        p.host = Some("127.0.0.1".into());
        p.secret = SecretSource::Container;
        p.user = creds.user;
        p.database = creds.database;
        p.service = creds.service;
        p.password = creds.password.filter(|v| !v.is_empty()).map(Secret::new);
        p.route = route;
        profiles.insert(name, p);
    }
    profiles
}

/// How the profile was shown to be local (or not), and what the runner needs to reach it.
#[derive(Debug, Clone)]
pub struct Classification {
    /// `Local` only when proven.
    pub class: Class,
    /// Whether writes are allowed (only for a proven local database).
    pub writable: bool,
    /// The evidence lines.
    pub evidence: Vec<String>,
    /// How the database was reached for the proof.
    pub via: Via,
    /// The proven container, when there is one.
    pub container: Option<ContainerFacts>,
    /// Published-port detection for a proven container.
    pub route: Option<Route>,
}

/// How a classification was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// A SQLite file.
    File,
    /// A host and port, declared.
    Host,
    /// A running container.
    Container,
}

/// Facts of a proven container (what `network_args` and the exec path need).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerFacts {
    /// The container id.
    pub id: String,
    /// The pod, when podman runs it in one.
    pub pod: String,
    /// The main process id (0 when unknown).
    pub pid: u64,
}

/// Returns the classification of a profile: `local` only when proven.
///
/// # Errors
/// A SQLite file that does not exist (exit code 4).
pub fn classify(
    p: &Profile,
    engine: Option<&dyn ContainerEngine>,
    env: &Env,
    root: &Path,
) -> Result<Classification, ProfileError> {
    let mut evidence: Vec<String> = vec![];
    let declared = p.class;

    if p.engine == "sqlite" {
        let path = super::expand_home(p.path.as_deref().unwrap_or(""), env);
        if !path.is_file() {
            return Err(ProfileError::with_code(
                format!("sqlite file {} doesn't exist", path.display()),
                EXIT_CONNECT,
            ));
        }
        let real = path.canonicalize().unwrap_or_else(|_| path.clone());
        let is_link = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink());
        if inside(&real, root) && !is_link {
            let shown = root
                .canonicalize()
                .ok()
                .and_then(|r| real.strip_prefix(r).ok().map(Path::to_path_buf))
                .unwrap_or_default();
            evidence.push(format!("file inside this repository ({})", shown.display()));
            return Ok(Classification {
                class: Class::Local,
                writable: true,
                evidence,
                via: Via::File,
                container: None,
                route: None,
            });
        }
        evidence.push("file outside this repository — not a local copy of this project".into());
        return Ok(Classification {
            class: if declared == Class::Local {
                Class::Production
            } else {
                declared
            },
            writable: false,
            evidence,
            via: Via::File,
            container: None,
            route: None,
        });
    }

    let wants_local = declared == Class::Local || p.container.is_some();
    if !wants_local {
        evidence.push(format!(
            "declared {} in {}",
            declared.as_str(),
            p.source_label()
        ));
        return Ok(Classification {
            class: declared,
            writable: false,
            evidence,
            via: Via::Host,
            container: None,
            route: None,
        });
    }

    let not_proven = |mut evidence: Vec<String>, reason: &str| -> Classification {
        evidence.push(format!("NOT PROVEN LOCAL: {reason}"));
        Classification {
            class: if declared == Class::Local {
                Class::Production
            } else {
                declared
            },
            writable: false,
            evidence,
            via: Via::Host,
            container: None,
            route: None,
        }
    };

    let Some(engine) = engine else {
        return Ok(not_proven(
            evidence,
            "no container engine (podman or docker) on this machine",
        ));
    };
    let (ok, why) = engine_endpoint_local(engine, env);
    if !ok {
        return Ok(not_proven(evidence, &why));
    }
    evidence.push(why);
    let Some(container) = p.container.as_deref() else {
        return Ok(not_proven(evidence, "no container named for this profile"));
    };
    if !valid_container_name(container) {
        return Ok(not_proven(
            evidence,
            "the container name is not a valid container name (letters, digits, `_`, `.`, `-`; no leading `-`)",
        ));
    }
    let Some(info) = inspect(engine, container) else {
        return Ok(not_proven(
            evidence,
            &format!("container {container} not found"),
        ));
    };
    if info.pointer("/State/Running").and_then(Value::as_bool) != Some(true) {
        return Ok(not_proven(
            evidence,
            &format!("container {container} is not running"),
        ));
    }
    let image = info
        .pointer("/Config/Image")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let img_engine = image_engine(&image);
    if img_engine != Some(p.engine.as_str())
        && !(p.engine == "mysql" && img_engine == Some("mariadb"))
    {
        return Ok(not_proven(
            evidence,
            &format!(
                "container image {image} is not a {} server (a proxy or another engine)",
                p.engine
            ),
        ));
    }
    let wd = info
        .pointer("/Config/Labels/com.docker.compose.project.working_dir")
        .and_then(Value::as_str)
        .unwrap_or("");
    if wd.is_empty() || !inside(Path::new(wd), root) {
        return Ok(not_proven(
            evidence,
            &format!("container {container} wasn't started by this repository's compose project"),
        ));
    }
    let rel = Path::new(wd)
        .canonicalize()
        .ok()
        .zip(root.canonicalize().ok())
        .and_then(|(w, r)| w.strip_prefix(r).ok().map(Path::to_path_buf))
        .unwrap_or_default();
    let rel = if rel.as_os_str().is_empty() {
        ".".to_string()
    } else {
        rel.display().to_string()
    };
    evidence.push(format!(
        "container {container} ({image}) running, compose project in {rel}"
    ));
    let cport = p
        .port
        .as_deref()
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| default_port(&p.engine))
        .unwrap_or(0);
    let route = detect_route(&info, cport);
    match &route {
        Route::Published { host, port } => {
            evidence.push(format!("reached through the published port {host}:{port}"));
        }
        Route::ContainerExec => evidence.push(
            "no published port on loopback: reached through the engine's client inside the container".into(),
        ),
    }
    let s = |ptr: &str| {
        info.pointer(ptr)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    Ok(Classification {
        class: Class::Local,
        writable: true,
        evidence,
        via: Via::Container,
        container: Some(ContainerFacts {
            id: s("/Id"),
            pod: s("/Pod"),
            pid: info
                .pointer("/State/Pid")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        }),
        route: Some(route),
    })
}
