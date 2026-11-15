//! H-exec baseline: Python owns the drivers and the execution JSON contract.
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn failure(message: &str) -> Value {
    json!({"ok": false, "stage": "connect", "error": message})
}

/// Pass the Python request unchanged, through stdin only. The Python interpreter
/// must have the requested engine's driver installed (SQLite uses the stdlib).
pub fn execute(request: &Value) -> Value {
    let mut command = Command::new("python3");
    command.args([
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../plugins/db/scripts/dbrun.py"
        ),
        "_exec",
    ]);
    execute_command(&mut command, request)
}

fn execute_command(command: &mut Command, request: &Value) -> Value {
    let mut child = match command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return failure("could not start Python execution process"),
    };
    let sent = serde_json::to_writer(child.stdin.as_mut().unwrap(), request).is_ok()
        && child.stdin.as_mut().unwrap().write_all(b"\n").is_ok();
    drop(child.stdin.take());
    if !sent {
        let _ = child.kill();
        let _ = child.wait();
        return failure("could not send execution request");
    }
    let output = match child.wait_with_output() {
        Ok(output) if output.status.success() => output,
        _ => return failure("Python execution process failed"),
    };
    // Never include raw output in transport errors; it may contain connection facts.
    let result = std::str::from_utf8(&output.stdout).ok().and_then(|text| {
        let mut lines = text.lines();
        let first = lines.next()?;
        if lines.next().is_some() {
            return None;
        }
        serde_json::from_str::<Value>(first).ok()
    });
    match result {
        Some(result) if result.is_object() && result["ok"].is_boolean() => result,
        _ => failure("Python execution process returned invalid JSON"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python(source: &str, request: &Value) -> Value {
        execute_command(Command::new("python3").args(["-c", source]), request)
    }

    #[test]
    fn request_and_result_preserve_the_python_contract() {
        let request = json!({"engine": "oracle", "conn": {"service": "test"},
            "mode": "write", "statements": ["SELECT 1"], "timeout": 5,
            "max_rows": 10, "expect": [1], "explain": false});
        let result = python(
            "import json,sys; r=json.load(sys.stdin); print(json.dumps({'ok':False,'stage':'verify','request':r}))",
            &request,
        );
        assert_eq!(result["request"], request);
        assert_eq!(result["stage"], "verify");
        assert_eq!(result["ok"], false);
    }

    #[test]
    fn transport_failures_do_not_echo_child_output() {
        for source in [
            "import sys; sys.stdin.read(); print('private output')",
            "import sys; sys.stdin.read(); print('{}')",
            "import sys; sys.stdin.read(); print('[]')",
            "import sys; sys.stdin.read(); print('{\"ok\":true}\\n{\"ok\":true}')",
            "import sys; sys.stdin.read(); print('{\"ok\":true}'); sys.exit(1)",
        ] {
            let result = python(source, &json!({}));
            assert_eq!(result["ok"], false);
            assert_eq!(result["stage"], "connect");
            assert!(!result.to_string().contains("private output"));
        }
        let result = execute_command(&mut Command::new("/nonexistent/dbrun-python"), &json!({}));
        assert_eq!(result["ok"], false);
    }
}
