//! Deterministic relation extraction.
//!
//! [`relations_from_records`] is a **pure function** of its input records: it
//! reads only explicit record fields, performs no I/O, reads no clock (all
//! timestamps come from the records), uses no randomness, and consults nothing
//! probabilistic — in particular, never embeddings or similarity. Given the same
//! records it returns the same relations, in the same order.
//!
//! Two provenance classes are produced:
//! - `Explicit`: stated by the record's own fields (`AuthoredBy`,
//!   `ParticipatedIn`, `PartOfThread`).
//! - `Rule { name }`: a named deterministic rule over fields (`Mentions` /
//!   `References` from parsed entity mentions).

use kb_core::{Derivation, EntityKind, EntityRef, Kind, Record, Relation, RelationKind};

/// The entity that represents a record itself in the graph, derived from its
/// source/kind. Chat and email messages are represented as their Slack thread /
/// document; commits, issues, and MRs map to their native entity kinds.
fn record_entity(r: &Record) -> EntityRef {
    match r.kind {
        Kind::Commit => EntityRef::new(EntityKind::Commit, native_id(r)),
        Kind::Issue => EntityRef::new(EntityKind::LinearIssue, native_id(r)),
        Kind::Doc => EntityRef::new(EntityKind::Document, native_id(r)),
        Kind::Event => EntityRef::new(EntityKind::Meeting, native_id(r)),
        Kind::Message | Kind::Thread => {
            if r.thread_id.is_empty() {
                EntityRef::new(EntityKind::Document, native_id(r))
            } else {
                EntityRef::new(EntityKind::SlackThread, r.thread_id.clone())
            }
        }
    }
}

/// The native id portion of a record id (`<source>-<native>`), falling back to
/// the whole id when it has no source prefix.
fn native_id(r: &Record) -> String {
    let prefix = format!("{}-", r.source);
    r.id.strip_prefix(&prefix).unwrap_or(&r.id).to_string()
}

/// A parsed person entity from an author/participant identifier, or `None` for
/// empty/placeholder identifiers.
fn person(id: &str) -> Option<EntityRef> {
    let id = id.trim();
    if id.is_empty() || id == "?" || id == "unknown" {
        return None;
    }
    Some(EntityRef::new(EntityKind::Person, id))
}

/// Parse a `Record.entities` mention string into a canonical [`EntityRef`].
///
/// Handles both the canonical form (`linear:AB-123`) and the legacy extract
/// forms produced by `ExtractStage` (`mr:!2935`, where the `!` is stripped).
fn parse_mention(s: &str) -> Option<EntityRef> {
    // Legacy MR mention form: `mr:!2935` (optionally `mr:!2935@project`).
    if let Some(rest) = s.strip_prefix("mr:!") {
        let (id, project) = rest
            .split_once('@')
            .map_or((rest, None), |(i, p)| (i, Some(p.to_string())));
        if id.is_empty() {
            return None;
        }
        return Some(EntityRef {
            kind: EntityKind::MergeRequest,
            id: id.to_string(),
            project,
        });
    }
    // Canonical form for everything else.
    s.parse::<EntityRef>().ok()
}

/// The relation kind for a mention: a commit or merge request *references* a
/// work item; anything else *mentions* it. Pure function of the subject kind.
const fn mention_predicate(subject_kind: EntityKind) -> RelationKind {
    match subject_kind {
        EntityKind::Commit | EntityKind::MergeRequest => RelationKind::References,
        _ => RelationKind::Mentions,
    }
}

/// The rule name for a mention edge, keyed on the mentioned entity's kind.
const fn mention_rule(object_kind: EntityKind) -> &'static str {
    match object_kind {
        EntityKind::LinearIssue => "mention:linear",
        EntityKind::MergeRequest => "mention:mr",
        EntityKind::Commit => "mention:commit",
        _ => "mention:other",
    }
}

/// Derive all relations from a set of records. Pure and deterministic.
#[must_use]
pub fn relations_from_records(records: &[Record]) -> Vec<Relation> {
    let mut out: Vec<Relation> = Vec::new();

    for r in records {
        let subject = record_entity(r);
        let ts = r.created_at;

        // Explicit: authorship.
        if let Some(author) = person(&r.author) {
            if let Ok(rel) = Relation::new(
                subject.clone(),
                RelationKind::AuthoredBy,
                author,
                r.id.clone(),
                Derivation::Explicit,
                ts,
            ) {
                out.push(rel);
            }
        }

        // Explicit: participation.
        for p in &r.participants {
            if let Some(participant) = person(p) {
                if let Ok(rel) = Relation::new(
                    participant,
                    RelationKind::ParticipatedIn,
                    subject.clone(),
                    r.id.clone(),
                    Derivation::Explicit,
                    ts,
                ) {
                    out.push(rel);
                }
            }
        }

        // Explicit: thread membership (only when the record is not itself the thread).
        if !r.thread_id.is_empty() {
            let thread = EntityRef::new(EntityKind::SlackThread, r.thread_id.clone());
            if thread != subject {
                if let Ok(rel) = Relation::new(
                    subject.clone(),
                    RelationKind::PartOfThread,
                    thread,
                    r.id.clone(),
                    Derivation::Explicit,
                    ts,
                ) {
                    out.push(rel);
                }
            }
        }

        // Rule: mentions / references from parsed entity strings.
        for ent in &r.entities {
            let Some(object) = parse_mention(ent) else {
                continue;
            };
            if object == subject {
                continue;
            }
            let predicate = mention_predicate(subject.kind);
            let rule = mention_rule(object.kind);
            if let Ok(rel) = Relation::new(
                subject.clone(),
                predicate,
                object.clone(),
                r.id.clone(),
                Derivation::rule(rule),
                ts,
            ) {
                out.push(rel);
            }
        }
    }

    // Deterministic ordering + dedup by stable key.
    out.sort_by_key(Relation::key);
    out.dedup_by(|a, b| a.key() == b.key());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};
    use kb_core::Source;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn base(id: &str, source: Source, kind: Kind) -> Record {
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

    #[test]
    fn deterministic_across_runs() {
        let mut r = base("git-repo-abc", Source::Git, Kind::Commit);
        r.author = "alice".into();
        r.entities = vec!["linear:AB-123".into(), "mr:!2935".into()];
        let records = vec![r];
        assert_eq!(
            relations_from_records(&records),
            relations_from_records(&records)
        );
    }

    #[test]
    fn every_relation_has_evidence_in_input() {
        let mut r = base("slack-C1-1.0", Source::Slack, Kind::Message);
        r.author = "U123".into();
        r.thread_id = "T9".into();
        r.entities = vec!["linear:AB-1".into()];
        let records = vec![r];
        let ids: std::collections::HashSet<_> = records.iter().map(|r| r.id.clone()).collect();
        let rels = relations_from_records(&records);
        assert!(!rels.is_empty());
        for rel in &rels {
            assert!(ids.contains(&rel.evidence_record_id));
        }
    }

    #[test]
    fn authorship_is_explicit_mention_is_rule() {
        let mut r = base("git-repo-abc", Source::Git, Kind::Commit);
        r.author = "alice".into();
        r.entities = vec!["linear:AB-123".into()];
        let rels = relations_from_records(&[r]);

        let authored = rels
            .iter()
            .find(|x| x.predicate == RelationKind::AuthoredBy)
            .unwrap();
        assert_eq!(authored.derivation, Derivation::Explicit);
        assert_eq!(authored.object, EntityRef::new(EntityKind::Person, "alice"));

        let refs_rel = rels
            .iter()
            .find(|x| x.predicate == RelationKind::References)
            .unwrap();
        assert_eq!(refs_rel.derivation, Derivation::rule("mention:linear"));
        assert_eq!(
            refs_rel.object,
            EntityRef::new(EntityKind::LinearIssue, "AB-123")
        );
    }

    #[test]
    fn commit_references_but_message_mentions() {
        let mut commit = base("git-repo-abc", Source::Git, Kind::Commit);
        commit.entities = vec!["linear:AB-1".into()];
        let commit_rels = relations_from_records(&[commit]);
        assert!(commit_rels
            .iter()
            .any(|r| r.predicate == RelationKind::References));

        let mut msg = base("slack-C1-2.0", Source::Slack, Kind::Message);
        msg.thread_id = "T1".into();
        msg.entities = vec!["linear:AB-1".into()];
        let msg_rels = relations_from_records(&[msg]);
        assert!(msg_rels
            .iter()
            .any(|r| r.predicate == RelationKind::Mentions));
    }

    #[test]
    fn no_relation_between_merely_similar_records() {
        // Two records with similar content but no shared entity, author, thread,
        // or participant. The extractor must produce nothing linking them.
        let mut a = base("slack-C1-1.0", Source::Slack, Kind::Message);
        a.content = "we should double verify the deployment".into();
        let mut b = base("slack-C2-1.0", Source::Slack, Kind::Message);
        b.content = "double verification of the deploy is wise".into();
        let rels = relations_from_records(&[a, b]);
        // No cross-record relation exists (each record's only possible edges would
        // be to its own author/thread, both empty here).
        assert!(rels.is_empty());
    }

    #[test]
    fn legacy_mr_mention_form_parses() {
        assert_eq!(
            parse_mention("mr:!2935"),
            Some(EntityRef::new(EntityKind::MergeRequest, "2935"))
        );
        assert_eq!(
            parse_mention("linear:AB-9"),
            Some(EntityRef::new(EntityKind::LinearIssue, "AB-9"))
        );
    }

    #[test]
    fn participants_produce_participated_in() {
        let mut r = base("slack-C1-1.0", Source::Slack, Kind::Message);
        r.thread_id = "T1".into();
        r.participants = vec!["alice".into(), "bob".into(), "?".into()];
        let rels = relations_from_records(&[r]);
        let parts = rels
            .iter()
            .filter(|x| x.predicate == RelationKind::ParticipatedIn)
            .count();
        assert_eq!(parts, 2); // "?" placeholder skipped
    }
}
