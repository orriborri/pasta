use super::*;
use serde_json::json;
use std::sync::Mutex;

struct FakeProvider {
    state: Mutex<Snapshot>,
    writes: Mutex<usize>,
    lose_ack: bool,
    fail_read_after_write: Mutex<bool>,
}
impl Provider for FakeProvider {
    fn snapshot(&self, _: &str, _: &str) -> impl Future<Output = Result<Snapshot>> + Send {
        let result =
            if *self.writes.lock().unwrap() > 0 && *self.fail_read_after_write.lock().unwrap() {
                Err(anyhow::anyhow!("read-back unavailable"))
            } else {
                Ok(self.state.lock().unwrap().clone())
            };
        std::future::ready(result)
    }
    fn modify(
        &self,
        _: &str,
        _: &str,
        add: &[String],
        remove: &[String],
    ) -> impl Future<Output = Result<()>> + Send {
        *self.writes.lock().unwrap() += 1;
        let mut state = self.state.lock().unwrap();
        state.label_ids.retain(|id| !remove.contains(id));
        state.label_ids.extend_from_slice(add);
        state.label_ids.sort();
        state.label_ids.dedup();
        drop(state);
        std::future::ready(if self.lose_ack {
            Err(anyhow::anyhow!("acknowledgement lost"))
        } else {
            Ok(())
        })
    }
}

struct Fixture {
    root: tempfile::TempDir,
    context: LabelContext,
    request: LabelRequest,
    provider: FakeProvider,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let registry = root.path().join("registry.json");
        std::fs::write(&registry, r#"{"entries":[{"jd_id":"00.01","label":"Inbox","gmail_label":true,"state":"approved"},{"jd_id":"41.13","label":"Training","gmail_label":true,"state":"approved"}]}"#).unwrap();
        let full = json!({"message":{"id":"a1","threadId":"a1","internalDate":"1234","payload":{"mimeType":"text/plain","body":{"data":"b3JpZ2luYWw"}},"labelIds":["INBOX","old"],"historyId":"1"}});
        let body_json = serde_json::to_string(&json!({"schema":"pasta.gmail.evidence.v1","account":"owner@example.com","message_id":"a1","full_message":full})).unwrap();
        let record_id = format!("gmail-message-{}", hash(body_json.as_bytes()));
        let directory = root.path().join("originals/messages");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{record_id}.json")), serde_json::to_vec(&json!({"version":1,"message":{"id":record_id,"source":"gmail","kind":"message","content":body_json}})).unwrap()).unwrap();
        let context = LabelContext {
            registry,
            evidence_dir: root.path().join("originals"),
        };
        let request = LabelRequest {
            request_key: "owner-request-1".into(),
            account: "owner@example.com".into(),
            message_id: "a1".into(),
            evidence_record_id: record_id,
            jd_id: "41.13".into(),
            rationale: "Training plan for sea rescue".into(),
        };
        let state = Snapshot {
            content_version: message_version(&full, "a1").unwrap(),
            label_ids: vec!["INBOX".into(), "old".into(), "STARRED".into()],
            labels: vec![
                Label {
                    id: "INBOX".into(),
                    name: "INBOX".into(),
                    kind: "system".into(),
                },
                Label {
                    id: "STARRED".into(),
                    name: "STARRED".into(),
                    kind: "system".into(),
                },
                Label {
                    id: "old".into(),
                    name: "JDex/00.01".into(),
                    kind: "user".into(),
                },
                Label {
                    id: "new".into(),
                    name: "JDex/41.13".into(),
                    kind: "user".into(),
                },
            ],
        };
        Self {
            root,
            context,
            request,
            provider: FakeProvider {
                state: Mutex::new(state),
                writes: Mutex::new(0),
                lose_ack: false,
                fail_read_after_write: Mutex::new(false),
            },
        }
    }
    fn store(&self) -> Store {
        Store::open(&self.root.path().join("assistant.sqlite")).unwrap()
    }
}

#[tokio::test]
async fn review_approval_write_readback_and_replay_preserve_unrelated_labels() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let preview = store
        .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
        .await
        .unwrap();
    assert_eq!(preview.state, State::Preview);
    assert!(store
        .apply_gmail_label(&fixture.context, &preview.id, &fixture.provider)
        .await
        .is_err());
    assert_eq!(*fixture.provider.writes.lock().unwrap(), 0);
    assert!(store.approve_gmail_label(&preview.id, "wrong").is_err());
    store
        .approve_gmail_label(&preview.id, &preview.digest)
        .unwrap();
    drop(store);
    let applied = fixture
        .store()
        .apply_gmail_label(&fixture.context, &preview.id, &fixture.provider)
        .await
        .unwrap();
    assert_eq!(applied.state, State::Applied);
    assert_eq!(
        fixture.provider.state.lock().unwrap().label_ids,
        ["INBOX", "STARRED", "new"]
    );
    fixture
        .store()
        .apply_gmail_label(&fixture.context, &preview.id, &fixture.provider)
        .await
        .unwrap();
    assert_eq!(*fixture.provider.writes.lock().unwrap(), 1);
}

#[tokio::test]
async fn registry_or_provider_changes_invalidate_approval_before_writing() {
    for change in ["registry", "labels", "content"] {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let plan = store
            .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
            .await
            .unwrap();
        store.approve_gmail_label(&plan.id, &plan.digest).unwrap();
        match change {
            "registry" => std::fs::write(&fixture.context.registry, "{}").unwrap(),
            "labels" => fixture
                .provider
                .state
                .lock()
                .unwrap()
                .label_ids
                .push("new".into()),
            _ => fixture.provider.state.lock().unwrap().content_version = "changed".into(),
        }
        assert!(store
            .apply_gmail_label(&fixture.context, &plan.id, &fixture.provider)
            .await
            .is_err());
        assert_eq!(*fixture.provider.writes.lock().unwrap(), 0);
    }
}

#[tokio::test]
async fn lost_ack_is_verified_without_a_second_write_and_conflicting_keys_fail() {
    let mut fixture = Fixture::new();
    fixture.provider.lose_ack = true;
    let mut store = fixture.store();
    let plan = store
        .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
        .await
        .unwrap();
    let mut changed = fixture.request.clone();
    changed.jd_id = "00.01".into();
    assert!(store
        .preview_gmail_label(&fixture.context, &changed, &fixture.provider)
        .await
        .is_err());
    store.approve_gmail_label(&plan.id, &plan.digest).unwrap();
    let applied = store
        .apply_gmail_label(&fixture.context, &plan.id, &fixture.provider)
        .await
        .unwrap();
    assert_eq!(applied.state, State::Applied);
    assert_eq!(*fixture.provider.writes.lock().unwrap(), 1);
}

#[tokio::test]
async fn uncertain_execution_survives_restart_and_only_reconciles_read_only() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let plan = store
        .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
        .await
        .unwrap();
    store.approve_gmail_label(&plan.id, &plan.digest).unwrap();
    *fixture.provider.fail_read_after_write.lock().unwrap() = true;
    assert_eq!(
        store
            .apply_gmail_label(&fixture.context, &plan.id, &fixture.provider)
            .await
            .unwrap()
            .state,
        State::Uncertain
    );
    assert!(store
        .apply_gmail_label(&fixture.context, &plan.id, &fixture.provider)
        .await
        .is_err());
    assert!(store.approve_gmail_label(&plan.id, &plan.digest).is_err());
    drop(store);
    *fixture.provider.fail_read_after_write.lock().unwrap() = false;
    let restored = fixture
        .store()
        .reconcile_gmail_label(&plan.id, &fixture.provider)
        .await
        .unwrap();
    assert_eq!(restored.state, State::Applied);
    assert_eq!(*fixture.provider.writes.lock().unwrap(), 1);
}

#[tokio::test]
async fn routes_and_retained_identities_require_explicit_permission() {
    for mutation in ["unknown", "disabled", "account", "evidence"] {
        let mut fixture = Fixture::new();
        match mutation {
            "unknown" => fixture.request.jd_id = "99.99".into(),
            "disabled" => std::fs::write(
                &fixture.context.registry,
                r#"{"entries":[{"jd_id":"41.13","label":"Training"}]}"#,
            )
            .unwrap(),
            "account" => fixture.request.account = "other@example.com".into(),
            _ => fixture.request.evidence_record_id = "../../secrets".into(),
        }
        assert!(fixture
            .store()
            .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
            .await
            .is_err());
        assert_eq!(*fixture.provider.writes.lock().unwrap(), 0);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn gog_adapter_reads_then_modifies_only_the_exact_native_message_delta() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(
            fixture
                .context
                .evidence_dir
                .join("messages")
                .join(format!("{}.json", fixture.request.evidence_record_id)),
        )
        .unwrap(),
    )
    .unwrap();
    let source: Value =
        serde_json::from_str(manifest["message"]["content"].as_str().unwrap()).unwrap();
    std::fs::write(
        fixture.root.path().join("full.json"),
        serde_json::to_vec(&source["full_message"]).unwrap(),
    )
    .unwrap();
    let labels = json!([{"id":"INBOX","name":"INBOX","type":"system"},{"id":"STARRED","name":"STARRED","type":"system"},{"id":"Label_1","name":"JDex/00.01","type":"user"},{"id":"Label_2","name":"JDex/41.13","type":"user"}]);
    std::fs::write(
        fixture.root.path().join("catalog.json"),
        serde_json::to_vec(&labels).unwrap(),
    )
    .unwrap();
    std::fs::write(
        fixture.root.path().join("state.json"),
        r#"["INBOX","STARRED","Label_1"]"#,
    )
    .unwrap();
    let executable = fixture.root.path().join("gog");
    std::fs::write(
        &executable,
        r"#!/usr/bin/env python3
import json,pathlib,sys
root=pathlib.Path(__file__).parent
args=sys.argv[1:]
assert '--account=owner@example.com' in args
assert '--json' in args and '--no-input' in args and '--wrap-untrusted=false' in args
with (root/'calls').open('a') as calls: calls.write(json.dumps(args)+'\n')
state=json.loads((root/'state.json').read_text())
if 'get' in args:
    assert '--readonly' in args
    assert args[args.index('get')+1]=='a1' and args[args.index('--format')+1]=='full'
    full=json.loads((root/'full.json').read_text()); full['message']['labelIds']=state
    print(json.dumps(full))
elif 'list' in args:
    assert '--readonly' in args and 'labels' in args
    print(json.dumps({'labels':json.loads((root/'catalog.json').read_text())}))
else:
    assert '--readonly' not in args
    assert args[4:8]==['gmail','messages','modify','a1']
    assert args[args.index('--add')+1]=='Label_2'
    assert args[args.index('--remove')+1]=='Label_1'
    (root/'state.json').write_text(json.dumps(['INBOX','STARRED','Label_2']))
    print(json.dumps({'modified':'a1'}))
",
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let provider = Gog::new(executable);
    let mut store = fixture.store();
    let preview = store
        .preview_gmail_label(&fixture.context, &fixture.request, &provider)
        .await
        .unwrap();
    store
        .approve_gmail_label(&preview.id, &preview.digest)
        .unwrap();
    assert_eq!(
        store
            .apply_gmail_label(&fixture.context, &preview.id, &provider)
            .await
            .unwrap()
            .state,
        State::Applied
    );
    let calls = std::fs::read_to_string(fixture.root.path().join("calls")).unwrap();
    assert_eq!(
        calls.lines().filter(|line| line.contains("modify")).count(),
        1
    );
}

#[tokio::test]
async fn operator_can_reject_or_revoke_before_execution_without_provider_writes() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let plan = store
        .preview_gmail_label(&fixture.context, &fixture.request, &fixture.provider)
        .await
        .unwrap();
    store.approve_gmail_label(&plan.id, &plan.digest).unwrap();
    assert_eq!(store.gmail_label_actions(20).unwrap().len(), 1);
    assert_eq!(
        store.reject_gmail_label(&plan.id).unwrap().state,
        State::Rejected
    );
    assert!(store
        .apply_gmail_label(&fixture.context, &plan.id, &fixture.provider)
        .await
        .is_err());
    assert!(store.approve_gmail_label(&plan.id, &plan.digest).is_err());
    assert_eq!(*fixture.provider.writes.lock().unwrap(), 0);
}
