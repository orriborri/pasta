//! Typed, provenance-bearing relations for the evidence graph.
//!
//! Every [`Relation`] names the record that is its evidence
//! (`evidence_record_id`) and records how it was produced via [`Derivation`].
//! Relations are only ever *explicit* (stated by a record's own fields) or
//! *deterministically derived* (produced by a named, pure rule). Nothing here
//! is ever created from semantic similarity.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::entity::EntityRef;

/// The typed predicate connecting two entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// Subject references an object work item (commit/MR → issue/MR).
    References,
    /// Subject record was authored by the object person.
    AuthoredBy,
    /// Object person participated in the subject record.
    ParticipatedIn,
    /// Subject record is part of the object thread.
    PartOfThread,
    /// Subject record mentions the object entity (chat/email mention).
    Mentions,
    /// Generic source-native link present in the record.
    LinkedTo,
}

impl RelationKind {
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::AuthoredBy => "authored_by",
            Self::ParticipatedIn => "participated_in",
            Self::PartOfThread => "part_of_thread",
            Self::Mentions => "mentions",
            Self::LinkedTo => "linked_to",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "references" => Self::References,
            "authored_by" => Self::AuthoredBy,
            "participated_in" => Self::ParticipatedIn,
            "part_of_thread" => Self::PartOfThread,
            "mentions" => Self::Mentions,
            "linked_to" => Self::LinkedTo,
            _ => return None,
        })
    }
}

/// How a relation was produced. This is the provenance that keeps the graph
/// honest: a relation is either stated by the source record (`Explicit`) or
/// produced by a named deterministic rule (`Rule`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Derivation {
    /// Stated directly by the evidence record's own fields.
    Explicit,
    /// Produced by a named, deterministic extraction rule.
    Rule { name: String },
}

impl Derivation {
    /// Named rule derivation from a static rule name.
    #[must_use]
    pub fn rule(name: &str) -> Self {
        Self::Rule {
            name: name.to_string(),
        }
    }

    /// Canonical string form: `"explicit"` or `"rule:<name>"`.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::Explicit => "explicit".to_string(),
            Self::Rule { name } => format!("rule:{name}"),
        }
    }

    /// Parse the canonical string form.
    #[must_use]
    pub fn from_canonical(s: &str) -> Option<Self> {
        if s == "explicit" {
            Some(Self::Explicit)
        } else {
            s.strip_prefix("rule:").map(Self::rule)
        }
    }
}

/// A typed, evidence-backed edge between two entities.
///
/// Invariant: `evidence_record_id` is non-empty and identifies the record that
/// evidences this relation. Construct via [`Relation::new`], which enforces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub subject: EntityRef,
    pub predicate: RelationKind,
    pub object: EntityRef,
    pub evidence_record_id: String,
    pub derivation: Derivation,
    pub created_at: DateTime<Utc>,
}

impl Relation {
    /// Construct a relation.
    ///
    /// # Errors
    /// Returns `Err` if `evidence_record_id` is empty — a relation must always
    /// name the record that evidences it.
    pub fn new(
        subject: EntityRef,
        predicate: RelationKind,
        object: EntityRef,
        evidence_record_id: impl Into<String>,
        derivation: Derivation,
        created_at: DateTime<Utc>,
    ) -> Result<Self, RelationError> {
        let evidence_record_id = evidence_record_id.into();
        if evidence_record_id.is_empty() {
            return Err(RelationError::MissingEvidence);
        }
        Ok(Self {
            subject,
            predicate,
            object,
            evidence_record_id,
            derivation,
            created_at,
        })
    }

    /// Stable identity key: a SHA-256 over the identifying fields (subject,
    /// predicate, object, evidence record id, derivation). Timestamp is excluded
    /// so re-derivation with the same evidence is idempotent. This is the graph's
    /// primary key.
    #[must_use]
    pub fn key(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.subject.canonical().as_bytes());
        h.update(b"|");
        h.update(self.predicate.as_tag().as_bytes());
        h.update(b"|");
        h.update(self.object.canonical().as_bytes());
        h.update(b"|");
        h.update(self.evidence_record_id.as_bytes());
        h.update(b"|");
        h.update(self.derivation.canonical().as_bytes());
        format!("{:x}", h.finalize())
    }
}

/// Error constructing a [`Relation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationError {
    /// `evidence_record_id` was empty.
    MissingEvidence,
}

impl std::fmt::Display for RelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEvidence => {
                f.write_str("relation requires a non-empty evidence_record_id")
            }
        }
    }
}

impl std::error::Error for RelationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn requires_evidence() {
        let r = Relation::new(
            EntityRef::new(EntityKind::Commit, "abc"),
            RelationKind::References,
            EntityRef::new(EntityKind::LinearIssue, "AB-1"),
            "",
            Derivation::rule("mention:linear"),
            ts(),
        );
        assert_eq!(r, Err(RelationError::MissingEvidence));
    }

    #[test]
    fn key_is_stable_and_ignores_timestamp() {
        let mk = |t| {
            Relation::new(
                EntityRef::new(EntityKind::Commit, "abc"),
                RelationKind::References,
                EntityRef::new(EntityKind::LinearIssue, "AB-1"),
                "git-repo-abc",
                Derivation::rule("mention:linear"),
                t,
            )
            .unwrap()
        };
        let a = mk(ts());
        let b = mk(Utc::now());
        assert_eq!(a.key(), b.key());
    }

    #[test]
    fn key_changes_with_derivation() {
        let base = |d| {
            Relation::new(
                EntityRef::new(EntityKind::Commit, "abc"),
                RelationKind::References,
                EntityRef::new(EntityKind::LinearIssue, "AB-1"),
                "git-repo-abc",
                d,
                ts(),
            )
            .unwrap()
        };
        assert_ne!(
            base(Derivation::Explicit).key(),
            base(Derivation::rule("mention:linear")).key()
        );
    }

    #[test]
    fn derivation_round_trip() {
        assert_eq!(
            Derivation::from_canonical("explicit"),
            Some(Derivation::Explicit)
        );
        assert_eq!(
            Derivation::from_canonical("rule:mention:linear"),
            Some(Derivation::rule("mention:linear"))
        );
        assert_eq!(Derivation::Explicit.canonical(), "explicit");
        assert_eq!(Derivation::rule("x").canonical(), "rule:x");
    }
}
