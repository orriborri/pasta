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
