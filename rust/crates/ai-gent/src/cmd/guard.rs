//! `ai-gent guard`: the guard evaluator (spec 033).
use crate::cli::{GuardAction, GuardArgs};
use ai_gent_core::exit::Exit;
use ai_gent_core::guard::branch::git_branch;
use ai_gent_core::guard::shadow;
use ai_gent_core::guard::{
    Ctx, Decision, Event, GuardId, MAX_COMMAND_BYTES, Shell, evaluate, evaluate_all,
    evaluate_opencode, reasons,
};
use ai_gent_core::version::VersionInfo;
use serde_json::{Value, json};
use std::io::Read;

/// Run `guard`. Evaluation always exits 0: a failure becomes an `ask` line (FR-014).
pub fn run(a: &GuardArgs, json_out: bool) -> Exit {
    match &a.action {
        Some(GuardAction::ListReasons) => return list_reasons(json_out),
        Some(GuardAction::ShadowReport { since, guard }) => {
            return shadow_report(since.as_deref(), guard.as_deref(), json_out);
        }
        None => {}
    }
    let mut input = String::new();
    let read = std::io::stdin().read_to_string(&mut input);
    let marker = GuardId::parse(&a.guard).map_or("ai-gent guard:", |g| marker_of(g));
    if a.shadow {
        // Never writes to stdout and never fails the hook: a problem is only logged to stderr.
        if let Err(why) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| shadow(a, &input)))
                .unwrap_or_else(|_| Err("internal error".into()))
        {
            eprintln!("ai-gent guard --shadow: {why}");
        }
        return Exit::Success;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &read {
        Ok(_) => evaluate_input(a, &input),
        Err(e) => Err(format!("cannot read stdin: {e}")),
    }));
    let out = match result {
        Ok(Ok(line)) => line,
        Ok(Err(why)) => Some(failure(a, marker, &why)),
        Err(_) => Some(failure(a, marker, "internal error")),
    };
    if let Some(line) = out {
        println!("{line}");
    }
    Exit::Success
}

fn marker_of(g: GuardId) -> &'static str {
    reasons::all()
        .iter()
        .find(|r| r.guard == g.name())
        .map_or("ai-gent guard:", |r| r.marker.as_str())
}

fn failure(a: &GuardArgs, marker: &str, why: &str) -> String {
    let reason = format!(
        "{marker} guard failed internally ({why}); allow only if you accept this command unguarded."
    );
    if a.format == "opencode" {
        json!({"decision": "ask", "reason": reason}).to_string()
    } else {
        Decision {
            level: ai_gent_core::guard::Level::Ask,
            reason,
            rule: "",
        }
        .hook_line()
        .unwrap_or_default()
    }
}

fn evaluate_input(a: &GuardArgs, input: &str) -> Result<Option<String>, String> {
    let guard = if a.guard == "all" {
        None
    } else {
        Some(GuardId::parse(&a.guard).ok_or_else(|| format!("unknown guard {}", a.guard))?)
    };
    let flag_shell = match &a.shell {
        Some(s) => Some(Shell::parse(s).ok_or_else(|| format!("unknown shell {s}"))?),
        None => None,
    };
    let branch = |d: &str| git_branch(d);
    let ctx = Ctx { branch_of: &branch };
    if a.format == "opencode" {
        let v: Value =
            serde_json::from_str(input).map_err(|e| format!("invalid event JSON: {e}"))?;
        let action = v["action"].as_str().unwrap_or("");
        let resources: Vec<String> = v["resources"]
            .as_array()
            .map(|r| {
                r.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if resources.iter().any(|r| r.len() > MAX_COMMAND_BYTES) {
            return Err("command too long".into());
        }
        let d = evaluate_opencode(
            action,
            &resources,
            v["cwd"].as_str().unwrap_or(""),
            flag_shell.unwrap_or(Shell::Sh),
            &ctx,
        );
        return Ok(Some(
            json!({"decision": d.level.name(), "reason": (!d.reason.is_empty()).then_some(d.reason)}).to_string(),
        ));
    }
    if a.format != "claude" {
        return Err(format!("unknown format {}", a.format));
    }
    let ev = Event::from_json(input)?;
    if ev.command().is_some_and(|c| c.len() > MAX_COMMAND_BYTES) {
        return Err("command too long".into());
    }
    let shell = flag_shell.unwrap_or(if ev.tool_name == "PowerShell" {
        Shell::PowerShell
    } else {
        Shell::Sh
    });
    let d = guard.map_or_else(
        || evaluate_all(&ev, shell, &ctx),
        |g| evaluate(g, &ev, shell, &ctx),
    );
    Ok(d.hook_line())
}

fn list_reasons(json_out: bool) -> Exit {
    if json_out {
        let rows: Vec<Value> = reasons::all()
            .iter()
            .map(|r| json!({"id": r.id, "guard": r.guard, "marker": r.marker, "decision": r.decision, "text": r.text}))
            .collect();
        println!("{}", ai_gent_core::output::ok_json(1, rows));
    } else {
        for r in reasons::all() {
            println!("{}\t{}\t{} {}", r.id, r.decision, r.marker, r.text);
        }
    }
    Exit::Success
}

/// Evaluate beside the shell guard and append the comparison to the shadow log.
fn shadow(a: &GuardArgs, input: &str) -> Result<(), String> {
    let guard = if a.guard == "all" {
        None
    } else {
        Some(GuardId::parse(&a.guard).ok_or_else(|| format!("unknown guard {}", a.guard))?)
    };
    let ev = Event::from_json(input)?;
    let shell = match &a.shell {
        Some(s) => Shell::parse(s).ok_or_else(|| format!("unknown shell {s}"))?,
        None if ev.tool_name == "PowerShell" => Shell::PowerShell,
        None => Shell::Sh,
    };
    let branch = |d: &str| git_branch(d);
    let ctx = Ctx { branch_of: &branch };
    let start = std::time::Instant::now();
    let d = guard.map_or_else(
        || evaluate_all(&ev, shell, &ctx),
        |g| evaluate(g, &ev, shell, &ctx),
    );
    let latency_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
    let shell_decision = a.shell_decision.clone().unwrap_or_else(|| "none".into());
    let info = VersionInfo::current(env!("CARGO_PKG_VERSION"));
    let cmd = ev.command();
    let rec = shadow::Record {
        schema: 1,
        ts: shadow::iso_utc(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |t| t.as_secs()),
        ),
        release: info.release,
        target: info.target,
        guard: a.guard.clone(),
        tool: ev.tool_name.clone(),
        shell: shell.name().to_string(),
        agree: shell_decision == d.level.name(),
        reason_equal: a.shell_reason.as_ref().map(|r| *r == d.reason),
        shell_decision,
        rust_decision: d.level.name().to_string(),
        expected_divergence: false,
        rule: (!d.rule.is_empty()).then(|| d.rule.to_string()),
        cmd_sha: cmd.map(|c| shadow::sha256_hex(c.as_bytes())[..16].to_string()),
        cmd_len: cmd.map_or(0, str::len),
        verb: cmd.and_then(shadow::verb),
        latency_us,
        capture: None,
    };
    let dir = shadow::default_log_dir().ok_or("no state directory (HOME or LOCALAPPDATA unset)")?;
    shadow::append(&dir, &rec).map_err(|e| format!("cannot write the shadow log: {e}"))
}

fn shadow_report(since: Option<&str>, guard: Option<&str>, json_out: bool) -> Exit {
    let since_secs = match since.map(shadow::parse_duration) {
        Some(None) => {
            eprintln!("ai-gent guard shadow-report: --since takes a number and s, m, h or d (7d)");
            return Exit::Usage;
        }
        Some(Some(d)) => Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |t| t.as_secs())
                .saturating_sub(d),
        ),
        None => None,
    };
    let Some(dir) = shadow::default_log_dir() else {
        eprintln!("ai-gent guard shadow-report: no state directory (HOME or LOCALAPPDATA unset)");
        return Exit::Runtime;
    };
    let rep = shadow::report(&shadow::read_all(&dir), since_secs, guard);
    if json_out {
        println!("{}", ai_gent_core::output::ok_json(1, &rep));
    } else {
        println!("shadow log: {}", dir.join(shadow::LOG_NAME).display());
        if rep.guards.is_empty() {
            println!("no records");
        }
        for (g, c) in &rep.guards {
            println!(
                "{g:<11} {:>6} evaluations  {:>6} agree  {:>4} disagree  {:>4} expected",
                c.evaluations, c.agree, c.disagree, c.expected_divergence
            );
        }
        for r in &rep.disagreements {
            println!(
                "DISAGREE {} {} shell={} rust={} rule={} sha={} verb={}",
                r.ts,
                r.guard,
                r.shell_decision,
                r.rust_decision,
                r.rule.as_deref().unwrap_or("-"),
                r.cmd_sha.as_deref().unwrap_or("-"),
                r.verb.as_deref().unwrap_or("-")
            );
        }
    }
    if rep.disagreements.is_empty() {
        Exit::Success
    } else {
        Exit::Runtime
    }
}
