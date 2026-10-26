//! Integration tests for the structured query layer over a temp Parquet +
//! graph derived from a fixed record fixture.

use chrono::{DateTime, TimeZone, Utc};
use kb_core::{EntityKind, EntityRef, KbConfig, Kind, Record, RelationKind, Source};
use kb_storage::{GraphStore, ParquetStore};

fn at(y: i32, mo: u32, d: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, 12, 0, 0).unwrap()
}

fn rec(id: &str, source: Source, kind: Kind, created: DateTime<Utc>) -> Record {
    Record {
        id: id.to_string(),
        source,
        kind,
        title: format!("title {id}"),
        content: format!("content of {id}"),
        author: String::new(),
        participants: vec![],
        created_at: created,
        updated_at: created,
        url: format!("https://example/{id}"),
        thread_id: String::new(),
        entities: vec![],
        tags: vec![],
    }
}

struct TempConfig {
    dir: std::path::PathBuf,
    config: KbConfig,
}
impl TempConfig {
    fn new() -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "kb-query-it-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let config = KbConfig {
            data_dir: dir.clone(),
        };
        Self { dir, config }
    }
}
impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Build a fixture: a commit (authored by alice, references AB-1) and a Slack
/// message (in thread T1, mentions AB-1). AB-1 has no record of its own.
fn setup() -> TempConfig {
    let tc = TempConfig::new();

    let mut commit = rec("git-repo-abc", Source::Git, Kind::Commit, at(2026, 1, 1));
    commit.author = "alice".into();
    commit.entities = vec!["linear:AB-1".into()];

    let mut msg = rec("slack-C1-1.0", Source::Slack, Kind::Message, at(2026, 2, 1));
    msg.author = "bob".into();
    msg.thread_id = "T1".into();
    msg.entities = vec!["linear:AB-1".into()];

    let records = vec![commit, msg];

    // Write source of truth + rebuild the derived graph.
    ParquetStore::new(&tc.config).write(&records).unwrap();
    let relations = kb_pipeline_relations(&records);
    let graph = GraphStore::open(&tc.config).unwrap();
    graph.rebuild(&records, &relations).unwrap();

    tc
}

// The relation extractor lives in kb-pipeline; import via a thin wrapper so this
// test crate does not need the dependency name inline everywhere.
fn kb_pipeline_relations(records: &[Record]) -> Vec<kb_core::Relation> {
    kb_pipeline::relations_from_records(records)
}

#[test]
fn get_entity_present_and_absent() {
    let tc = setup();
    // The commit has a backing record.
    let commit = EntityRef::new(EntityKind::Commit, "repo-abc");
    let cv = kb_query::get_entity(&tc.config, &commit).unwrap();
    assert!(cv.present, "commit record exists");
    assert!(cv.out_degree >= 1);

    // AB-1 is only a relation endpoint; no record.
    let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
    let iv = kb_query::get_entity(&tc.config, &issue).unwrap();
    assert!(!iv.present, "issue has no backing record");
    assert!(iv.in_degree >= 1, "issue is referenced/mentioned");
}

#[test]
fn get_related_directions_filter_and_determinism() {
    let tc = setup();
    let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");

    let all = kb_query::get_related(&tc.config, &issue, None, 50).unwrap();
    // AB-1 is the object of a References (from commit) and a Mentions (from msg).
    assert!(all.relations.len() >= 2);

    let refs =
        kb_query::get_related(&tc.config, &issue, Some(RelationKind::References), 50).unwrap();
    assert!(refs.relations.iter().all(|r| r.predicate == "references"));
    assert!(!refs.relations.is_empty());

    // Determinism.
    let again = kb_query::get_related(&tc.config, &issue, None, 50).unwrap();
    assert_eq!(all, again);
}

#[test]
fn get_evidence_present_and_absent() {
    let tc = setup();
    let present = kb_query::get_evidence(&tc.config, &["git-repo-abc"]).unwrap();
    assert_eq!(present.len(), 1);
    assert_eq!(present[0].record_id, "git-repo-abc");
    assert!(!present[0].title.is_empty());

    let absent = kb_query::get_evidence(&tc.config, &["does-not-exist"]).unwrap();
    assert!(absent.is_empty(), "absent evidence is not fabricated");
}

#[test]
fn get_context_is_grounded() {
    let tc = setup();
    let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
    let ctx = kb_query::get_context(&tc.config, &issue, 50).unwrap();

    assert!(!ctx.relations.is_empty());
    // Every relation traces to an evidence id.
    for r in &ctx.relations {
        assert!(!r.evidence_record_id.is_empty());
    }
    // Every evidence view corresponds to a real record id from the fixture.
    let known: std::collections::HashSet<&str> =
        ["git-repo-abc", "slack-C1-1.0"].into_iter().collect();
    for e in &ctx.evidence {
        assert!(known.contains(e.record_id.as_str()));
    }
}

#[test]
fn get_timeline_is_ordered_and_deterministic() {
    let tc = setup();
    let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
    let tl = kb_query::get_timeline(&tc.config, &issue, 50).unwrap();
    assert_eq!(tl.len(), 2);
    // Commit (Jan) before message (Feb).
    assert_eq!(tl[0].record.record_id, "git-repo-abc");
    assert_eq!(tl[1].record.record_id, "slack-C1-1.0");
    assert!(tl[0].at <= tl[1].at);

    let again = kb_query::get_timeline(&tc.config, &issue, 50).unwrap();
    assert_eq!(tl, again);
}

#[test]
fn get_changes_pages_stably_with_opaque_cursor() {
    let tc = setup();

    let first = kb_query::get_changes(&tc.config, None, None, 1).unwrap();
    assert_eq!(first.records.len(), 1);
    assert!(first.has_more);
    assert_eq!(first.records[0].record_id, "git-repo-abc");
    let cursor = first.next_cursor.clone().expect("first page has cursor");

    let second = kb_query::get_changes(&tc.config, Some(&cursor), None, 10).unwrap();
    assert_eq!(second.records.len(), 1);
    assert!(!second.has_more);
    assert_eq!(second.records[0].record_id, "slack-C1-1.0");
    assert_ne!(second.next_cursor.as_deref(), Some(cursor.as_str()));

    let empty = kb_query::get_changes(
        &tc.config,
        second.next_cursor.as_deref(),
        Some("git"),
        10,
    )
    .unwrap();
    assert!(empty.records.is_empty());
    assert_eq!(empty.next_cursor, second.next_cursor);
}

#[test]
fn get_changes_pages_by_ingestion_not_source_time() {
    let tc = TempConfig::new();
    let store = ParquetStore::new(&tc.config);
    let future_event = rec("calendar-future", Source::Calendar, Kind::Event, at(2099, 1, 1));
    store.write_at(&[future_event], at(2026, 9, 1)).unwrap();

    let first = kb_query::get_changes(&tc.config, None, None, 10).unwrap();
    assert_eq!(first.records.len(), 1);
    let cursor = first.next_cursor.unwrap();

    // Fetched later, but dated long before the cursor's record.
    let late = rec("git-old-commit", Source::Git, Kind::Commit, at(2020, 1, 1));
    store.write_at(&[late], at(2026, 9, 2)).unwrap();

    let second = kb_query::get_changes(&tc.config, Some(&cursor), None, 10).unwrap();
    let ids: Vec<_> = second.records.iter().map(|r| r.record_id.as_str()).collect();
    assert_eq!(ids, ["git-old-commit"]);
    assert!(second.records[0].ingested_at.starts_with("2026-09-02"));
}

#[test]
fn get_changes_reports_newest_ingested_version() {
    let tc = TempConfig::new();
    let store = ParquetStore::new(&tc.config);
    let mut v1 = rec("linear-AB-1", Source::Linear, Kind::Issue, at(2026, 1, 1));
    store.write_at(&[v1.clone()], at(2026, 9, 1)).unwrap();
    v1.title = "renamed".into();
    store.write_at(&[v1], at(2026, 9, 1) + chrono::Duration::milliseconds(1)).unwrap();

    let page = kb_query::get_changes(&tc.config, None, None, 10).unwrap();
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].title, "renamed");
}

#[test]
fn get_changes_rejects_malformed_cursor() {
    let tc = setup();
    assert!(kb_query::get_changes(&tc.config, Some("not-a-cursor"), None, 10).is_err());
}

#[test]
fn late_arrival_and_same_timestamp_edit_are_visible_after_cursor() {
    let tc = setup();
    let first = kb_query::get_changes(&tc.config, None, None, 100).unwrap();
    let mut old = rec("git-late", Source::Git, Kind::Commit, at(2020, 1, 1));
    let parquet = ParquetStore::new(&tc.config);
    parquet.write(std::slice::from_ref(&old)).unwrap();
    let next = kb_query::get_changes(&tc.config, first.next_cursor.as_deref(), None, 100).unwrap();
    assert_eq!(next.records.len(), 1);
    assert_eq!(next.records[0].record_id, old.id);
    old.content = "Edited without changing source timestamp".into();
    parquet.write(&[old]).unwrap();
    let edited = kb_query::get_changes(&tc.config, next.next_cursor.as_deref(), None, 100).unwrap();
    assert_eq!(edited.records.len(), 1);
    let evidence = kb_query::get_evidence(&tc.config, &["git-late"]).unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].content, "Edited without changing source timestamp");
}

#[test]
fn assistant_capture_restart_import_retrieve_and_search_preserves_versions() {
    let temp = TempConfig::new();
    let path = temp.dir.join("assistant.sqlite");
    let store = pasta_assistant::Store::open(&path).unwrap();
    let id = store.capture("telegram", "owner/chat/message", "Call the electrician").unwrap();
    store.capture("telegram", "owner/chat/message", "Call the plumber instead").unwrap();
    drop(store);
    let records = kb_fetchers::assistant::fetch(&path).unwrap();
    let parquet = ParquetStore::new(&temp.config);
    parquet.write_changed(&records).unwrap();
    parquet.write_changed(&records).unwrap();
    assert_eq!(parquet.read_all_ingested().unwrap().len(), 2);
    let record_id = format!("vault-assistant-{id}");
    let evidence = kb_query::get_evidence(&temp.config, &[&record_id]).unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].content, "Call the electrician");
    assert_eq!(evidence[0].url, format!("assistant:evidence:{id}"));
    let text = kb_storage::TextIndex::open(&temp.config).unwrap();
    text.upsert(&records).unwrap();
    assert!(!text.search("electrician", 5).unwrap().is_empty());
    let page = kb_query::get_changes(&temp.config, None, Some("vault"), 10).unwrap();
    assert_eq!(page.records.len(), 2);
    assert!(page.records.iter().all(|r| !r.ingested_at.is_empty()));
}

#[cfg(unix)]
#[tokio::test]
async fn gmail_attachment_originals_rebuild_search_after_projection_loss() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempConfig::new();
    let executable = temp.dir.join("gog");
    std::fs::write(&executable, r"#!/usr/bin/env python3
import json, sys, base64
if 'search' in sys.argv:
    print(json.dumps({'messages':[{'id':'a1'}]}))
else:
    assert 'get' in sys.argv
    data = b'call electrician'
    print(json.dumps({'headers':{'subject':'Original mail'},'body':'Retained original message',
      'message':{'id':'a1','threadId':'a1','internalDate':'1793012400000',
        'payload':{'mimeType':'multipart/mixed','parts':[
          {'partId':'1','filename':'../notes.txt','mimeType':'text/plain','body':{'size':len(data),'data':base64.urlsafe_b64encode(data).decode()}}]}}}))
").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let fetcher = kb_fetchers::gmail::GmailFetcher::new().with_account("owner@example.com")
        .with_gog_executable(&executable).with_evidence_dir(temp.dir.join("originals"));
    let config = KbConfig { data_dir: temp.dir.join("indexes") };
    let state = kb_core::SyncState::open_staged(&config).unwrap();
    let records = fetcher.fetch(&state).await.unwrap();
    assert_eq!(records.len(), 2);
    let attachment = records.iter().find(|record| record.kind == Kind::Doc).unwrap();
    let attachment_id = attachment.id.clone();
    let expected_content = attachment.content.clone();
    ParquetStore::new(&config).write_changed(&records).unwrap();
    state.commit_windows().unwrap();
    drop(state);
    std::fs::remove_dir_all(&config.data_dir).unwrap();
    // No provider reads are possible now; only retained originals remain.
    std::fs::remove_file(executable).unwrap();
    let replay = fetcher.retained_records().unwrap();
    ParquetStore::new(&config).write_changed(&replay).unwrap();
    let text = kb_storage::TextIndex::open(&config).unwrap();
    text.upsert(&replay).unwrap();
    assert!(!text.search("electrician", 5).unwrap().is_empty());
    let evidence = kb_query::get_evidence(&config, &[&attachment_id]).unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].content, expected_content);
    assert!(!temp.dir.join("notes.txt").exists());
}
