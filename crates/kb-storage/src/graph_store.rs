//! SQLite-backed evidence graph (`graph.db`).
//!
//! This is a **derived index**, a sibling of the Tantivy and `LanceDB` indexes:
//! it is fully rebuildable from Parquet (the source of truth) and safe to
//! delete. It stores entities (keyed by canonical [`EntityRef`] string) and
//! relations (keyed by [`Relation::key`]), each relation retaining its evidence
//! pointer and provenance.

use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Derivation, EntityRef, KbConfig, Record, Relation, RelationKind};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A relation as read back from the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRelation {
    pub subject: EntityRef,
    pub predicate: RelationKind,
    pub object: EntityRef,
    pub evidence_record_id: String,
    pub derivation: Derivation,
    pub created_at: DateTime<Utc>,
}

/// SQLite-backed evidence graph store. Thread-safe via a `Mutex` around the
/// connection, matching the pattern used by `kb-core`'s `SyncState`.
pub struct GraphStore {
    conn: Mutex<Connection>,
}

// SAFETY: Connection is only accessed through the Mutex, ensuring exclusive access.
unsafe impl Sync for GraphStore {}

impl GraphStore {
    /// Open (or create) the graph database at `config.graph_path()`.
    ///
    /// # Errors
    /// Returns error if the database cannot be opened or tables created.
    ///
    /// # Panics
    /// Panics if the graph path has no parent directory.
    pub fn open(config: &KbConfig) -> Result<Self> {
        Self::open_path(&config.graph_path())
    }

    /// Open (or create) the graph database at an explicit path (used by tests).
    ///
    /// # Errors
    /// Returns error if the database cannot be opened or tables created.
    ///
    /// # Panics
    /// Panics if the path has no parent directory.
    pub fn open_path(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Rebuild the entire graph from scratch, transactionally: clear all rows,
    /// insert entities (from record entities and every relation endpoint), then
    /// insert relations. This is the authoritative path used by `kb reindex`.
    ///
    /// # Errors
    /// Returns error if any database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn rebuild(&self, records: &[Record], relations: &[Relation]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM relations", [])?;
        tx.execute("DELETE FROM entities", [])?;

        // Entities from record-declared references + all relation endpoints.
        for r in records {
            for ent in &r.entities {
                if let Ok(e) = ent.parse::<EntityRef>() {
                    insert_entity(&tx, &e)?;
                }
            }
        }
        for rel in relations {
            insert_entity(&tx, &rel.subject)?;
            insert_entity(&tx, &rel.object)?;
        }
        for rel in relations {
            insert_relation(&tx, rel)?;
        }
        tx.commit()?;
        drop(conn);
        Ok(())
    }

    /// Incrementally upsert relations (and their endpoint entities). Idempotent
    /// on the relation key. Used by the sync path between reindexes.
    ///
    /// # Errors
    /// Returns error if any database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn upsert(&self, relations: &[Relation]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for rel in relations {
            insert_entity(&tx, &rel.subject)?;
            insert_entity(&tx, &rel.object)?;
            insert_relation(&tx, rel)?;
        }
        tx.commit()?;
        drop(conn);
        Ok(())
    }

    /// Number of stored entities.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn entity_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
        drop(conn);
        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Number of stored relations.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn relation_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM relations", [], |r| r.get(0))?;
        drop(conn);
        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Relations whose subject is the given entity.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn relations_by_subject(&self, subject: &EntityRef) -> Result<Vec<StoredRelation>> {
        self.query_relations("subject_ref = ?1", &subject.canonical())
    }

    /// Relations whose object is the given entity.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn relations_by_object(&self, object: &EntityRef) -> Result<Vec<StoredRelation>> {
        self.query_relations("object_ref = ?1", &object.canonical())
    }

    /// Relations evidenced by the given record id.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn relations_by_evidence(&self, record_id: &str) -> Result<Vec<StoredRelation>> {
        self.query_relations("evidence_record_id = ?1", record_id)
    }

    /// Whether an entity is present in the graph.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn has_entity(&self, entity: &EntityRef) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE ref = ?1",
            [entity.canonical()],
            |r| r.get(0),
        )?;
        drop(conn);
        Ok(n > 0)
    }

    /// All relations incident to an entity (as subject or object), deduped by
    /// relation key and ordered deterministically by key.
    ///
    /// # Errors
    /// Returns error if the query fails.
    pub fn relations_incident(&self, entity: &EntityRef) -> Result<Vec<StoredRelation>> {
        let mut out = self.relations_by_subject(entity)?;
        out.extend(self.relations_by_object(entity)?);
        // Dedup by identity: subject+predicate+object+evidence+derivation.
        out.sort_by(|a, b| {
            incident_key(a).cmp(&incident_key(b))
        });
        out.dedup_by(|a, b| incident_key(a) == incident_key(b));
        Ok(out)
    }

    /// The `(out_degree, in_degree)` of an entity: relations where it is the
    /// subject, and where it is the object.
    ///
    /// # Errors
    /// Returns error if the query fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn entity_degree(&self, entity: &EntityRef) -> Result<(usize, usize)> {
        let conn = self.conn.lock().unwrap();
        let out: i64 = conn.query_row(
            "SELECT COUNT(*) FROM relations WHERE subject_ref = ?1",
            [entity.canonical()],
            |r| r.get(0),
        )?;
        let incoming: i64 = conn.query_row(
            "SELECT COUNT(*) FROM relations WHERE object_ref = ?1",
            [entity.canonical()],
            |r| r.get(0),
        )?;
        drop(conn);
        Ok((usize::try_from(out).unwrap_or(0), usize::try_from(incoming).unwrap_or(0)))
    }

    fn query_relations(&self, where_clause: &str, param: &str) -> Result<Vec<StoredRelation>> {
        let sql = format!(
            "SELECT subject_ref, predicate, object_ref, evidence_record_id, derivation, created_at
             FROM relations WHERE {where_clause} ORDER BY key"
        );
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<(String, String, String, String, String, String)> = stmt
            .query_map([param], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        drop(conn);

        let mut out = Vec::with_capacity(rows.len());
        for (subject, predicate, object, evidence, derivation, created) in rows {
            out.push(StoredRelation {
                subject: subject.parse().map_err(to_sqlite_err)?,
                predicate: RelationKind::from_tag(&predicate)
                    .ok_or_else(|| anyhow::anyhow!("bad predicate {predicate}"))?,
                object: object.parse().map_err(to_sqlite_err)?,
                evidence_record_id: evidence,
                derivation: Derivation::from_canonical(&derivation)
                    .ok_or_else(|| anyhow::anyhow!("bad derivation {derivation}"))?,
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |d| d.with_timezone(&Utc)),
            });
        }
        Ok(out)
    }
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS entities (
        ref     TEXT PRIMARY KEY,
        kind    TEXT NOT NULL,
        id      TEXT NOT NULL,
        project TEXT
    );
    CREATE TABLE IF NOT EXISTS relations (
        key                TEXT PRIMARY KEY,
        subject_ref        TEXT NOT NULL,
        predicate          TEXT NOT NULL,
        object_ref         TEXT NOT NULL,
        evidence_record_id TEXT NOT NULL,
        derivation         TEXT NOT NULL,
        created_at         TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_rel_subject  ON relations(subject_ref);
    CREATE INDEX IF NOT EXISTS idx_rel_object   ON relations(object_ref);
    CREATE INDEX IF NOT EXISTS idx_rel_evidence ON relations(evidence_record_id);
";

fn insert_entity(tx: &rusqlite::Transaction<'_>, e: &EntityRef) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO entities (ref, kind, id, project) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![e.canonical(), e.kind.as_tag(), e.id, e.project],
    )?;
    Ok(())
}

fn insert_relation(tx: &rusqlite::Transaction<'_>, rel: &Relation) -> Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO relations
         (key, subject_ref, predicate, object_ref, evidence_record_id, derivation, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            rel.key(),
            rel.subject.canonical(),
            rel.predicate.as_tag(),
            rel.object.canonical(),
            rel.evidence_record_id,
            rel.derivation.canonical(),
            rel.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn to_sqlite_err<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// Identity tuple used to dedup incident relations deterministically.
fn incident_key(r: &StoredRelation) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        r.subject.canonical(),
        r.predicate.as_tag(),
        r.object.canonical(),
        r.evidence_record_id,
        r.derivation.canonical(),
    )
}

/// Delete the graph database file if it exists (used before a full rebuild).
///
/// # Errors
/// Returns error if the file exists but cannot be removed.
pub fn remove_graph_db(config: &KbConfig) -> Result<()> {
    remove_path(&config.graph_path())
}

fn remove_path(path: &PathBuf) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use kb_core::EntityKind;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn rel(
        subj: EntityRef,
        pred: RelationKind,
        obj: EntityRef,
        ev: &str,
        d: Derivation,
    ) -> Relation {
        Relation::new(subj, pred, obj, ev, d, ts()).unwrap()
    }

    fn temp_store() -> (GraphStore, tempdir::Guard) {
        let guard = tempdir::Guard::new();
        let store = GraphStore::open_path(&guard.path.join("graph.db")).unwrap();
        (store, guard)
    }

    #[test]
    fn rebuild_stores_entities_and_relations() {
        let (store, _g) = temp_store();
        let commit = EntityRef::new(EntityKind::Commit, "abc123");
        let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
        let relations = vec![rel(
            commit.clone(),
            RelationKind::References,
            issue.clone(),
            "git-repo-abc123",
            Derivation::rule("mention:linear"),
        )];
        store.rebuild(&[], &relations).unwrap();
        assert_eq!(store.relation_count().unwrap(), 1);
        assert_eq!(store.entity_count().unwrap(), 2);
        assert!(store.has_entity(&commit).unwrap());
        assert!(store.has_entity(&issue).unwrap());
    }

    #[test]
    fn endpoint_without_backing_record_is_stored() {
        let (store, _g) = temp_store();
        // The object issue has no Record at all; it must still be stored as an
        // entity, with the relation evidenced by the mentioning record.
        let msg = EntityRef::new(EntityKind::SlackThread, "T1");
        let issue = EntityRef::new(EntityKind::LinearIssue, "ZZ-9");
        let relations = vec![rel(
            msg,
            RelationKind::Mentions,
            issue.clone(),
            "slack-C1-1.0",
            Derivation::rule("mention:linear"),
        )];
        store.rebuild(&[], &relations).unwrap();
        assert!(store.has_entity(&issue).unwrap());
        let by_ev = store.relations_by_evidence("slack-C1-1.0").unwrap();
        assert_eq!(by_ev.len(), 1);
        assert_eq!(by_ev[0].object, issue);
    }

    #[test]
    fn upsert_is_idempotent() {
        let (store, _g) = temp_store();
        let r = rel(
            EntityRef::new(EntityKind::Commit, "abc"),
            RelationKind::References,
            EntityRef::new(EntityKind::LinearIssue, "AB-1"),
            "git-repo-abc",
            Derivation::rule("mention:linear"),
        );
        store.upsert(std::slice::from_ref(&r)).unwrap();
        store.upsert(std::slice::from_ref(&r)).unwrap();
        assert_eq!(store.relation_count().unwrap(), 1);
    }

    #[test]
    fn query_by_subject_and_object() {
        let (store, _g) = temp_store();
        let commit = EntityRef::new(EntityKind::Commit, "abc");
        let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
        store
            .upsert(&[rel(
                commit.clone(),
                RelationKind::References,
                issue.clone(),
                "git-repo-abc",
                Derivation::rule("mention:linear"),
            )])
            .unwrap();
        assert_eq!(store.relations_by_subject(&commit).unwrap().len(), 1);
        assert_eq!(store.relations_by_object(&issue).unwrap().len(), 1);
        assert_eq!(store.relations_by_subject(&issue).unwrap().len(), 0);
    }

    #[test]
    fn incident_and_degree() {
        let (store, _g) = temp_store();
        let person = EntityRef::new(EntityKind::Person, "alice");
        let commit = EntityRef::new(EntityKind::Commit, "abc");
        let issue = EntityRef::new(EntityKind::LinearIssue, "AB-1");
        // commit -> issue (commit subject), commit -> alice (commit subject).
        store
            .upsert(&[
                rel(commit.clone(), RelationKind::References, issue.clone(), "git-repo-abc", Derivation::rule("mention:linear")),
                rel(commit.clone(), RelationKind::AuthoredBy, person.clone(), "git-repo-abc", Derivation::Explicit),
            ])
            .unwrap();

        // commit is subject of 2 relations, object of none.
        assert_eq!(store.entity_degree(&commit).unwrap(), (2, 0));
        // issue is object of 1.
        assert_eq!(store.entity_degree(&issue).unwrap(), (0, 1));
        // incident to commit = 2; incident to issue = 1; incident to alice = 1.
        assert_eq!(store.relations_incident(&commit).unwrap().len(), 2);
        assert_eq!(store.relations_incident(&issue).unwrap().len(), 1);
        assert_eq!(store.relations_incident(&person).unwrap().len(), 1);
    }

    /// Minimal temp-dir helper (no external dev-dependency needed).
    mod tempdir {
        use std::path::PathBuf;
        pub struct Guard {
            pub path: PathBuf,
        }
        impl Guard {
            pub fn new() -> Self {
                let mut path = std::env::temp_dir();
                let uniq = format!(
                    "kb-graph-test-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                );
                path.push(uniq);
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }
}
