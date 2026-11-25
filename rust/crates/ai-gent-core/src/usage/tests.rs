use super::*;
#[test]
fn digest_format_requires_exact_lowercase_sha256() {
    assert!(identity::digest(&format!("sha256:{}", "a".repeat(64))));
    assert!(!identity::digest(&format!("sha256:{}", "A".repeat(64))));
    assert!(!identity::digest("sha256:short"));
}

#[test]
fn invalid_utf8_unknown_fields_and_nil_identifiers_are_rejected_before_storage() {
    let directory = tempfile::tempdir().unwrap();
    for input in [
        b"not json".as_slice(),
        b"\xff",
        br#"{"schema_version":1,"prompt":"private"}"#,
    ] {
        assert_eq!(
            execute(directory.path(), "register", input)
                .unwrap_err()
                .exit_code(),
            2
        );
        assert!(!directory.path().join(".ai-gent").exists());
    }
    let mut request: serde_json::Value = registration();
    request["request_id"] = "00000000-0000-0000-0000-000000000000".into();
    assert_eq!(
        execute(directory.path(), "register", request.to_string().as_bytes())
            .unwrap_err()
            .exit_code(),
        2
    );
}
#[test]
fn oversized_requests_are_rejected_without_allocating_storage() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        execute(directory.path(), "register", &vec![b' '; MAX_RECORD + 1])
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert!(!directory.path().join(".ai-gent").exists());
}

#[test]
fn core_registration_replays_idempotently_and_denies_role_or_scope_mismatch() {
    let root = tempfile::tempdir().unwrap();
    let feature = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&feature).unwrap();
    std::fs::write(feature.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let mut input: serde_json::Value = registration();
    let first = execute(root.path(), "register", input.to_string().as_bytes()).unwrap();
    assert_eq!(
        execute(root.path(), "register", input.to_string().as_bytes()).unwrap(),
        first
    );
    input["request_id"] = "10000000-0000-4000-8000-000000000002".into();
    input["payload"]["role"] = "reviewer".into();
    assert_eq!(
        execute(root.path(), "register", input.to_string().as_bytes())
            .unwrap_err()
            .exit_code(),
        2
    );
    input["payload"]["role"] = "builder".into();
    input["payload"]["task"]["repository_id"] = "20000000-0000-4000-8000-000000000002".into();
    assert_eq!(
        execute(root.path(), "register", input.to_string().as_bytes())
            .unwrap_err()
            .exit_code(),
        4
    );
    assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 1);
}

fn registration() -> serde_json::Value {
    serde_json::json!({
        "schema_version":1,"request_id":"10000000-0000-4000-8000-000000000001",
        "repository_id":"20000000-0000-4000-8000-000000000001","operation":"register",
        "payload":{"task":{"repository_id":"20000000-0000-4000-8000-000000000001",
        "feature_id":"046-task-usage-ledger","task_id":"T001"},"phase":"builder",
        "role":"builder","purpose":"implementation","configuration":{"agent_app":"codex",
        "agent_version":"fixture-1","config_digest":format!("sha256:{}","a".repeat(64))}}
    })
}

#[test]
fn streamed_files_preserve_noncanonical_frame_and_legacy_line_boundary() {
    let root = tempfile::tempdir().unwrap();
    let feature = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&feature).unwrap();
    std::fs::write(feature.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let request = registration();
    execute(root.path(), "register", request.to_string().as_bytes()).unwrap();
    let expected = replay(root.path()).unwrap();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // Whitespace is outside the canonical payload checksum, including legacy maximum padding.
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.resize(MAX_RECORD, b' ');
    bytes.push(b'\n');
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(replay(root.path()).unwrap(), expected);
    bytes.insert(bytes.len() - 1, b' ');
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(replay(root.path()).unwrap_err().code, "limit_exceeded");
}

#[test]
fn streamed_both_journals_preserve_whole_file_error_precedence() {
    use std::io::{Seek, SeekFrom, Write};
    for observations_log in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let store = journal::Store::open(root.path(), true).unwrap();
        let path = if observations_log {
            observations::path(&store)
        } else {
            store.log.clone()
        };
        let read_error = || {
            if observations_log {
                observations::read(&store, &identity::Ledger::default())
                    .err()
                    .unwrap()
                    .code
            } else {
                store.load().err().unwrap().code
            }
        };
        std::fs::write(&path, b"bad\nx").unwrap();
        assert_eq!(read_error(), "evidence_corrupt");
        let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(64 * 1024 * 1024 + 1).unwrap();
        file.seek(SeekFrom::End(-1)).unwrap();
        file.write_all(b"\n").unwrap();
        assert_eq!(read_error(), "limit_exceeded");
        std::fs::write(&path, b"bad\n").unwrap();
        assert_eq!(read_error(), "evidence_corrupt");
        std::fs::write(&path, b"\n").unwrap();
        assert_eq!(read_error(), "evidence_corrupt");
        std::fs::write(&path, b"").unwrap();
        if observations_log {
            assert!(
                observations::read(&store, &identity::Ledger::default())
                    .unwrap()
                    .is_empty()
            );
        } else {
            assert_eq!(store.load().unwrap().sequence, 0);
        }
    }
}
