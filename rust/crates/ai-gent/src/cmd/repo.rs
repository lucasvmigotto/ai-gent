//! `ai-gent repo`: detect, cli, api, project-status, batch and chain (a port of `reporun.py`, spec 037
//! Group C). Exit codes: 0 ok, 1 error, 2 usage, 3 refused (the repo contract, `contracts/cli.md`);
//! `cli` passes the child's own status through.
use ai_gent_core::exit::Exit;
use ai_gent_core::passstore::redact;
use ai_gent_core::repo::{
    self, Ctx, Failed,
    api::{Call, api_call, render},
    batch, chain, cli as repo_cli, detect,
    project_status::{Status, project_status_call},
};
use ai_gent_core::tokens::pyjson::dumps;
use clap::{Args, Subcommand};
use std::io::Read;

/// Arguments of `repo`.
#[derive(Debug, Args)]
pub struct RepoArgs {
    /// What to do.
    #[command(subcommand)]
    pub action: RepoAction,
}

/// Repo subcommands.
#[derive(Debug, Subcommand)]
pub enum RepoAction {
    /// Platform, CLI and token state for this repository (nothing decrypted).
    Detect,
    /// Run the platform's own CLI with the token from pass.
    Cli {
        /// github, azure-devops, gitlab or jira.
        #[arg(value_parser = ["github", "azure-devops", "gitlab", "jira"])]
        platform: String,
        /// The user asked for this specific change.
        #[arg(long)]
        write: bool,
        /// Another pass entry under the prefix.
        #[arg(long)]
        entry: Option<String>,
        /// The CLI's own arguments (after `--`).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Read a GitHub Projects v2 board's status field and set an item's option.
    ProjectStatus {
        /// The project number.
        number: i64,
        /// Login of the project's owner (default: the remote's owner).
        #[arg(long)]
        owner: Option<String>,
        /// The issue or pull request URL.
        #[arg(long)]
        url: String,
        /// The single-select field's name.
        #[arg(long, default_value = "Status")]
        field: String,
        /// A sync state, mapped to a live option.
        #[arg(long, value_parser = ["todo", "doing", "review", "done"])]
        state: Option<String>,
        /// The exact option name to set.
        #[arg(long)]
        value: Option<String>,
        /// The user asked for this specific change.
        #[arg(long)]
        write: bool,
        /// Another pass entry under the prefix.
        #[arg(long)]
        entry: Option<String>,
    },
    /// Several calls from a JSON plan, one confirmation.
    Batch {
        /// The plan file.
        file: String,
        /// The user confirmed this batch.
        #[arg(long)]
        write: bool,
        /// List the steps, call nothing.
        #[arg(long)]
        dry_run: bool,
        /// Another pass entry under the prefix.
        #[arg(long)]
        entry: Option<String>,
        /// Also write the result JSON here.
        #[arg(long)]
        out: Option<String>,
        /// Seconds per request.
        #[arg(long, default_value_t = 30.0)]
        timeout: f64,
    },
    /// The branch chain: work, integration, optional staging, production.
    Chain,
    /// Call the platform's REST API with the token from pass.
    Api {
        /// github, azure-devops, gitlab, bitbucket, forgejo or jira.
        #[arg(value_parser = ["github", "azure-devops", "gitlab", "bitbucket", "forgejo", "jira"])]
        platform: String,
        /// HTTP method.
        method: String,
        /// Relative to the API base; `{owner}` `{repo}` `{org}` `{project}` `{workspace}` `{project_path}` are filled from the remote.
        path: String,
        /// Request body file, or `-` for stdin.
        #[arg(long)]
        data: Option<String>,
        /// Query pair `k=v` (repeatable).
        #[arg(long, value_name = "K=V")]
        query: Vec<String>,
        /// The user asked for this specific change.
        #[arg(long)]
        write: bool,
        /// Another pass entry under the prefix.
        #[arg(long)]
        entry: Option<String>,
        /// Follow next-page links.
        #[arg(long)]
        paginate: bool,
        /// Page limit with --paginate.
        #[arg(long, default_value_t = 10)]
        max_pages: usize,
        /// Seconds per request.
        #[arg(long, default_value_t = 30.0)]
        timeout: f64,
    },
}

fn to_exit(code: i32) -> Exit {
    match code {
        0 => Exit::Success,
        1 => Exit::Runtime,
        2 => Exit::Usage,
        _ => Exit::NothingToDo, // 3: refused (exit.rs names code 3 NothingToDo; the repo contract says refused)
    }
}

fn fail(f: &Failed) -> Exit {
    let line = if f.refused {
        format!("REFUSED: {}", f.msg)
    } else {
        format!("error: {}", f.msg)
    };
    repo::err(&redact(&line));
    to_exit(f.code)
}

fn read_body(path: &str) -> Result<Vec<u8>, Failed> {
    if path == "-" {
        let mut v = Vec::new();
        std::io::stdin()
            .read_to_end(&mut v)
            .map_err(|e| Failed::err(format!("stdin: {}", e.kind())))?;
        Ok(v)
    } else {
        std::fs::read(path).map_err(|e| Failed::err(format!("{path}: {}", e.kind())))
    }
}

/// Run `ai-gent repo`.
#[allow(clippy::too_many_lines)]
pub fn run(args: &RepoArgs, json: bool) -> Exit {
    let mut ctx = Ctx::from_process();
    match &args.action {
        RepoAction::Detect => {
            detect::run(&ctx, json);
            Exit::Success
        }
        RepoAction::Chain => {
            let c = chain::chain(&ctx);
            if json {
                repo::out(&chain::to_json(&c));
            } else {
                for l in chain::to_text(&c) {
                    repo::out(&l);
                }
            }
            Exit::Success
        }
        RepoAction::Cli {
            platform,
            write,
            entry,
            args,
        } => match repo_cli::run_cli_stream(&ctx, platform, args, entry.as_deref(), *write) {
            Ok(0) => Exit::Success,
            Ok(code) => std::process::exit(code),
            Err(f) => fail(&f),
        },
        RepoAction::ProjectStatus {
            number,
            owner,
            url,
            field,
            state,
            value,
            write,
            entry,
        } => {
            let r = project_status_call(
                &ctx,
                &Status {
                    number: *number,
                    url,
                    owner: owner.as_deref(),
                    field,
                    state: state.as_deref(),
                    value: value.as_deref(),
                    entry: entry.as_deref(),
                    write: *write,
                },
            );
            match r {
                Ok(j) => {
                    repo::out(&dumps(&j, true, Some(2)));
                    Exit::Success
                }
                Err(f) => fail(&f),
            }
        }
        RepoAction::Batch {
            file,
            write,
            dry_run,
            entry,
            out,
            timeout,
        } => {
            ctx.timeout = *timeout;
            let o = batch::Options {
                file,
                write: *write,
                dry_run: *dry_run,
                entry: entry.as_deref(),
                out: out.as_deref(),
            };
            match batch::run(&ctx, &o) {
                Ok(res) => {
                    for l in &res.stdout {
                        repo::out(l);
                    }
                    if let Some(s) = &res.stderr {
                        repo::note(s);
                    }
                    to_exit(res.code)
                }
                Err(f) => fail(&f),
            }
        }
        RepoAction::Api {
            platform,
            method,
            path,
            data,
            query,
            write,
            entry,
            paginate,
            max_pages,
            timeout,
        } => {
            ctx.timeout = *timeout;
            let body = match data.as_deref().map(read_body).transpose() {
                Ok(b) => b,
                Err(f) => return fail(&f),
            };
            let mut pairs: Vec<(String, String)> = Vec::new();
            for q in query {
                if let Some((k, v)) = q.split_once('=') {
                    if let Some(slot) = pairs.iter_mut().find(|(pk, _)| pk == k) {
                        slot.1 = v.to_string();
                    } else {
                        pairs.push((k.to_string(), v.to_string()));
                    }
                }
            }
            let call = Call {
                platform,
                method,
                path,
                body: body.as_deref(),
                query: pairs,
                entry: entry.as_deref(),
                write: *write,
                paginate: *paginate,
                max_pages: *max_pages,
            };
            match api_call(&ctx, &call) {
                Ok(d) => {
                    if let Some(t) = render(&d) {
                        repo::out(&t);
                    }
                    Exit::Success
                }
                Err(f) if f.http_status.is_some() => {
                    repo::err(&redact(&f.msg));
                    Exit::Runtime
                }
                Err(f) => fail(&f),
            }
        }
    }
}
