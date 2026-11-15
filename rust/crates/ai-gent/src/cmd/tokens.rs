//! `ai-gent tokens`: the port of `plugins/agents/scripts/token_report.py` (spec 035).
//!
//! Same subcommands, flags and output as the Python tool, which stays in place until the 3.0.0
//! cutover (decision D7). Output is plain text, or with `--json` one raw JSON document in the
//! Python key order (`specs/035-token-tools/contracts/cli.md`).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::collapsible_if,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::fn_params_excessive_bools,
    clippy::unnecessary_wraps
)]
use ai_gent_core::exit::Exit;
use ai_gent_core::tokens::attribute::{Group, build_groups, make_record, planned_tags, tasks_key};
use ai_gent_core::tokens::budget::{
    MIN_RATE_MESSAGES, OPUS_PARALLEL_RISK, WaveCard, pack_batches, parse_budget, roles_models,
    usage_messages, usage_rate, wave_cards,
};
use ai_gent_core::tokens::estimate::{
    MIN_SAMPLES, conf_weight, context_factor, estimate_card, event_rate, learn_context,
};
use ai_gent_core::tokens::history::{
    DEFAULT_HISTORY, default_index, history_paths, load_history, load_history_from, read_history,
    rec_id, upsert_history, write_history,
};
use ai_gent_core::tokens::pyjson::{J, dumps, obj};
use ai_gent_core::tokens::report::{
    git_markers, load_snapshots, markers_from_log, render, save_snapshots, split_coordinator,
    stamp_minutes, summary_lines,
};
use ai_gent_core::tokens::specs::{Feature, load_features};
use ai_gent_core::tokens::tags::{format_tags, parse_tags, repo_info};
use ai_gent_core::tokens::transcripts::{
    config_dirs, discover_sessions, family, gather, project_name, project_roots, slug_of,
};
use ai_gent_core::tokens::util::{
    OMap, expanduser, fmt_epoch, fmt_g, fmt_tok, head, k, parse_iso, read_text, round_i,
    system_now, task_ids, to_utc,
};
use ai_gent_core::tokens::writers::{
    ExtraRow, Unit, write_ledger, write_task_lines, write_tracker,
};
use clap::{Args, Subcommand};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const DEFAULT_SNAPSHOTS: &str = "token-snapshots.json";
const DEFAULT_ROLLUP: &str = "docs/product/token-usage.md";
const DEFAULT_LEDGER: &str = ".ai-gent/orchestrate-status.md";
const SEED_PROJECTS: [&str; 5] = [
    "-home-lucas-codes-llm4glpi",
    "-home-lucas-codes-ai-gent",
    "-home-lucas-codes-secrets4claude",
    "-home-lucas-codes-dottod",
    "-home-lucas-codes-rstory",
];

/// Arguments of `tokens`.
#[derive(Debug, Args)]
pub struct TokensArgs {
    /// Fix the clock (tests only): an ISO 8601 timestamp.
    #[arg(long, hide = true, global = true)]
    now: Option<String>,
    /// What to run.
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// The project-wide roll-up (by stage, label, agent).
    Report {
        /// The session directory slug under `~/.claude*/projects/` (pass as `--project=-slug`).
        #[arg(long, allow_hyphen_values = true)]
        project: String,
        /// Restrict to specific sessions (repeatable).
        #[arg(long)]
        session: Vec<String>,
        /// The report file (required unless --summary).
        #[arg(long)]
        out: Option<String>,
        /// Print a few lines instead of one.
        #[arg(long)]
        summary: bool,
        /// Keep only the N costliest labels and agents.
        #[arg(long)]
        top: Option<usize>,
        /// Stage markers file.
        #[arg(long, default_value = DEFAULT_SNAPSHOTS)]
        snapshots: String,
    },
    /// Attribute each subagent to its card and write `used` into the specs.
    Tasks {
        /// The session directory slug.
        #[arg(long, allow_hyphen_values = true)]
        project: String,
        /// Restrict to specific sessions (repeatable).
        #[arg(long)]
        session: Vec<String>,
        /// The specs folder.
        #[arg(long, default_value = "specs")]
        specs_dir: String,
        /// Also write the project-wide report here.
        #[arg(long)]
        rollup_out: Option<String>,
        /// The history file.
        #[arg(long, default_value = DEFAULT_HISTORY)]
        history: String,
        /// The orchestration ledger.
        #[arg(long, default_value = DEFAULT_LEDGER)]
        ledger: String,
        /// Project name for the history (default: derived from --project).
        #[arg(long)]
        name: Option<String>,
        /// Stage markers (default: read from git).
        #[arg(long, default_value = DEFAULT_SNAPSHOTS)]
        snapshots: String,
        /// Skip the deprecated specs/*/tokens.md files and roll-up.
        #[arg(long)]
        no_legacy: bool,
        /// Read everything, write nothing, print the attribution.
        #[arg(long)]
        dry_run: bool,
        /// Print the totals only.
        #[arg(long)]
        summary: bool,
        /// List at most N unattributed agents.
        #[arg(long)]
        top: Option<usize>,
    },
    /// The projection for a planned card, from history or the starting table.
    Estimate {
        /// A role's default model.
        #[arg(long, value_parser = ["builder", "thinker", "scout"], conflicts_with = "model")]
        role: Option<String>,
        /// A model name or family.
        #[arg(long, default_value = "sonnet")]
        model: String,
        /// low, medium or high.
        #[arg(long, value_parser = ["low", "medium", "high"], default_value = "medium")]
        effort: String,
        /// backend, frontend, qa or general.
        #[arg(long)]
        layer: Option<String>,
        /// The card's tags.
        #[arg(long, allow_hyphen_values = true)]
        tags: Option<String>,
        /// How many tasks the card holds.
        #[arg(long)]
        tasks: Option<usize>,
        /// A history file (repeatable).
        #[arg(long)]
        history: Vec<String>,
        /// Deprecated: a card label in the roll-up.
        #[arg(long)]
        label: Option<String>,
        /// Deprecated fallback when there is no history.
        #[arg(long, default_value = DEFAULT_ROLLUP)]
        rollup: String,
    },
    /// Rebuild the cross-project index from the repos' committed histories.
    Index {
        /// A repo dir or history file (repeatable).
        #[arg(long)]
        repo: Vec<String>,
        /// A folder whose subfolders are repos (repeatable).
        #[arg(long)]
        scan: Vec<String>,
        /// The index (default `~/.ai-gent/token-history.jsonl`).
        #[arg(long)]
        index: Option<String>,
    },
    /// Seed the index from past sessions (read-only on sessions and repos).
    Seed {
        /// A project slug (repeatable; default the owner's five).
        #[arg(long, allow_hyphen_values = true)]
        project: Vec<String>,
        /// Where the repos live.
        #[arg(long, default_value = "~/codes")]
        codes_dir: String,
        /// The index.
        #[arg(long)]
        index: Option<String>,
        /// Write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Proceed, shrink or stop: a wave's projections against the budget.
    Budget {
        /// Card ids of the wave, comma separated.
        #[arg(long)]
        cards: Option<String>,
        /// Or projections by hand: `[model:]400k,[model:]300k`.
        #[arg(long, allow_hyphen_values = true)]
        projected: Option<String>,
        /// The specs folder.
        #[arg(long, default_value = "specs")]
        specs_dir: String,
        /// The budget file.
        #[arg(long, default_value = ".ai-gent/budget.yaml")]
        budget: String,
        /// The session slug (default: this directory's).
        #[arg(long, allow_hyphen_values = true)]
        project: Option<String>,
        /// The session whose usage counts (default: the newest).
        #[arg(long)]
        session: Vec<String>,
        /// A history file (repeatable).
        #[arg(long)]
        history: Vec<String>,
    },
    /// A stage marker by hand.
    Snapshot {
        /// The marker's label.
        #[arg(long)]
        label: String,
        /// When (ISO 8601); default now.
        #[arg(long)]
        at: Option<String>,
        /// Stage markers file.
        #[arg(long, default_value = DEFAULT_SNAPSHOTS)]
        snapshots: String,
    },
    /// Stage markers from first-parent landings.
    BackfillGit {
        /// The repo path, or `-` to read `git log` from stdin.
        #[arg(long)]
        repo: String,
        /// The ref.
        #[arg(long = "ref", default_value = "origin/dev")]
        git_ref: String,
        /// Stage markers file.
        #[arg(long, default_value = DEFAULT_SNAPSHOTS)]
        snapshots: String,
    },
}

/// Run `tokens`.
pub fn run(a: &TokensArgs, json_out: bool) -> Exit {
    let now = match a.now.as_deref() {
        Some(s) => match parse_iso(s) {
            Some(e) => e,
            None => return usage(&format!("--now: not an ISO 8601 timestamp: {s}")),
        },
        None => system_now(),
    };
    let mut out = String::new();
    let res = dispatch(&a.action, json_out, now, &mut out);
    print!("{out}");
    match res {
        Ok(()) => Exit::Success,
        Err(Fail::Usage(m)) => usage(&m),
        Err(Fail::Run(m)) => {
            eprintln!("ai-gent tokens: {m}");
            Exit::Runtime
        }
    }
}

fn usage(m: &str) -> Exit {
    eprintln!("ai-gent tokens: error: {m}");
    Exit::Usage
}

enum Fail {
    Usage(String),
    Run(String),
}

impl<E: std::fmt::Display> From<E> for Fail {
    fn from(e: E) -> Self {
        Fail::Run(e.to_string())
    }
}

type R = Result<(), Fail>;

fn dispatch(action: &Action, json_out: bool, now: i64, out: &mut String) -> R {
    match action {
        Action::Report {
            project,
            session,
            out: file,
            summary,
            top,
            snapshots,
        } => report(
            project,
            session,
            file.as_deref(),
            *summary,
            *top,
            snapshots,
            now,
            out,
        ),
        Action::Tasks {
            project,
            session,
            specs_dir,
            rollup_out,
            history,
            ledger,
            name,
            snapshots,
            no_legacy,
            dry_run,
            summary,
            top,
        } => {
            if json_out && !dry_run {
                return Err(Fail::Usage(
                    "tasks --json is read-only and needs --dry-run".into(),
                ));
            }
            let o = TasksOpts {
                project,
                session,
                specs_dir,
                rollup_out: rollup_out.as_deref(),
                history,
                ledger,
                name: name.as_deref(),
                snapshots,
                no_legacy: *no_legacy,
                dry_run: *dry_run,
                summary: *summary,
                top: *top,
                json: json_out,
            };
            tasks(&o, now, out)
        }
        Action::Estimate {
            role,
            model,
            effort,
            layer,
            tags,
            tasks,
            history,
            label,
            rollup,
        } => {
            let o = EstOpts {
                role: role.as_deref(),
                model,
                effort,
                layer: layer.as_deref(),
                tags: tags.as_deref(),
                tasks: *tasks,
                history,
                label: label.as_deref(),
                rollup,
                json: json_out,
            };
            estimate(&o, out)
        }
        Action::Index { repo, scan, index } => index_cmd(repo, scan, index.as_deref(), out),
        Action::Seed {
            project,
            codes_dir,
            index,
            dry_run,
        } => seed(project, codes_dir, index.as_deref(), *dry_run, out),
        Action::Budget {
            cards,
            projected,
            specs_dir,
            budget,
            project,
            session,
            history,
        } => {
            let o = BudgetOpts {
                cards: cards.as_deref().unwrap_or(""),
                projected: projected.as_deref().unwrap_or(""),
                specs_dir,
                budget,
                project: project.as_deref(),
                session: session.first().map(String::as_str),
                history,
                json: json_out,
            };
            budget_cmd(&o, now, out)
        }
        Action::Snapshot {
            label,
            at,
            snapshots,
        } => snapshot(label, at.as_deref(), snapshots, now, out),
        Action::BackfillGit {
            repo,
            git_ref,
            snapshots,
        } => backfill(repo, git_ref, snapshots, out),
    }
}

fn snaps_or_fail(p: &str) -> Result<Vec<J>, Fail> {
    load_snapshots(Path::new(p)).map_err(Fail::Run)
}

// ------------------------------------------------------------------ report

#[allow(clippy::too_many_arguments)]
fn report(
    project: &str,
    session: &[String],
    file: Option<&str>,
    summary: bool,
    top: Option<usize>,
    snapshots: &str,
    now: i64,
    out: &mut String,
) -> R {
    if file.is_none() && !summary {
        return Err(Fail::Usage("report needs --out or --summary".into()));
    }
    let roots = project_roots(project);
    let sessions: Vec<String> = if session.is_empty() {
        discover_sessions(&roots)
    } else {
        session.to_vec()
    };
    let g = gather(&roots, &sessions);
    let snaps = snaps_or_fail(snapshots)?;
    let name = project_name(project);
    let (lines, grand) = render(
        &name,
        &sessions,
        roots.len(),
        &g.coords,
        &g.rows,
        &g.tagged,
        &snaps,
        top,
        &stamp_minutes(now),
    );
    if let Some(f) = file {
        std::fs::write(f, lines.join("\n"))?;
    }
    if summary {
        let l = summary_lines(
            &name,
            &sessions,
            &g.coords,
            &g.rows,
            &g.main_msgs,
            top.unwrap_or(5),
        );
        let _ = writeln!(out, "{}", l.join("\n"));
    } else {
        let _ = writeln!(
            out,
            "wrote {}: {} subagents, {} stage markers, billable {}, total {}",
            file.unwrap_or(""),
            g.rows.len(),
            snaps.len(),
            k(grand.0),
            k(grand.1)
        );
    }
    Ok(())
}

// ------------------------------------------------------------------- tasks

struct TasksOpts<'a> {
    project: &'a str,
    session: &'a [String],
    specs_dir: &'a str,
    rollup_out: Option<&'a str>,
    history: &'a str,
    ledger: &'a str,
    name: Option<&'a str>,
    snapshots: &'a str,
    no_legacy: bool,
    dry_run: bool,
    summary: bool,
    top: Option<usize>,
    json: bool,
}

struct FeatSummary {
    used: i64,
    coordinator: f64,
    cards: usize,
    lines: usize,
}

fn rec_i(r: &J, key: &str) -> Option<i64> {
    r.get(key).and_then(J::as_i64)
}

#[allow(clippy::too_many_lines)]
fn tasks(o: &TasksOpts, now: i64, out: &mut String) -> R {
    let roots = project_roots(o.project);
    let sessions: Vec<String> = if o.session.is_empty() {
        discover_sessions(&roots)
    } else {
        o.session.to_vec()
    };
    let g = gather(&roots, &sessions);
    let mut snaps = snaps_or_fail(o.snapshots)?;
    if snaps.is_empty() {
        snaps = git_markers(None);
    }
    let project = o.name.map_or_else(|| project_name(o.project), String::from);
    let feats = load_features(Path::new(o.specs_dir));
    let repo = std::env::current_dir().ok().map(|d| repo_info(&d));
    let groups = build_groups(&g.rows, &feats);
    let (share, unassigned, coord_total) =
        split_coordinator(&g.main_msgs, &groups, &g.rows, &snaps);

    let rollup: Option<String> = o
        .rollup_out
        .map(String::from)
        .or_else(|| (!o.no_legacy).then(|| DEFAULT_ROLLUP.to_string()));
    if let Some(r) = &rollup {
        if !o.json {
            let (lines, _) = render(
                &project,
                &sessions,
                roots.len(),
                &g.coords,
                &g.rows,
                &g.tagged,
                &snaps,
                None,
                &stamp_minutes(now),
            );
            if !o.dry_run {
                ai_gent_core::tokens::util::ensure_parent(Path::new(r))?;
                std::fs::write(r, lines.join("\n"))?;
            }
        }
    }
    let mut legacy = (0usize, 0usize);
    if !o.no_legacy && !o.dry_run {
        legacy = write_legacy(
            &g.rows,
            Path::new(o.specs_dir),
            rollup.as_deref().unwrap_or(""),
            now,
        )?;
    }

    let mut feat_bill: OMap<i64> = OMap::new();
    for gr in &groups {
        if let Some(f) = &gr.feature {
            *feat_bill.entry(f, || 0) += gr.billable;
        }
    }
    let mut records: Vec<J> = Vec::new();
    let mut by_card: OMap<Unit> = OMap::new();
    let mut summary_map: OMap<FeatSummary> = OMap::new();
    let mut spans: Vec<J> = Vec::new();
    for (fname, feat) in feats.iter() {
        let mine: Vec<&Group> = groups
            .iter()
            .filter(|x| x.feature.as_deref() == Some(fname))
            .collect();
        let mut units: Vec<Unit> = Vec::new();
        let mut extras: Vec<ExtraRow> = Vec::new();
        for (cid, c) in feat.cards.iter() {
            if c.projected.is_some() && !mine.iter().any(|x| x.card.as_deref() == Some(cid)) {
                units.push(Unit {
                    label: Some(format!("`{cid}`")),
                    tasks: c.tasks.clone(),
                    projected: c.projected,
                    used: None,
                    cache_read: 0,
                    tags: c.tags.clone(),
                });
            }
        }
        let fb = feat_bill.get(fname).copied().unwrap_or(0);
        let fshare = share.get(fname).copied().unwrap_or(0.0);
        for gr in &mine {
            let cshare = if fb != 0 {
                fshare * gr.billable as f64 / fb as f64
            } else {
                0.0
            };
            let rec = make_record(
                &project,
                gr,
                &g.rows,
                &feats,
                Some(cshare),
                "measured",
                repo.as_ref(),
            );
            let projected = rec_i(&rec, "projected");
            if let Some(card) = &gr.card {
                spans.push(obj(vec![
                    ("card", J::s(card)),
                    ("feature", J::s(fname)),
                    (
                        "model",
                        rec.get("executor_model").cloned().unwrap_or(J::Null),
                    ),
                    ("projected", J::opt_i(projected)),
                    ("used", J::Int(gr.billable)),
                    ("start", J::s(format!("{}Z", gr.start))),
                    ("end", J::s(format!("{}Z", gr.end))),
                    ("agents", J::Int(gr.rows.len() as i64)),
                    ("calls", J::Int(gr.calls)),
                ]));
            }
            records.push(rec);
            let label = if let Some(c) = &gr.card {
                Some(format!("`{c}`"))
            } else if gr.tasks.is_empty() {
                None
            } else {
                Some(tasks_key(&gr.tasks))
            };
            let Some(label) = label else {
                extras.push(ExtraRow {
                    label: format!("{} (feature-level)", head(&gr.desc, 50)),
                    used: gr.billable,
                    cache_read: gr.cache_read,
                });
                continue;
            };
            let u = Unit {
                label: Some(label),
                tasks: gr.tasks.clone(),
                projected,
                used: Some(gr.billable),
                cache_read: gr.cache_read,
                tags: planned_tags(gr, &feats),
            };
            if let Some(c) = &gr.card {
                by_card.set(c, u.clone());
            }
            units.push(u);
        }
        for u in &units {
            if u.used.is_none() && !u.tasks.is_empty() {
                if let Some(l) = &u.label {
                    by_card.set(l.trim_matches('`'), u.clone());
                }
            }
        }
        let used_units = units.iter().filter(|u| u.used.is_some()).count();
        let changed = write_task_lines(feat, &units, o.dry_run)?;
        if used_units > 0 || !extras.is_empty() {
            write_tracker(feat, &units, &extras, fshare, o.dry_run)?;
        }
        summary_map.set(
            fname,
            FeatSummary {
                used: fb,
                coordinator: fshare,
                cards: used_units,
                lines: changed,
            },
        );
    }
    let unattributed: Vec<&Group> = groups
        .iter()
        .filter(|x| x.confidence == "agent" || x.feature.is_none())
        .collect();
    for gr in &unattributed {
        records.push(make_record(
            &project,
            gr,
            &g.rows,
            &feats,
            None,
            "measured",
            repo.as_ref(),
        ));
    }
    let n;
    if o.dry_run {
        n = records.len();
    } else {
        n = upsert_history(Path::new(o.history), records)?;
        write_ledger(Path::new(o.ledger), &by_card, false)?;
    }

    if o.json {
        let doc = obj(vec![
            ("cards", J::Arr(spans)),
            ("unattributed", J::Int(unattributed.len() as i64)),
        ]);
        let _ = writeln!(out, "{}", dumps(&doc, true, None));
        return Ok(());
    }
    let _ = writeln!(
        out,
        "{project}: {} subagent(s) in {} session(s); {}",
        g.rows.len(),
        sessions.len(),
        if o.dry_run {
            "dry run, nothing written".to_string()
        } else {
            format!("wrote {} ({n} records)", o.history)
        }
    );
    if !o.summary {
        let _ = writeln!(
            out,
            "feature | used (subagents) | coordinator share | total | cards | task lines"
        );
        let mut names: Vec<&String> = summary_map.keys().collect();
        names.sort();
        for name in names {
            let Some(s) = summary_map.get(name) else {
                continue;
            };
            if s.used != 0 || s.cards != 0 {
                let c = round_i(s.coordinator);
                let _ = writeln!(
                    out,
                    "{name} | {} | {} | {} | {} | {}",
                    fmt_tok(Some(s.used)),
                    fmt_tok(Some(c)),
                    fmt_tok(Some(s.used + c)),
                    s.cards,
                    s.lines
                );
            }
        }
    }
    if !o.no_legacy && !o.summary {
        let _ = writeln!(
            out,
            "deprecated legacy output: {} specs/*/tokens.md, {} unmapped card(s) in {} (--no-legacy turns it off; both go in a later major)",
            legacy.0,
            legacy.1,
            rollup.as_deref().unwrap_or("")
        );
    }
    let un: i64 = unattributed.iter().map(|x| x.billable).sum();
    let _ = writeln!(
        out,
        "unattributed (reported, not guessed): {} agent(s), {}",
        unattributed.len(),
        fmt_tok(Some(un))
    );
    let mut ranked = unattributed.clone();
    ranked.sort_by_key(|x| std::cmp::Reverse(x.billable));
    let shown: Vec<&&Group> = ranked.iter().take(o.top.unwrap_or(usize::MAX)).collect();
    for gr in &shown {
        let _ = writeln!(
            out,
            "  {:>7}  {:<6} {}",
            fmt_tok(Some(gr.billable)),
            gr.model,
            head(&gr.desc, 80)
        );
    }
    if shown.len() < unattributed.len() {
        let _ = writeln!(
            out,
            "  … {} more not shown (--top {})",
            unattributed.len() - shown.len(),
            o.top.unwrap_or(0)
        );
    }
    let _ = writeln!(
        out,
        "coordinator: {} billable in all, {} in windows with no attributed subagents",
        fmt_tok(Some(coord_total)),
        fmt_tok(Some(unassigned))
    );
    let agent_total: i64 = g.rows.iter().map(|r| r.s.billable).sum();
    let _ = writeln!(
        out,
        "total: {} billable ({} subagents)",
        fmt_tok(Some(agent_total + coord_total)),
        fmt_tok(Some(agent_total))
    );
    Ok(())
}

/// The deprecated `specs/<feature>/tokens.md` files and the Unmapped section of the roll-up.
fn write_legacy(
    rows: &[ai_gent_core::tokens::transcripts::Row],
    specs_dir: &Path,
    rollup: &str,
    now: i64,
) -> Result<(usize, usize), Fail> {
    let mut feature_of: OMap<String> = OMap::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(specs_dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    dirs.sort();
    for d in dirs {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let b = name.as_bytes();
        if !(b.len() >= 4 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b'-') {
            continue;
        }
        let tm = d.join("tasks.md");
        if !tm.is_file() {
            continue;
        }
        let ids: std::collections::BTreeSet<String> =
            task_ids(&read_text(&tm)?).into_iter().collect();
        for t in ids {
            feature_of.set(&t, name.clone());
        }
    }
    let mut per_feature: OMap<Vec<usize>> = OMap::new();
    let mut unmapped: Vec<usize> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let mut features: Vec<String> = task_ids(&r.desc)
            .iter()
            .filter_map(|t| feature_of.get(t).cloned())
            .collect();
        features.sort();
        features.dedup();
        if let Some(f) = features.first() {
            per_feature.entry(f, Vec::new).push(i);
        } else {
            unmapped.push(i);
        }
    }
    let mut names: Vec<&String> = per_feature.keys().collect();
    names.sort();
    let stamp = stamp_minutes(now);
    for name in &names {
        let Some(idx) = per_feature.get(name) else {
            continue;
        };
        let mut fr: Vec<&ai_gent_core::tokens::transcripts::Row> =
            idx.iter().map(|&i| &rows[i]).collect();
        fr.sort_by(|a, b| a.s.start.cmp(&b.s.start));
        let mut lines: Vec<String> = vec![
            format!("# Token usage — {name}"),
            String::new(),
            format!(
                "Generated {stamp}. Deprecated output of `token_report.py tasks` (use the `Tokens:` tags on tasks.md, tracker.md and docs/product/token-history.jsonl); turn it off with `--no-legacy`. Linked to `tasks.md` by task id; a record file, never written into it. Cards spanning several features attribute their full cost to the first feature their task ids name."
            ),
            String::new(),
            "| Task(s) | Who | Model(s) | Calls | Billable | Total |".into(),
            "|---|---|---|---|---|---|".into(),
        ];
        for r in &fr {
            let ids = task_ids(&r.desc);
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} |",
                if ids.is_empty() {
                    "—".to_string()
                } else {
                    ids.join(", ")
                },
                r.who,
                r.s.models,
                r.s.calls,
                k(r.s.billable),
                k(r.s.total)
            ));
        }
        lines.push(format!(
            "| **Feature total** | | | | **{}** | **{}** |",
            k(fr.iter().map(|r| r.s.billable).sum()),
            k(fr.iter().map(|r| r.s.total).sum())
        ));
        lines.push(String::new());
        std::fs::write(specs_dir.join(name).join("tokens.md"), lines.join("\n"))?;
    }
    if !unmapped.is_empty() {
        let mut text = String::from(
            "\n## Unmapped\n\nCards whose description names no task id, so their cost isn't linked to any feature's `tasks.md`:\n\n| Who | Task | Billable | Total |\n|---|---|---|---|\n",
        );
        for &i in &unmapped {
            let r = &rows[i];
            let _ = writeln!(
                text,
                "| {} | {} | {} | {} |",
                r.who,
                r.desc,
                k(r.s.billable),
                k(r.s.total)
            );
        }
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(rollup)?;
        std::io::Write::write_all(&mut f, text.as_bytes())?;
    }
    Ok((per_feature.len(), unmapped.len()))
}

// ---------------------------------------------------------------- estimate

struct EstOpts<'a> {
    role: Option<&'a str>,
    model: &'a str,
    effort: &'a str,
    layer: Option<&'a str>,
    tags: Option<&'a str>,
    tasks: Option<usize>,
    history: &'a [String],
    label: Option<&'a str>,
    rollup: &'a str,
    json: bool,
}

fn role_model(role: &str) -> &'static str {
    match role {
        "thinker" => "opus",
        "scout" => "haiku",
        _ => "sonnet",
    }
}

fn parse_k(s: &str) -> f64 {
    let t = s.trim().replace([',', '*'], "");
    if let Some(m) = t.strip_suffix('M') {
        return m.parse::<f64>().unwrap_or(0.0) * 1_000_000.0;
    }
    if let Some(m) = t.strip_suffix('k') {
        return m.parse::<f64>().unwrap_or(0.0) * 1_000.0;
    }
    t.parse::<f64>().unwrap_or(0.0)
}

/// Rows `(label, model, billable, total)` of a roll-up's `## By agent` table.
fn read_by_agent(path: &str) -> Vec<(String, String, f64, f64)> {
    let Ok(text) = read_text(Path::new(path)) else {
        return vec![];
    };
    let Some(start) = text.find("## By agent\n\n") else {
        return vec![];
    };
    let rest = &text[start + "## By agent\n\n".len()..];
    let body = rest.find("\n## ").map_or(rest, |e| &rest[..e]);
    body.lines()
        .filter(|l| l.starts_with('|'))
        .skip(2)
        .filter_map(|ln| {
            let cells: Vec<&str> = ln.trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() >= 8).then(|| {
                (
                    cells[2].to_string(),
                    cells[4].to_string(),
                    parse_k(cells[6]),
                    parse_k(cells[7]),
                )
            })
        })
        .collect()
}

fn legacy_estimate(o: &EstOpts, fam: &str) -> Option<String> {
    let rows = read_by_agent(o.rollup);
    let sel: Vec<&(String, String, f64, f64)> = match o.label {
        Some(l) => rows.iter().filter(|r| r.0 == l).collect(),
        None => rows.iter().filter(|r| r.1.contains(fam)).collect(),
    };
    if sel.is_empty() {
        return None;
    }
    let how = o
        .label
        .map_or_else(|| format!("model '{fam}'"), |l| format!("label '{l}'"));
    let n = sel.len() as f64;
    let b = sel.iter().map(|r| r.2).sum::<f64>() / n;
    let t = sel.iter().map(|r| r.3).sum::<f64>() / n;
    Some(format!(
        "{how}: {} sample(s) in {}, average billable {}, average total {} (deprecated roll-up fallback)",
        sel.len(),
        o.rollup,
        k(b as i64),
        k(t as i64)
    ))
}

fn estimate(o: &EstOpts, out: &mut String) -> R {
    let fam = o
        .role
        .map_or_else(|| family(o.model), |r| role_model(r).to_string());
    let tags = o.tags.map(parse_tags);
    let hist = load_history(&history_paths(o.history));
    let est = estimate_card(
        &hist,
        &fam,
        Some(o.effort),
        o.layer,
        o.tasks,
        tags.as_ref(),
        Some(o.role.unwrap_or("builder")),
    );
    if o.label.is_some() || (est.basis == "starting table" && !o.json) {
        if let Some(old) = legacy_estimate(o, &fam) {
            let _ = writeln!(out, "{old}");
            return Ok(());
        }
        if let Some(l) = o.label {
            let _ = writeln!(
                out,
                "no history for label '{l}' in {} — fall back to S/M/L sizing",
                o.rollup
            );
            return Ok(());
        }
    }
    let mut label = format!("{fam} · effort {}", o.effort);
    if let Some(l) = o.layer.filter(|l| !l.is_empty()) {
        let _ = write!(label, " · layer {l}");
    }
    if let Some(t) = o.tasks.filter(|t| *t != 0) {
        let _ = write!(label, " · {t} task(s)");
    }
    if let Some(t) = tags.as_ref().filter(|t| !t.is_empty()) {
        let _ = write!(label, " · tags {}", format_tags(t));
    }
    if o.json {
        let mut doc = vec![("query".to_string(), J::s(&label))];
        doc.extend(est.to_j());
        let _ = writeln!(out, "{}", dumps(&J::Obj(doc), true, None));
        return Ok(());
    }
    let med = fmt_tok(Some(round_i(est.median)));
    let _ = writeln!(out, "estimate for {label}");
    let _ = writeln!(
        out,
        "  projection ~{med} billable (median), upper bound ~{} (p75)",
        fmt_tok(Some(round_i(est.p75)))
    );
    let mut mix = est.mix.clone();
    mix.sort_by(|a, b| conf_weight(&b.0).total_cmp(&conf_weight(&a.0)));
    let mix = mix
        .iter()
        .map(|(c, n)| format!("{n} {c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let tier = est.tier.clone().unwrap_or_else(|| "None".into());
    match est.basis {
        "tags" => {
            let _ = writeln!(
                out,
                "  basis: {} record(s) at match level {} ({tier}), confidence {} ({mix})",
                est.n,
                est.level,
                est.confidence_label.unwrap_or("None")
            );
            let _ = writeln!(
                out,
                "  context: x{:.2} for {fam}/{} ({}); bias correction x{:.2}{}",
                est.context.0,
                o.role.unwrap_or("builder"),
                est.context.1,
                est.bias.0,
                if est.bias.1.is_empty() {
                    " (fewer than 5 records per tag)".to_string()
                } else {
                    format!(" ({})", est.bias.1.join("; "))
                }
            );
        }
        "history" => {
            let _ = writeln!(
                out,
                "  basis: {} similar record(s) matching {tier} ({mix}); exact weighs 1.0, feature-level {}, agent-only {}{}",
                est.n,
                fmt_py_float(conf_weight("feature")),
                fmt_py_float(conf_weight("agent")),
                if tags.as_ref().is_some_and(|t| !t.is_empty()) {
                    "; tags matched too few records, so the match fell back to effort and model"
                } else {
                    ""
                }
            );
        }
        _ => {
            let _ = writeln!(
                out,
                "  basis: the starting table (only {} similar record(s); {MIN_SAMPLES} needed for history to take over)",
                est.n
            );
        }
    }
    let _ = writeln!(out, "  on the card: Tokens: projected ~{med} | used —");
    Ok(())
}

fn fmt_py_float(f: f64) -> String {
    ai_gent_core::tokens::pyjson::float_repr(f)
}

// ------------------------------------------------------------- index, seed

fn index_cmd(repo: &[String], scan: &[String], index: Option<&str>, out: &mut String) -> R {
    let index = index.map_or_else(default_index, PathBuf::from);
    let mut paths: Vec<String> = repo.to_vec();
    for d in scan {
        let base = PathBuf::from(expanduser(d));
        let mut found: Vec<String> = ai_gent_core::tokens::util::glob_children(&base)
            .into_iter()
            .map(|p| p.join(DEFAULT_HISTORY))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        found.sort();
        paths.extend(found);
    }
    if Path::new(DEFAULT_HISTORY).exists() {
        paths.push(DEFAULT_HISTORY.to_string());
    }
    let paths: Vec<String> = paths
        .into_iter()
        .map(|p| {
            if Path::new(&p).is_dir() {
                Path::new(&p)
                    .join(DEFAULT_HISTORY)
                    .to_string_lossy()
                    .into_owned()
            } else {
                p
            }
        })
        .collect();
    let mut repo_recs: OMap<J> = OMap::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for p in &paths {
        let abs = std::path::absolute(p).unwrap_or_else(|_| PathBuf::from(p));
        if seen.contains(&abs) {
            continue;
        }
        seen.push(abs.clone());
        for r in read_history(&abs) {
            let id = rec_id(&r).to_string();
            repo_recs.set(&id, r);
        }
    }
    let seeds: Vec<J> = read_history(&index)
        .into_iter()
        .filter(|r| r.get("source").and_then(J::as_str) == Some("seed"))
        .collect();
    let mut all: Vec<J> = repo_recs.values().cloned().collect();
    all.extend(seeds.iter().cloned());
    let merged = load_history_from(all);
    write_history(&index, &merged)?;
    let mut counts: Vec<((String, String), i64)> = Vec::new();
    for r in &merged {
        let key = (py_repr(r.get("project")), py_repr(r.get("source")));
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some(e) => e.1 += 1,
            None => counts.push((key, 1)),
        }
    }
    let uniq: std::collections::BTreeSet<&String> = paths.iter().collect();
    let _ = writeln!(
        out,
        "wrote {}: {} record(s) from {} repo history file(s) and {} seeded",
        index.display(),
        merged.len(),
        uniq.len(),
        seeds.len()
    );
    counts.sort_by_key(|((p, s), n)| format!("(({p}, {s}), {n})"));
    for ((p, s), n) in counts {
        let _ = writeln!(
            out,
            "  {}: {n} {}",
            p.trim_matches('\''),
            s.trim_matches('\'')
        );
    }
    Ok(())
}

/// `repr()` of a JSON string or null, enough for sorting and printing project names.
fn py_repr(v: Option<&J>) -> String {
    match v {
        Some(J::Str(s)) => format!("'{s}'"),
        _ => "None".into(),
    }
}

fn seed(
    projects: &[String],
    codes_dir: &str,
    index: Option<&str>,
    dry: bool,
    out: &mut String,
) -> R {
    let index = index.map_or_else(default_index, PathBuf::from);
    let projects: Vec<String> = if projects.is_empty() {
        SEED_PROJECTS.iter().map(|s| (*s).to_string()).collect()
    } else {
        projects.to_vec()
    };
    let mut new: Vec<J> = Vec::new();
    let mut report: OMap<(usize, usize, usize, [i64; 3], i64)> = OMap::new();
    for slug in &projects {
        let name = project_name(slug);
        let roots = project_roots(slug);
        let sessions = discover_sessions(&roots);
        let g = gather(&roots, &sessions);
        let repo_dir = PathBuf::from(expanduser(codes_dir)).join(&name);
        let feats: OMap<Feature> = load_features(&repo_dir.join("specs"));
        let repo = repo_info(&repo_dir);
        let groups = build_groups(&g.rows, &feats);
        let recs: Vec<J> = groups
            .iter()
            .filter(|x| x.billable > 0)
            .map(|x| make_record(&name, x, &g.rows, &feats, None, "seed", Some(&repo)))
            .collect();
        let mut c = [0i64; 3];
        let mut used = 0;
        for r in &recs {
            match r.get("confidence").and_then(J::as_str) {
                Some("exact") => c[0] += 1,
                Some("feature") => c[1] += 1,
                _ => c[2] += 1,
            }
            used += rec_i(r, "used").unwrap_or(0);
        }
        report.set(&name, (roots.len(), sessions.len(), g.rows.len(), c, used));
        new.extend(recs);
    }
    if !dry {
        let mine: std::collections::HashSet<String> =
            projects.iter().map(|s| project_name(s)).collect();
        let mut keep: Vec<J> = read_history(&index)
            .into_iter()
            .filter(|r| {
                !(r.get("source").and_then(J::as_str) == Some("seed")
                    && r.get("project")
                        .and_then(J::as_str)
                        .is_some_and(|p| mine.contains(p)))
            })
            .collect();
        keep.extend(new.iter().cloned());
        write_history(&index, &keep)?;
    }
    let _ = writeln!(
        out,
        "{}seeded {} record(s) into {} (sessions and specs read-only)",
        if dry { "dry run: " } else { "" },
        new.len(),
        index.display()
    );
    let _ = writeln!(
        out,
        "project | config dirs | sessions | subagents | records by confidence | billable"
    );
    for (name, (dirs, sess, agents, c, bill)) in report.iter() {
        let _ = writeln!(
            out,
            "{name} | {dirs} | {sess} | {agents} | {} exact, {} feature, {} agent | {}",
            c[0],
            c[1],
            c[2],
            fmt_tok(Some(*bill))
        );
    }
    Ok(())
}

// ------------------------------------------------------------------ budget

struct BudgetOpts<'a> {
    cards: &'a str,
    projected: &'a str,
    specs_dir: &'a str,
    budget: &'a str,
    project: Option<&'a str>,
    session: Option<&'a str>,
    history: &'a [String],
    json: bool,
}

fn newest_session(roots: &[PathBuf]) -> Option<String> {
    let mut files: Vec<PathBuf> = Vec::new();
    for r in roots {
        files.extend(
            ai_gent_core::tokens::util::glob_children(r)
                .into_iter()
                .filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.is_file()),
        );
    }
    let mtime = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut best: Option<&PathBuf> = None;
    for f in &files {
        if best.is_none_or(|b| mtime(f) > mtime(b)) {
            best = Some(f);
        }
    }
    best.and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
}

#[allow(clippy::too_many_lines)]
fn budget_cmd(o: &BudgetOpts, now: i64, out: &mut String) -> R {
    let cfg = parse_budget(Path::new(o.budget));
    let c = |key: &str| cfg.get(key).copied();
    let slug = o.project.map_or_else(
        || {
            slug_of(
                &std::env::current_dir()
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        },
        String::from,
    );
    let dirs = config_dirs();
    let mut roots: Vec<PathBuf> = dirs
        .iter()
        .map(|d| d.join("projects").join(&slug))
        .filter(|p| p.exists())
        .collect();
    roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    let sid: Option<String> = o.session.map(String::from).or_else(|| {
        if roots.is_empty() {
            None
        } else {
            newest_session(&roots)
        }
    });
    let week_msgs = usage_messages(&fmt_epoch(now - 7 * 86_400), &dirs);
    let (used_session, scope) = if let Some(h) = c("session_window_hours").filter(|h| *h != 0) {
        let since = fmt_epoch(now - h * 3600);
        (
            week_msgs
                .iter()
                .filter(|m| m.ts.as_str() >= since.as_str())
                .map(|m| m.usage.bill())
                .sum::<i64>(),
            format!("the last {}h, all projects", fmt_g(h as f64)),
        )
    } else if let Some(s) = &sid {
        let g = gather(&roots, std::slice::from_ref(s));
        (
            g.coords
                .iter()
                .chain(g.rows.iter())
                .map(|r| r.s.billable)
                .sum(),
            format!("session {}", head(s, 8)),
        )
    } else {
        (0, "no session found".to_string())
    };
    let used_week: i64 = week_msgs.iter().map(|m| m.usage.bill()).sum();
    let window = c("rate_window_minutes").unwrap_or(30).max(1);
    let (per_hour, n_msgs) = usage_rate(&week_msgs, now, window);
    let have_rate = n_msgs == 0 || n_msgs >= MIN_RATE_MESSAGES;
    let eta = |left: i64| -> Option<i64> {
        (have_rate && per_hour != 0 && n_msgs >= MIN_RATE_MESSAGES)
            .then(|| round_i(left as f64 / per_hour as f64 * 60.0))
    };
    let feats = load_features(Path::new(o.specs_dir));
    let history = load_history(&history_paths(o.history));
    let roles = roles_models(Path::new(".ai-gent/orchestrate.yaml"));
    let cards = wave_cards(o.projected, o.cards, &feats, &history, &roles);
    if cards.is_empty() {
        return Err(Fail::Run(
            "budget: name the wave's cards with --cards ID,ID or --projected [model:]400k,..."
                .into(),
        ));
    }
    let overhead = c("coordinator_overhead_pct").unwrap_or(35) as f64 / 100.0;
    let reserve = c("reserve_pct").unwrap_or(10) as f64 / 100.0;
    let max_agents = usize::try_from(c("max_agents").unwrap_or(2)).unwrap_or(2);
    let max_opus = usize::try_from(c("max_opus").unwrap_or(1)).unwrap_or(1);
    let (rate, rate_src) = event_rate(&history);
    let n_opus = cards.iter().filter(|x| x.model == "opus").count();
    let extra_opus = n_opus.min(max_opus).saturating_sub(1);
    let risk = rate + OPUS_PARALLEL_RISK * extra_opus as f64;
    let factor = (1.0 + overhead) * (1.0 + risk);
    let mut caps: Vec<(&str, i64, i64)> = Vec::new();
    for (name, key, used) in [
        ("session", "session_billable", used_session),
        ("week", "week_billable", used_week),
    ] {
        if let Some(cap) = c(key) {
            caps.push((name, cap, used));
        }
    }
    let room = caps.iter().map(|(_, cap, used)| cap - used).min();
    let avail = room.map(|r| r.max(0) as f64 * (1.0 - reserve));

    let mut say = String::new();
    let _ = writeln!(say, "wave: {} card(s)", cards.len());
    for cd in &cards {
        let up = cd
            .p75
            .filter(|p| *p != 0)
            .map_or_else(String::new, |p| format!(", p75 ~{}", fmt_tok(Some(p))));
        let cf = cd
            .conf
            .as_deref()
            .map_or_else(String::new, |x| format!(", confidence {x}"));
        let _ = writeln!(
            say,
            "  {:<24} {:<7} ~{:>6}  ({}{up}{cf})",
            cd.label,
            cd.model,
            fmt_tok(Some(cd.projected)),
            cd.how
        );
    }
    let total: i64 = cards.iter().map(|x| x.projected).sum();
    let need = total as f64 * factor;
    let _ = writeln!(
        say,
        "projected {} + coordinator {}% + event risk {:.0}% ({rate_src}: resumes, fix rounds, flake hunts{}) = {} billable",
        fmt_tok(Some(total)),
        fmt_g(c("coordinator_overhead_pct").unwrap_or(35) as f64),
        risk * 100.0,
        if extra_opus > 0 {
            format!(
                "; +{:.0}% for {} Opus in parallel",
                OPUS_PARALLEL_RISK * extra_opus as f64 * 100.0,
                extra_opus + 1
            )
        } else {
            String::new()
        },
        fmt_tok(Some(round_i(need)))
    );
    for (name, cap, used) in &caps {
        let _ = writeln!(
            say,
            "{name}: cap {}, used {} ({}), left {}",
            fmt_tok(Some(*cap)),
            fmt_tok(Some(*used)),
            if *name == "session" {
                scope.as_str()
            } else {
                "last 7 days, all projects"
            },
            fmt_tok(Some((cap - used).max(0)))
        );
    }
    if caps.is_empty() {
        let _ = writeln!(
            say,
            "no caps in {}: set session_billable and week_billable to enable the check",
            o.budget
        );
    }
    if !have_rate {
        let _ = writeln!(
            say,
            "rate: too few messages ({n_msgs}) in the last {} min",
            fmt_g(window as f64)
        );
    } else if n_msgs == 0 {
        let _ = writeln!(say, "rate: idle for {} min", fmt_g(window as f64));
    } else {
        let _ = writeln!(
            say,
            "rate: {}/h over the last {} min ({n_msgs} messages, all projects)",
            fmt_tok(Some(per_hour)),
            fmt_g(window as f64)
        );
    }
    let mut fit = cards.len();
    let (verdict, why);
    match avail {
        None => {
            verdict = "proceed";
            why = "no caps set".to_string();
        }
        Some(av) if need <= av => {
            verdict = "proceed";
            why = format!(
                "within the {} left after a {}% reserve",
                fmt_tok(Some(round_i(av))),
                fmt_g(c("reserve_pct").unwrap_or(10) as f64)
            );
        }
        Some(av) => {
            fit = 0;
            let mut acc = 0.0;
            for cd in &cards {
                if (acc + cd.projected as f64) * factor > av {
                    break;
                }
                acc += cd.projected as f64;
                fit += 1;
            }
            verdict = if fit > 0 { "shrink" } else { "stop" };
            why = if fit > 0 {
                format!(
                    "only {} usable; the first {fit} card(s) fit ({})",
                    fmt_tok(Some(round_i(av))),
                    fmt_tok(Some(round_i(acc * factor)))
                )
            } else {
                format!(
                    "{} usable is less than the first card needs",
                    fmt_tok(Some(round_i(av)))
                )
            };
        }
    }
    let _ = writeln!(say, "verdict: {verdict} - {why}");
    let batches: Vec<Vec<&WaveCard>> = if fit > 0 {
        pack_batches(&cards[..fit], max_agents, max_opus)
    } else {
        vec![]
    };
    if fit > 0 {
        let _ = writeln!(
            say,
            "dispatch (at most {max_agents} subagent(s) at once, {max_opus} on Opus):"
        );
        for (i, b) in batches.iter().enumerate() {
            let _ = writeln!(
                say,
                "  batch {}: {}",
                i + 1,
                b.iter()
                    .map(|x| format!("{} ({})", x.label, x.model))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    let learned = learn_context(&history);
    for cd in &cards {
        if cd.model == "opus"
            && cd.role != "reviewer"
            && matches!(cd.conf.as_deref(), Some("high" | "medium"))
            && cd.projected >= 400_000
        {
            let f_opus = context_factor(&learned, "opus", &cd.role).0;
            let _ = writeln!(
                say,
                "  suggest: {} is expensive on Opus at confidence {}; a Sonnet builder with the thinker reviewing inline would cost about ~{}",
                cd.label,
                cd.conf.as_deref().unwrap_or(""),
                fmt_tok(Some(round_i(cd.projected as f64 / f_opus)))
            );
        }
    }
    let _ = writeln!(
        say,
        "Prefer Sonnet builders, review inline unless the card needs a fresh context."
    );
    if o.json {
        let rate_j = if have_rate {
            obj(vec![
                ("per_hour", J::Int(per_hour)),
                ("window_minutes", J::Int(window)),
                ("messages", J::Int(n_msgs)),
            ])
        } else {
            J::Null
        };
        let doc = obj(vec![
            ("verdict", J::s(verdict)),
            ("why", J::s(&why)),
            ("fit", J::Int(fit as i64)),
            ("total", J::Int(total)),
            ("need", J::Int(round_i(need))),
            ("rate", rate_j),
            (
                "caps",
                J::Arr(
                    caps.iter()
                        .map(|(n, cap, used)| {
                            obj(vec![
                                ("name", J::s(*n)),
                                ("cap", J::Int(*cap)),
                                ("used", J::Int(*used)),
                                ("left", J::Int((cap - used).max(0))),
                                ("eta_minutes", J::opt_i(eta((cap - used).max(0)))),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "cards",
                J::Arr(
                    cards
                        .iter()
                        .map(|x| {
                            obj(vec![
                                ("label", J::s(&x.label)),
                                ("model", J::s(&x.model)),
                                ("projected", J::Int(x.projected)),
                                ("p75", J::opt_i(x.p75)),
                                ("confidence", J::opt_s(x.conf.as_deref())),
                                ("how", J::s(&x.how)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "batches",
                J::Arr(
                    batches
                        .iter()
                        .map(|b| J::Arr(b.iter().map(|x| J::s(&x.label)).collect()))
                        .collect(),
                ),
            ),
        ]);
        let _ = writeln!(out, "{}", dumps(&doc, true, None));
    } else {
        out.push_str(&say);
    }
    Ok(())
}

// ------------------------------------------------------ snapshot, backfill

fn snapshot(label: &str, at: Option<&str>, path: &str, now: i64, out: &mut String) -> R {
    let when = match at {
        Some(a) => {
            to_utc(a).ok_or_else(|| Fail::Run(format!("--at: not an ISO 8601 timestamp: {a}")))?
        }
        None => fmt_epoch(now),
    };
    let mut snaps = snaps_or_fail(path)?;
    snaps.push(obj(vec![
        ("at", J::s(&when)),
        ("label", J::s(label)),
        ("source", J::s("manual")),
    ]));
    let saved = save_snapshots(Path::new(path), snaps).map_err(Fail::Run)?;
    let _ = writeln!(out, "snapshot '{label}' at {when} ({} total)", saved.len());
    Ok(())
}

fn backfill(repo: &str, git_ref: &str, path: &str, out: &mut String) -> R {
    let log = if repo == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
        s
    } else {
        let o = std::process::Command::new("git")
            .args([
                "-C",
                repo,
                "log",
                "--first-parent",
                "--reverse",
                "--format=%cI|%s",
                git_ref,
            ])
            .output()?;
        if !o.status.success() {
            return Err(Fail::Run(format!("git log {git_ref} failed in {repo}")));
        }
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let new = markers_from_log(&log);
    let mut snaps = snaps_or_fail(path)?;
    snaps.extend(new.iter().cloned());
    let saved = save_snapshots(Path::new(path), snaps).map_err(Fail::Run)?;
    let _ = writeln!(
        out,
        "backfilled {} markers from {git_ref} ({} total)",
        new.len(),
        saved.len()
    );
    Ok(())
}
