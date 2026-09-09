//! Integration tests for the evidence graph derived index: rebuild from records,
//! determinism/reproducibility, and endpoint-without-backing-record storage.

use chrono::{DateTime, TimeZone, Utc};
use kb_core::{EntityKind, EntityRef, Kind, Record, Source};
use kb_pipeline::relations_from_records;
use kb_storage::GraphStore;

fn ts() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

fn rec(id: &str, source: Source, kind: Kind) -> Record {
    Record {
        id: id.to_string(),
        source,
        kind,
        title: String::new(),
        content: String::new(),
        author: String::new(),
        participants: vec![],
        created_at: ts(),
        updated_at: ts(),
        url: String::new(),
        thread_id: String::new(),
        entities: vec![],
        tags: vec![],
    }
}

fn fixture() -> Vec<Record> {
    // A commit authored by alice referencing a Linear ticket that has no record.
    let mut commit = rec("git-repo-abc123", Source::Git, Kind::Commit);
    commit.author = "alice".into();
    commit.entities = vec!["linear:AB-123".into()];

    // A Slack message in a thread, authored by bob, mentioning the same ticket.
    let mut msg = rec("slack-C1-1.0", Source::Slack, Kind::Message);
    msg.author = "bob".into();
    msg.thread_id = "T9".into();
    msg.participants = vec!["alice".into()];
    msg.entities = vec!["linear:AB-123".into()];

    vec![commit, msg]
}

struct TempDir {
    path: std::path::PathBuf,
}
impl TempDir {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "kb-graph-it-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
    fn db(&self, name: &str) -> std::path::PathBuf {
        self.path.join(name)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn rebuild_from_records_produces_expected_graph() {
    let dir = TempDir::new();
    let records = fixture();
    let relations = relations_from_records(&records);

    let store = GraphStore::open_path(&dir.db("graph.db")).unwrap();
    store.rebuild(&records, &relations).unwrap();

    // The mentioned ticket, both people, the commit, and the thread are entities.
    let issue = EntityRef::new(EntityKind::LinearIssue, "AB-123");
    let commit = EntityRef::new(EntityKind::Commit, "repo-abc123");
    let thread = EntityRef::new(EntityKind::SlackThread, "T9");
    assert!(store.has_entity(&issue).unwrap());
    assert!(store.has_entity(&commit).unwrap());
    assert!(store.has_entity(&thread).unwrap());
    assert!(store
        .has_entity(&EntityRef::new(EntityKind::Person, "alice"))
        .unwrap());
    assert!(store
        .has_entity(&EntityRef::new(EntityKind::Person, "bob"))
        .unwrap());

    // The commit -> issue edge is evidenced by the commit record.
    let by_commit = store.relations_by_subject(&commit).unwrap();
    assert!(by_commit
        .iter()
        .any(|r| r.object == issue && r.evidence_record_id == "git-repo-abc123"));

    // Every stored relation references a record present in the fixture.
    let ids: std::collections::HashSet<_> = records.iter().map(|r| r.id.clone()).collect();
    for e in &["git-repo-abc123", "slack-C1-1.0"] {
        for r in store.relations_by_evidence(e).unwrap() {
            assert!(ids.contains(&r.evidence_record_id));
        }
    }
}

#[test]
fn rebuild_is_reproducible() {
    let records = fixture();
    let relations = relations_from_records(&records);

    let dir_a = TempDir::new();
    let a = GraphStore::open_path(&dir_a.db("a.db")).unwrap();
    a.rebuild(&records, &relations).unwrap();

    let dir_b = TempDir::new();
    let b = GraphStore::open_path(&dir_b.db("b.db")).unwrap();
    b.rebuild(&records, &relations).unwrap();

    assert_eq!(a.entity_count().unwrap(), b.entity_count().unwrap());
    assert_eq!(a.relation_count().unwrap(), b.relation_count().unwrap());

    // Deleting and rebuilding the same store yields identical counts.
    let before_e = a.entity_count().unwrap();
    let before_r = a.relation_count().unwrap();
    a.rebuild(&records, &relations).unwrap();
    assert_eq!(a.entity_count().unwrap(), before_e);
    assert_eq!(a.relation_count().unwrap(), before_r);
}

#[test]
fn endpoint_without_backing_record_is_stored_with_evidence() {
    let dir = TempDir::new();
    // Only the mentioning message exists; the referenced ticket has no record.
    let mut msg = rec("slack-C1-9.0", Source::Slack, Kind::Message);
    msg.thread_id = "T1".into();
    msg.entities = vec!["linear:ZZ-999".into()];
    let records = vec![msg];
    let relations = relations_from_records(&records);

    let store = GraphStore::open_path(&dir.db("graph.db")).unwrap();
    store.rebuild(&records, &relations).unwrap();

    let issue = EntityRef::new(EntityKind::LinearIssue, "ZZ-999");
    assert!(
        store.has_entity(&issue).unwrap(),
        "endpoint entity stored even without a record"
    );

    let mentions = store.relations_by_object(&issue).unwrap();
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].evidence_record_id, "slack-C1-9.0");
}
