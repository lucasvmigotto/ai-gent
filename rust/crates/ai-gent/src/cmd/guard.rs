//! `ai-gent guard`: the guard evaluator (spec 033).
use crate::cli::{GuardAction, GuardArgs};
use ai_gent_core::exit::Exit;
use ai_gent_core::guard::branch::git_branch;
use ai_gent_core::guard::{
    Ctx, Decision, Event, GuardId, MAX_COMMAND_BYTES, Shell, evaluate, evaluate_all,
    evaluate_opencode, reasons,
};
use serde_json::{Value, json};
use std::io::Read;

/// Run `guard`. Evaluation always exits 0: a failure becomes an `ask` line (FR-014).
pub fn run(a: &GuardArgs, json_out: bool) -> Exit {
    if let Some(GuardAction::ListReasons) = a.action {
        return list_reasons(json_out);
    }
    let mut input = String::new();
    let read = std::io::stdin().read_to_string(&mut input);
    let marker = GuardId::parse(&a.guard).map_or("ai-gent guard:", |g| marker_of(g));
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
