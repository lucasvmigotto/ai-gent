use super::*;
use serde_json::{Value, json};

fn uuid(number: u128) -> Uuid {
    Uuid::from_u128(number)
}
fn fixture() -> Value {
    json!({"storage_schema_version":2,"kind":"segmented_usage_journal",
        "repository_id":uuid(1),"generation":1,"predecessor_manifest_digest":null,
        "migration_id":null,"streams":{"registrations":empty(2),"observations":empty(3)}})
}
fn empty(id: u128) -> Value {
    stream(&[], id, 0, 0)
}
fn closed(id: u128, first: u64, last: u64, bytes: u64) -> Value {
    json!({"object_id":uuid(id),"first_sequence":first,"last_sequence":last,
        "byte_length":bytes,"raw_sha256":hash(b"synthetic object; source not verified"),"frame_count":last-first+1})
}
fn stream(closed: &[Value], id: u128, sequence: u64, bytes: u64) -> Value {
    let previous = closed
        .last()
        .map_or(0, |value| value["last_sequence"].as_u64().unwrap());
    json!({"sequence":sequence,"closed":closed,"active":{"object_id":uuid(id),
        "first_sequence":previous+1,"committed_sequence":sequence,"committed_bytes":bytes,
        "committed_raw_sha256":if bytes==0 {hash(b"")} else {hash(b"synthetic active; source not verified")}}})
}
fn head(manifest: &[u8], generation: u64) -> Vec<u8> {
    canonical(&json!({"storage_schema_version":2,"generation":generation,"manifest_digest":hash(manifest)})).unwrap()
}
fn parse(value: &Value) -> Result<Manifest> {
    let bytes = canonical(value).unwrap();
    parse_metadata(
        &head(&bytes, value["generation"].as_u64().unwrap_or(1)),
        &bytes,
        uuid(1),
    )
}
fn rejects(value: &Value, code: &str) {
    assert_eq!(parse(value).unwrap_err().code, code);
}

#[test]
fn accepts_empty_and_contiguous_mixed_metadata_without_verifying_sources() {
    let parsed = parse(&fixture()).unwrap();
    assert_eq!(parsed.generation, 1);
    let mut value = fixture();
    value["streams"]["registrations"] =
        stream(&[closed(4, 1, 2, 123), closed(5, 3, 4, 130)], 2, 5, 71);
    value["streams"]["observations"] = stream(&[closed(6, 1, 7, 321)], 3, 7, 0);
    let parsed = parse(&value).unwrap();
    assert_eq!(parsed.streams.registrations.sequence, 5);
    assert_eq!(parsed.streams.observations.closed[0].frame_count, 7);
}

#[test]
fn input_bounds_precede_json_and_accept_exact_byte_count_for_parsing() {
    for (h, m, expected) in [
        (vec![b'x'; MAX_HEAD + 1], vec![], "limit_exceeded"),
        (vec![], vec![b'x'; MAX_MANIFEST + 1], "limit_exceeded"),
        (vec![b'x'; MAX_HEAD], vec![], "evidence_corrupt"),
        (vec![], vec![b'x'; MAX_MANIFEST], "evidence_corrupt"),
    ] {
        assert_eq!(parse_metadata(&h, &m, uuid(1)).unwrap_err().code, expected);
    }
}

#[test]
fn head_must_bind_manifest_schema_generation_and_literal_digest() {
    let bytes = canonical(&fixture()).unwrap();
    for change in [
        json!({"storage_schema_version":1}),
        json!({"generation":2}),
        json!({"generation":0}),
        json!({"manifest_digest":hash(b"other")}),
        json!({"manifest_digest":"sha256:BAD"}),
        json!({"extra":true}),
    ] {
        let mut h: Value = serde_json::from_slice(&head(&bytes, 1)).unwrap();
        h.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert_eq!(
            parse_metadata(&canonical(&h).unwrap(), &bytes, uuid(1))
                .unwrap_err()
                .code,
            "evidence_corrupt"
        );
    }
}

#[test]
fn canonicality_rejects_whitespace_duplicates_and_missing_nullable_fields() {
    let mut bytes = canonical(&fixture()).unwrap();
    bytes.push(b'\n');
    assert_eq!(
        parse_metadata(&head(&bytes, 1), &bytes, uuid(1))
            .unwrap_err()
            .code,
        "evidence_corrupt"
    );
    let bytes = canonical(&fixture()).unwrap();
    let mut h = head(&bytes, 1);
    h.push(b' ');
    assert_eq!(
        parse_metadata(&h, &bytes, uuid(1)).unwrap_err().code,
        "evidence_corrupt"
    );
    for key in ["migration_id", "predecessor_manifest_digest"] {
        let mut value = fixture();
        value.as_object_mut().unwrap().remove(key);
        rejects(&value, "evidence_corrupt");
    }
    let mut duplicate = b"{\"generation\":1,".to_vec();
    duplicate.extend_from_slice(&bytes[1..]);
    assert_eq!(
        parse_metadata(&head(&duplicate, 1), &duplicate, uuid(1))
            .unwrap_err()
            .code,
        "evidence_corrupt"
    );
}

#[test]
fn every_typed_layer_rejects_unknown_fields() {
    for pointer in [
        "",
        "/streams",
        "/streams/registrations",
        "/streams/observations/active",
    ] {
        let mut value = fixture();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(true));
        rejects(&value, "evidence_corrupt");
    }
    let mut value = fixture();
    value["streams"]["registrations"] = stream(&[closed(4, 1, 1, 50)], 2, 1, 0);
    value["streams"]["registrations"]["closed"][0]["extra"] = json!(true);
    rejects(&value, "evidence_corrupt");
}

#[test]
fn version_kind_repository_and_migration_identity_are_validated() {
    for (key, invalid) in [
        ("storage_schema_version", json!(1)),
        ("kind", json!("different")),
        ("repository_id", json!(Uuid::nil())),
        ("repository_id", json!(uuid(99))),
        ("migration_id", json!(Uuid::nil())),
    ] {
        let mut value = fixture();
        value[key] = invalid;
        rejects(&value, "evidence_corrupt");
    }
    let bytes = canonical(&fixture()).unwrap();
    assert_eq!(
        parse_metadata(&head(&bytes, 1), &bytes, Uuid::nil())
            .unwrap_err()
            .code,
        "evidence_corrupt"
    );
}

#[test]
fn generation_and_predecessor_consistency_do_not_impose_retention_policy() {
    for (generation, prior, valid) in [
        (0, None, false),
        (1, None, true),
        (1, Some(hash(b"prior")), false),
        (2, None, false),
        (2, Some(hash(b"prior")), true),
        (u64::MAX, Some(hash(b"prior")), true),
        (2, Some("sha256:bad".into()), false),
    ] {
        let mut value = fixture();
        value["generation"] = json!(generation);
        value["predecessor_manifest_digest"] = json!(prior);
        assert_eq!(parse(&value).is_ok(), valid);
    }
}

#[test]
fn object_ids_are_non_nil_and_unique_across_roles_and_streams() {
    for id in [Uuid::nil(), uuid(2)] {
        let mut value = fixture();
        value["streams"]["observations"]["active"]["object_id"] = json!(id);
        rejects(&value, "evidence_corrupt");
    }
    let mut value = fixture();
    value["streams"]["registrations"] = stream(&[closed(4, 1, 1, 50)], 2, 1, 0);
    value["streams"]["observations"] = stream(&[closed(4, 1, 1, 50)], 3, 1, 0);
    rejects(&value, "evidence_corrupt");
    value["streams"]["observations"] = empty(4);
    rejects(&value, "evidence_corrupt");
    value["streams"]["observations"] = empty(3);
    value["streams"]["registrations"]["closed"][0]["object_id"] = json!(Uuid::nil());
    rejects(&value, "evidence_corrupt");
}

#[test]
fn closed_ranges_reject_gaps_overlaps_wrong_counts_and_underflow() {
    for (first, last, count) in [
        (0, 0, 1),
        (2, 3, 2),
        (1, 2, 1),
        (2, 1, 1),
        (1, u64::MAX, u64::MAX),
        (0, u64::MAX, u64::MAX),
        (1, 1, 0),
    ] {
        let mut value = fixture();
        let mut object = closed(4, 1, 1, 50);
        object["first_sequence"] = json!(first);
        object["last_sequence"] = json!(last);
        object["frame_count"] = json!(count);
        value["streams"]["registrations"]["closed"] = json!([object]);
        rejects(&value, "evidence_corrupt");
    }
    for first in [1, 3] {
        let mut value = fixture();
        value["streams"]["registrations"] =
            stream(&[closed(4, 1, 1, 50), closed(5, first, 3, 50)], 2, 3, 0);
        rejects(&value, "evidence_corrupt");
    }
}

#[test]
fn empty_active_requires_next_first_and_previous_committed_sequence_and_empty_hash() {
    for (key, invalid) in [
        ("first_sequence", json!(0)),
        ("committed_sequence", json!(1)),
        ("committed_raw_sha256", json!(hash(b"not empty"))),
    ] {
        let mut value = fixture();
        value["streams"]["registrations"]["active"][key] = invalid;
        rejects(&value, "evidence_corrupt");
    }
    let mut value = fixture();
    value["streams"]["registrations"]["sequence"] = json!(1);
    value["streams"]["registrations"]["active"]["committed_sequence"] = json!(1);
    rejects(&value, "evidence_corrupt");
}

#[test]
fn nonempty_active_requires_positive_count_matching_sequence_and_hash_shape() {
    for (sequence, committed, bytes, raw) in [
        (0, 0, 10, hash(b"data")),
        (2, 1, 10, hash(b"data")),
        (2, 2, 1, hash(b"data")),
        (1, 1, 10, "SHA256:bad".into()),
    ] {
        let mut value = fixture();
        value["streams"]["registrations"] = stream(&[], 2, sequence, bytes);
        value["streams"]["registrations"]["active"]["committed_sequence"] = json!(committed);
        value["streams"]["registrations"]["active"]["committed_raw_sha256"] = json!(raw);
        rejects(&value, "evidence_corrupt");
    }
}

#[test]
fn closed_byte_bounds_and_hash_shapes_are_checked() {
    for (bytes, raw, expected) in [
        (0, hash(b"data"), "evidence_corrupt"),
        (1, hash(b"data"), "evidence_corrupt"),
        (2, "sha256:bad".into(), "evidence_corrupt"),
        (MAX_SEGMENT + 1, hash(b"data"), "limit_exceeded"),
        (u64::MAX, hash(b"data"), "limit_exceeded"),
    ] {
        let mut value = fixture();
        let mut object = closed(4, 1, 2, bytes);
        object["raw_sha256"] = json!(raw);
        value["streams"]["registrations"] = stream(&[object], 2, 2, 0);
        rejects(&value, expected);
    }
}

#[test]
fn migration_exception_is_first_closed_only_and_capped_at_legacy_capacity() {
    let mut value = fixture();
    value["migration_id"] = json!(uuid(10));
    value["streams"]["registrations"] = stream(&[closed(4, 1, 1, MAX_LEGACY)], 2, 1, 0);
    assert!(parse(&value).is_ok());
    value["migration_id"] = Value::Null;
    rejects(&value, "limit_exceeded");
    value["migration_id"] = json!(uuid(10));
    value["streams"]["registrations"]["closed"][0]["byte_length"] = json!(MAX_LEGACY + 1);
    rejects(&value, "limit_exceeded");
    value["streams"]["registrations"] = stream(
        &[closed(4, 1, 1, 50), closed(5, 2, 2, MAX_SEGMENT + 1)],
        2,
        2,
        0,
    );
    rejects(&value, "limit_exceeded");
    value["streams"]["registrations"] = stream(&[], 2, 1, MAX_SEGMENT + 1);
    rejects(&value, "limit_exceeded");
}

#[test]
fn stream_closed_count_and_frame_caps_have_explicit_boundaries() {
    for count in [MAX_CLOSED, MAX_CLOSED + 1] {
        let objects: Vec<_> = (0..count)
            .map(|i| closed(i as u128 + 10, i as u64 + 1, i as u64 + 1, 50))
            .collect();
        let mut value = fixture();
        value["streams"]["registrations"] = stream(&objects, 2, count as u64, 0);
        if count == MAX_CLOSED {
            assert!(parse(&value).is_ok());
        } else {
            rejects(&value, "limit_exceeded");
        }
    }
    for sequence in [MAX_FRAMES, MAX_FRAMES + 1, u64::MAX] {
        let mut value = fixture();
        value["streams"]["registrations"] = stream(&[], 2, sequence, MAX_SEGMENT);
        if sequence == MAX_FRAMES {
            assert!(parse(&value).is_ok());
        } else {
            rejects(&value, "limit_exceeded");
        }
    }
}

#[test]
fn stream_and_combined_byte_caps_accept_exact_boundary() {
    let mut value = fixture();
    for (name, start, active) in [("registrations", 10, 2), ("observations", 100, 3)] {
        let objects: Vec<_> = (0_u64..32)
            .map(|i| closed(start + u128::from(i), i + 1, i + 1, MAX_SEGMENT))
            .collect();
        value["streams"][name] = stream(&objects, active, 32, 0);
    }
    assert!(parse(&value).is_ok()); // each256MiB, combined512MiB metadata only.
    value["streams"]["observations"]["sequence"] = json!(33);
    value["streams"]["observations"]["active"]["committed_sequence"] = json!(33);
    value["streams"]["observations"]["active"]["committed_bytes"] = json!(1);
    value["streams"]["observations"]["active"]["committed_raw_sha256"] = json!(hash(b"x"));
    rejects(&value, "limit_exceeded");
}

#[test]
fn canonical_uuid_spelling_and_same_stream_uniqueness_are_required() {
    let mut value = fixture();
    value["streams"]["observations"]["active"]["object_id"] =
        json!("ABCDEF00-0000-4000-8000-000000000003");
    rejects(&value, "evidence_corrupt");
    value["streams"]["observations"]["active"]["object_id"] =
        json!("abcdef00000040008000000000000003");
    rejects(&value, "evidence_corrupt");
    value = fixture();
    value["streams"]["registrations"] =
        stream(&[closed(4, 1, 1, 50), closed(4, 2, 2, 50)], 2, 2, 0);
    rejects(&value, "evidence_corrupt");
    value["streams"]["registrations"] = stream(&[closed(2, 1, 1, 50)], 2, 1, 0);
    rejects(&value, "evidence_corrupt");
}
