use super::*;
use serde_json::json;

fn fixture(filename: &str, mime: &str, data: &str, size: u64) -> Value {
    json!({"message":{"payload":{"mimeType":"multipart/mixed","parts":[
        {"partId":"0","mimeType":"text/plain","body":{"data":"bWVzc2FnZQ","size":7}},
        {"partId":"1","filename":filename,"mimeType":mime,"body":{"data":data,"size":size}}
    ]}}})
}

#[test]
fn originals_survive_restart_and_retries_without_filename_paths_or_duplicate_blobs() {
    let root = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(root.path()).unwrap();
    let parts = discover(&fixture("../../receipt.txt", "text/plain", "aGVsbG8", 5)).unwrap();
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    let source = root.path().join("download");
    std::fs::write(&source, b"hello").unwrap();
    let blob = store
        .retain("owner@example.com", "a1", part, &source)
        .unwrap();
    assert_eq!(
        std::fs::read(store.blob_path(&blob).unwrap()).unwrap(),
        b"hello"
    );
    drop(store);
    let restarted = EvidenceStore::open(root.path()).unwrap();
    let replay = restarted
        .cached("owner@example.com", "a1", part)
        .unwrap()
        .unwrap();
    assert_eq!(replay.sha256, blob.sha256);
    assert!(restarted
        .cached("other@example.com", "a1", part)
        .unwrap()
        .is_none());
    assert_eq!(
        std::fs::read_dir(root.path().join("blobs"))
            .unwrap()
            .count(),
        1
    );
    assert!(!root.path().join("receipt.txt").exists());
    std::fs::write(restarted.blob_path(&blob).unwrap(), b"wrong").unwrap();
    assert!(restarted.cached("owner@example.com", "a1", part).is_err());
}

#[test]
fn wrong_size_is_not_published_and_changed_bytes_keep_distinct_versions() {
    let root = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(root.path()).unwrap();
    let parts = discover(&fixture("receipt.txt", "text/plain", "aGVsbG8", 5)).unwrap();
    let source = root.path().join("download");
    std::fs::write(&source, b"bad").unwrap();
    assert!(store
        .retain("owner@example.com", "a1", &parts[0], &source)
        .is_err());
    assert!(store
        .cached("owner@example.com", "a1", &parts[0])
        .unwrap()
        .is_none());
    std::fs::write(&source, b"hello").unwrap();
    let original = store
        .retain("owner@example.com", "a1", &parts[0], &source)
        .unwrap();
    let changed = discover(&fixture("receipt.txt", "text/plain", "d29ybGQ", 5)).unwrap();
    std::fs::write(&source, b"world").unwrap();
    let revised = store
        .retain("owner@example.com", "a1", &changed[0], &source)
        .unwrap();
    assert_ne!(original.sha256, revised.sha256);
    assert_eq!(
        std::fs::read_dir(root.path().join("blobs"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn text_projection_and_local_rebuild_keep_source_links_and_honest_coverage() {
    let root = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(root.path()).unwrap();
    for (message_id, filename, mime, data, bytes, status) in [
        (
            "a1",
            "notes.txt",
            "text/plain",
            "aGVsbG8",
            b"hello".as_slice(),
            "utf8_text",
        ),
        (
            "b2",
            "receipt.pdf",
            "application/pdf",
            "aGVsbG8",
            b"hello".as_slice(),
            "unsupported_mime",
        ),
        (
            "c3",
            "binary.txt",
            "text/plain",
            "_w",
            b"\xff".as_slice(),
            "unsupported_encoding",
        ),
    ] {
        let mut full = super::super::tests::full_message(message_id, "Original message");
        full["message"]["payload"] =
            fixture(filename, mime, data, bytes.len() as u64)["message"]["payload"].clone();
        let parent =
            super::super::message_to_record("owner@example.com", message_id, &[], full.clone())
                .unwrap();
        let part = discover(&full).unwrap().remove(0);
        let path = root.path().join("download");
        std::fs::write(&path, bytes).unwrap();
        let blob = store
            .retain("owner@example.com", message_id, &part, &path)
            .unwrap();
        let records = store.finish(parent, vec![(part, blob)]).unwrap();
        let content: Value = serde_json::from_str(&records[1].content).unwrap();
        assert_eq!(content["parent_record_id"], records[0].id);
        assert_eq!(content["extraction"]["status"], status);
        assert_eq!(content["indexing"], "not_verified");
        if status == "utf8_text" {
            assert_eq!(content["extraction"]["text"], "hello");
        } else {
            assert!(content["extraction"]["text"].is_null());
        }
    }
    drop(store);
    let replay = read_records(root.path()).unwrap();
    assert_eq!(replay.len(), 6);
    assert_eq!(
        replay
            .iter()
            .filter(|record| record.kind == Kind::Doc)
            .count(),
        3
    );
    let backup = tempfile::tempdir().unwrap();
    for directory in ["blobs", "receipts", "messages"] {
        std::fs::create_dir(backup.path().join(directory)).unwrap();
        for entry in std::fs::read_dir(root.path().join(directory)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(
                entry.path(),
                backup.path().join(directory).join(entry.file_name()),
            )
            .unwrap();
        }
    }
    drop(root);
    let restored = read_records(backup.path()).unwrap();
    assert_eq!(
        replay.iter().map(Record::content_hash).collect::<Vec<_>>(),
        restored
            .iter()
            .map(Record::content_hash)
            .collect::<Vec<_>>()
    );
}

#[test]
fn discovery_handles_inline_files_empty_headers_and_mime_case() {
    let mut full = fixture("", "TEXT/PLAIN; charset=utf-8", "aGVsbG8", 5);
    // This is a normal body, not an unnamed attachment.
    full["message"]["payload"]["parts"][1]["headers"] = json!([{"name":"Content-ID"}]);
    assert!(discover(&full).unwrap().is_empty());
    full["message"]["payload"]["parts"][1]["headers"] =
        json!([{"name":"Content-ID","value":"<inline-file>"}]);
    assert_eq!(discover(&full).unwrap().len(), 1);
    full["message"]["payload"]["parts"][1]["body"]["data"] = Value::Null;
    full["message"]["payload"]["parts"][1]["parts"] = json!([{"mimeType":"text/plain"}]);
    assert!(discover(&full).is_err());
}

#[test]
fn declared_charsets_do_not_silently_reinterpret_valid_utf8_bytes() {
    let root = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(root.path()).unwrap();
    for (id, charset, status) in [
        ("a1", "windows-1252", "unsupported_encoding"),
        ("b2", "us-ascii", "unsupported_encoding"),
        ("c3", "UTF-8", "utf8_text"),
    ] {
        let mut full = super::super::tests::full_message(id, "Original message");
        full["message"]["payload"] =
            fixture("names.txt", "text/plain", "w6Q", 2)["message"]["payload"].clone();
        full["message"]["payload"]["parts"][1]["headers"] =
            json!([{"name":"Content-Type","value":format!("text/plain; charset={charset}")}]);
        let part = discover(&full).unwrap().remove(0);
        let parent = super::super::message_to_record("owner@example.com", id, &[], full).unwrap();
        let source = root.path().join("download");
        std::fs::write(&source, b"\xc3\xa4").unwrap();
        let blob = store
            .retain("owner@example.com", id, &part, &source)
            .unwrap();
        let records = store.finish(parent, vec![(part, blob)]).unwrap();
        let content: Value = serde_json::from_str(&records[1].content).unwrap();
        assert_eq!(content["extraction"]["status"], status);
        if status == "utf8_text" {
            assert_eq!(content["extraction"]["text"], "ä");
        } else {
            assert!(content["extraction"]["text"].is_null());
        }
    }
}
