//! The HTTP layer: the system `curl` (the dependency policy has no HTTP crate, `deny.toml`), fed its
//! configuration, including the `Authorization` header, on stdin, so no value reaches a command line.
use super::{Ctx, Failed, R};
use crate::exec;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// A finished request.
#[derive(Debug)]
pub struct Response {
    /// HTTP status.
    pub status: u16,
    /// Response headers, names lowercased.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// A header by (lowercase) name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn cfg_quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A private scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> std::io::Result<Self> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let dir =
            std::env::temp_dir().join(format!("ai-gent-repo-{}-{n}-{nanos}", std::process::id()));
        std::fs::create_dir(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One request: no redirects are followed; the headers include the credential.
///
/// # Errors
/// When `curl` is missing, times out or cannot connect (the message holds no credential).
pub fn request(
    ctx: &Ctx,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> R<Response> {
    let curl = exec::which("curl", &ctx.env).ok_or_else(|| {
        Failed::err("curl isn't installed; ai-gent repo api needs it (it is part of Windows 10+, macOS and most Linux installs)")
    })?;
    let scratch = Scratch::new()
        .map_err(|e| Failed::err(format!("cannot create a scratch directory: {}", e.kind())))?;
    let body_file = scratch.0.join("body");
    let out_file = scratch.0.join("out");
    let head_file = scratch.0.join("head");
    let mut cfg = String::from("silent\nshow-error\n");
    cfg += &format!("url = {}\n", cfg_quote(url));
    if method == "HEAD" {
        cfg += "head\n";
    } else {
        cfg += &format!("request = {}\n", cfg_quote(method));
    }
    for (k, v) in headers {
        cfg += &format!("header = {}\n", cfg_quote(&format!("{k}: {v}")));
    }
    if let Some(b) = body {
        std::fs::write(&body_file, b)
            .map_err(|e| Failed::err(format!("cannot write the request body: {}", e.kind())))?;
        cfg += &format!(
            "data-binary = {}\n",
            cfg_quote(&format!("@{}", body_file.display()))
        );
    }
    cfg += &format!("output = {}\n", cfg_quote(&out_file.display().to_string()));
    cfg += &format!(
        "dump-header = {}\n",
        cfg_quote(&head_file.display().to_string())
    );
    cfg += &format!("write-out = {}\n", cfg_quote("%{http_code}"));
    cfg += &format!("max-time = {}\n", ctx.timeout.max(1.0));
    cfg += "proto = \"=http,https\"\n";
    let mut cmd = Command::new(curl);
    cmd.args(["-K", "-"]);
    let o = exec::run(
        cmd,
        &ctx.env,
        Some(cfg.as_bytes()),
        Duration::from_secs_f64(ctx.timeout + 10.0),
    )
    .map_err(|e| Failed::err(format!("cannot run curl: {}", e.kind())))?;
    drop(cfg);
    if o.code != 0 {
        let why = if o.code == 28 || o.timed_out {
            "timed out".to_string()
        } else {
            String::from_utf8_lossy(&o.stderr).trim().replace('\n', " ")
        };
        return Err(Failed::err(format!("{method} {url}: {why}")));
    }
    let status: u16 = String::from_utf8_lossy(&o.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    let raw_head = std::fs::read_to_string(&head_file).unwrap_or_default();
    let block = raw_head.replace("\r\n", "\n");
    let last = block.trim().rsplit("\n\n").next().unwrap_or("");
    let headers = last
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    let body = std::fs::read(&out_file).unwrap_or_default();
    Ok(Response {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::cfg_quote;

    #[test]
    fn quoting() {
        assert_eq!(cfg_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
