use super::*;
use serde_json::json;

fn full_message(id: &str, body: &str) -> Value {
    json!({
        "message": {"id":id,"threadId":"abc123","internalDate":"1793012400000",
            "historyId":"42","labelIds":["INBOX","Label_1"],
            "payload":{"mimeType":"multipart/mixed","headers":[
                {"name":"Subject","value":"Same subject"},
                {"name":"X-Original","value":"åäö"}],
                "parts":[{"mimeType":"text/plain","body":{"data":"b3JpZ2luYWw"}},
                    {"mimeType":"application/pdf","filename":"receipt.pdf","body":{"attachmentId":"opaque-id","size":17}}]}},
        "headers":{"from":"Alice <alice@example.com>","to":"Owner <owner@example.com>","subject":"Same subject","date":"invalid sender date"},
        "body":body,
        "attachments":[{"attachmentId":"opaque-id","filename":"receipt.pdf","size":17}]
    })
}

#[test]
fn full_body_provider_payload_and_attachment_coverage_are_retained() {
    let body = format!("{}\nOriginal åäö", "long body ".repeat(5000));
    let full = full_message("a1", &body);
    let record = message_to_record(
        "owner@example.com",
        "a1",
        &["Invoices".into()],
        full.clone(),
    )
    .unwrap();
    let snapshot: Value = serde_json::from_str(&record.content).unwrap();
    assert_eq!(snapshot["full_message"], full);
    assert_eq!(snapshot["full_message"]["body"], body);
    assert_eq!(record.kind, Kind::Message);
    assert_eq!(record.created_at.timestamp_millis(), 1_793_012_400_000);
    assert!(record.tags.contains(&"attachments:metadata-only".into()));
    assert!(record.tags.contains(&"Invoices".into()));
    assert!(record.url.contains("authuser=owner%40example.com#all/a1"));
    assert!(record.participants.contains(&"owner@example.com".into()));
}

#[test]
fn identities_are_account_message_and_version_scoped_with_deterministic_replays() {
    let first = message_to_record(
        "owner@example.com",
        "a1",
        &["B".into(), "A".into()],
        full_message("a1", "original"),
    )
    .unwrap();
    let replay = message_to_record(
        "owner@example.com",
        "a1",
        &["A".into(), "B".into(), "A".into()],
        full_message("a1", "original"),
    )
    .unwrap();
    assert_eq!(first.id, replay.id);
    assert_eq!(first.content_hash(), replay.content_hash());
    let corrected = message_to_record(
        "owner@example.com",
        "a1",
        &["A".into(), "B".into()],
        full_message("a1", "correction"),
    )
    .unwrap();
    let other_message = message_to_record(
        "owner@example.com",
        "b2",
        &[],
        full_message("b2", "original"),
    )
    .unwrap();
    let other_account = message_to_record(
        "other@example.com",
        "a1",
        &[],
        full_message("a1", "original"),
    )
    .unwrap();
    assert_ne!(first.id, corrected.id);
    assert_ne!(first.id, other_message.id);
    assert_ne!(first.id, other_account.id);
    assert_eq!(first.thread_id, corrected.thread_id);
    assert_eq!(first.thread_id, other_message.thread_id);
    assert_ne!(first.thread_id, other_account.thread_id);
}

#[test]
fn malformed_or_incomplete_full_responses_fail_without_fabricated_dates_or_bodies() {
    for value in [json!({"message":{"id":"a1"}}), json!({"error":"denied"})] {
        assert!(message_to_record("owner@example.com", "a1", &[], value).is_err());
    }
    let mut value = full_message("a1", "original");
    value["message"]["internalDate"] = json!("bad timestamp");
    assert!(message_to_record("owner@example.com", "a1", &[], value).is_err());
    assert!(message_to_record(
        "owner@example.com",
        "b2",
        &[],
        full_message("a1", "wrong message")
    )
    .is_err());
    let mut empty = full_message("a1", "");
    empty.as_object_mut().unwrap().remove("body");
    let record = message_to_record("owner@example.com", "a1", &[], empty).unwrap();
    assert!(record.tags.contains(&"body:not-reported".into()));
    assert!(!record.content.contains("invalid sender date\nSame subject"));
}

#[cfg(unix)]
mod command_contract {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};

    struct Harness {
        directory: PathBuf,
        state: SyncState,
    }
    impl Harness {
        fn new(mode: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "pasta-gmail-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("mode"), mode).unwrap();
            std::fs::write(directory.join("messages.json"), serde_json::to_vec(&json!({"a1":full_message("a1", "Original first"), "b2":full_message("b2", "Original second")})).unwrap()).unwrap();
            let script = directory.join("gog");
            std::fs::write(&script, r"#!/usr/bin/env python3
import json, pathlib, sys
root = pathlib.Path(__file__).parent
args = sys.argv[1:]
with (root / 'calls').open('a') as output:
    output.write(json.dumps(args) + '\n')
assert '--account=owner@example.com' in args
assert '--readonly' in args and '--no-input' in args and '--json' in args
assert '--wrap-untrusted=false' in args
mode = (root / 'mode').read_text()
if 'search' in args:
    assert args[5:8] == ['gmail', 'messages', 'search']
    assert '-category:promotions -category:social' in args[8]
    assert '--all' not in args
    if mode == 'malformed':
        print(json.dumps({'threads': []}))
    elif mode == 'empty':
        print(json.dumps({'messages': [], 'nextPageToken': ''}))
    elif '--page' in args:
        assert args[args.index('--page') + 1] == 'next'
        print(json.dumps({'messages': [{'id':'a1'}, {'id':'b2','labels':['Invoices']}], 'nextPageToken': 'next' if mode == 'repeat' else ''}))
    else:
        print(json.dumps({'messages': [{'id':'a1','labels':['Invoices']}], 'nextPageToken':'next'}))
elif 'get' in args:
    assert args[5:7] == ['gmail', 'get']
    assert args[args.index('--format') + 1] == 'full'
    assert '--use-indexed-attachment-ids=false' in args
    message_id = args[7]
    if mode == 'fail_get' and message_id == 'b2':
        print('injected provider failure', file=sys.stderr)
        sys.exit(1)
    print(json.dumps(json.loads((root / 'messages.json').read_text())[message_id]))
else:
    raise AssertionError('unexpected command, no mailbox mutations permitted')
").unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let config = kb_core::KbConfig {
                data_dir: directory.join("kb"),
            };
            let state = SyncState::open_staged(&config).unwrap();
            Self { directory, state }
        }
        fn fetcher(&self) -> GmailFetcher {
            let mut fetcher = GmailFetcher::new().with_account("OWNER@example.com");
            fetcher.executable = self.directory.join("gog");
            fetcher
        }
        fn calls(&self) -> Vec<Value> {
            std::fs::read_to_string(self.directory.join("calls"))
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        }
    }
    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[tokio::test]
    async fn paginates_gets_each_message_once_and_stages_account_cursor_after_success() {
        let harness = Harness::new("ok");
        harness
            .state
            .update_window("gmail_forward", "2099/01/01", "2099/01/02")
            .unwrap();
        let records = harness.fetcher().fetch(&harness.state).await.unwrap();
        assert_eq!(records.len(), 2);
        assert_ne!(records[0].id, records[1].id);
        let calls = harness.calls();
        assert_eq!(calls.len(), 4); // Two search pages, two full gets; duplicate a1 skipped.
        let key = cursor_key("owner@example.com");
        assert!(harness.state.cursor(&key).is_none());
        // Production kb-sync persists records before committing this stage.
        harness.state.commit_windows().unwrap();
        assert!(harness.state.cursor(&key).is_some());
        assert!(harness
            .state
            .cursor(&cursor_key("other@example.com"))
            .is_none());
    }

    #[tokio::test]
    async fn failures_on_later_messages_or_pagination_leave_cursor_retryable() {
        for mode in ["fail_get", "repeat", "malformed"] {
            let harness = Harness::new(mode);
            assert!(harness.fetcher().fetch(&harness.state).await.is_err());
            harness.state.commit_windows().unwrap();
            assert!(harness
                .state
                .cursor(&cursor_key("owner@example.com"))
                .is_none());
            std::fs::write(harness.directory.join("mode"), "ok").unwrap();
            assert_eq!(
                harness.fetcher().fetch(&harness.state).await.unwrap().len(),
                2
            );
        }
    }

    #[tokio::test]
    async fn explicit_empty_window_succeeds_but_invalid_account_fails_before_command() {
        let harness = Harness::new("empty");
        assert!(harness
            .fetcher()
            .fetch(&harness.state)
            .await
            .unwrap()
            .is_empty());
        harness.state.commit_windows().unwrap();
        assert!(harness
            .state
            .cursor(&cursor_key("owner@example.com"))
            .is_some());
        let mut fetcher = harness.fetcher();
        fetcher.account = Some("auto".into());
        assert!(fetcher.fetch(&harness.state).await.is_err());
        assert_eq!(harness.calls().len(), 1);
    }
}

#[test]
fn mailbox_lists_project_individual_people_without_changing_original_headers() {
    let mut full = full_message("a1", "Original body");
    full["headers"]["to"] = json!("\"Family, Bob\" <bob@example.com>, Carol <carol@example.com>");
    full["headers"]["cc"] = json!("Helpers: Dan <dan@example.com>, Eve <eve@example.com>;");
    let record = message_to_record("owner@example.com", "a1", &[], full.clone()).unwrap();
    assert_eq!(record.author, "alice@example.com");
    assert_eq!(
        record.participants,
        [
            "alice@example.com",
            "bob@example.com",
            "carol@example.com",
            "dan@example.com",
            "eve@example.com"
        ]
    );
    let snapshot: Value = serde_json::from_str(&record.content).unwrap();
    assert_eq!(snapshot["full_message"], full);
    full["headers"]["bcc"] = json!("Broken <");
    let partial = message_to_record("owner@example.com", "a1", &[], full).unwrap();
    assert_eq!(partial.participants, record.participants);
    assert!(partial.tags.contains(&"participants:partial".into()));
}
