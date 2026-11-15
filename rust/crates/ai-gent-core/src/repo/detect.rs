//! `detect`: the platform, the CLI and the token state, without decrypting anything.
use super::entry::{default_entry, pass_has};
use super::platforms::platform;
use super::remote::{Remote, detect_repo, jira_url};
use super::{Ctx, out};
use crate::exec;
use crate::tokens::pyjson::{J, dumps, obj};
use std::process::Command;
use std::time::Duration;

fn status_ok(ctx: &Ctx, prog: &str, args: &[&str], secs: u64, strip_tokens: bool) -> bool {
    let mut env = ctx.env.clone();
    if strip_tokens {
        for k in ["GH_TOKEN", "GITHUB_TOKEN", "GITLAB_TOKEN"] {
            env.remove(k);
        }
    }
    let Some(path) = exec::which(prog, &ctx.env) else {
        return false;
    };
    let mut c = Command::new(path);
    c.args(args);
    exec::run(c, &env, None, Duration::from_secs(secs)).is_ok_and(|o| o.code == 0)
}

/// The state of a platform's CLI.
struct CliState {
    name: Option<&'static str>,
    installed: bool,
    signed_in: Option<bool>,
    devops_extension: Option<bool>,
}

fn cli_state(ctx: &Ctx, plat: &str) -> CliState {
    let name = platform(plat).and_then(|p| p.cli);
    let Some(n) = name else {
        return CliState {
            name: None,
            installed: false,
            signed_in: None,
            devops_extension: None,
        };
    };
    let installed = exec::which(n, &ctx.env).is_some();
    let mut st = CliState {
        name,
        installed,
        signed_in: None,
        devops_extension: None,
    };
    if installed && (plat == "github" || plat == "gitlab") {
        st.signed_in = Some(status_ok(ctx, n, &["auth", "status"], 20, true));
    }
    if installed && plat == "azure-devops" {
        st.devops_extension = Some(status_ok(
            ctx,
            "az",
            &["extension", "show", "--name", "azure-devops", "-o", "none"],
            30,
            false,
        ));
    }
    st
}

struct PlatformReport {
    name: String,
    entry: String,
    in_pass: bool,
    cli: CliState,
    url: Option<String>,
}

fn remote_json(r: &Remote) -> J {
    J::Obj(r.facts.iter().map(|(k, v)| (k.clone(), J::s(v))).collect())
}

/// Run `detect`; prints text or JSON.
pub fn run(ctx: &Ctx, json: bool) {
    let repo = detect_repo(ctx);
    let mut wanted: Vec<String> = Vec::new();
    if !repo.str("platform").is_empty() {
        wanted.push(repo.str("platform").to_string());
    }
    if !jira_url(ctx, None).is_empty() || pass_has(ctx, &default_entry(ctx, "jira")) {
        wanted.push("jira".to_string());
    }
    let mut reports: Vec<PlatformReport> = Vec::new();
    for p in wanted {
        if platform(&p).is_none() {
            continue;
        }
        let entry = default_entry(ctx, &p);
        let url = (p == "jira").then(|| {
            let u = jira_url(ctx, None);
            if u.is_empty() {
                "(set git config ai-gent.jira.url, or url: in the entry)".to_string()
            } else {
                u
            }
        });
        let in_pass = pass_has(ctx, &entry);
        let cli = cli_state(ctx, &p);
        if let Some(existing) = reports.iter_mut().find(|r| r.name == p) {
            *existing = PlatformReport {
                name: p,
                entry,
                in_pass,
                cli,
                url,
            };
        } else {
            reports.push(PlatformReport {
                name: p,
                entry,
                in_pass,
                cli,
                url,
            });
        }
    }
    if json {
        let plats = reports
            .iter()
            .map(|r| {
                let mut cli = vec![
                    ("name", J::opt_s(r.cli.name)),
                    ("installed", J::Bool(r.cli.installed)),
                ];
                if let Some(s) = r.cli.signed_in {
                    cli.push(("signed_in", J::Bool(s)));
                }
                if let Some(s) = r.cli.devops_extension {
                    cli.push(("devops_extension", J::Bool(s)));
                }
                let mut m = vec![
                    ("token_entry", J::s(&r.entry)),
                    ("token_in_pass", J::Bool(r.in_pass)),
                    ("cli", obj(cli)),
                ];
                if let Some(u) = &r.url {
                    m.push(("url", J::s(u)));
                }
                (r.name.clone(), obj(m))
            })
            .collect();
        let report = obj(vec![
            ("repository", remote_json(&repo)),
            ("platforms", J::Obj(plats)),
        ]);
        out(&dumps(&report, true, Some(2)));
        return;
    }
    let url = repo.str("url");
    out(&format!(
        "remote    {}",
        if url.is_empty() { "(none)" } else { url }
    ));
    let pf = repo.str("platform");
    out(&format!(
        "platform  {}",
        if pf.is_empty() {
            "unknown \u{2014} set git config ai-gent.platform <github|gitlab|azure-devops|bitbucket|forgejo>"
        } else {
            pf
        }
    ));
    for k in ["owner", "org", "project", "namespace", "workspace", "repo"] {
        if !repo.str(k).is_empty() {
            out(&format!("{k:<9} {}", repo.str(k)));
        }
    }
    for r in &reports {
        let how = if r.cli.installed { "CLI" } else { "REST API" };
        let mut extra = String::new();
        if let Some(s) = r.cli.signed_in {
            extra = if s { ", signed in" } else { ", not signed in" }.to_string();
        }
        if let Some(s) = r.cli.devops_extension {
            extra = format!(
                ", azure-devops extension{}",
                if s {
                    ""
                } else {
                    " MISSING (az extension add --name azure-devops)"
                }
            );
        }
        out(&format!(
            "\n{}: {how} ({}{}{extra})",
            r.name,
            r.cli.name.unwrap_or("no CLI"),
            if r.cli.installed { " installed" } else { "" }
        ));
        out(&format!(
            "  token   {} \u{2014} {}",
            r.entry,
            if r.in_pass { "in pass" } else { "not in pass" }
        ));
        if r.name == "jira" {
            out(&format!("  url     {}", r.url.clone().unwrap_or_default()));
        }
        if !r.in_pass {
            out(&format!(
                "  add it in your own terminal: pass insert -m {}",
                r.entry
            ));
        }
    }
}
