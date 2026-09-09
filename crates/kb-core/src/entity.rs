//! Canonical typed entity reference for the evidence graph.
//!
//! An [`EntityRef`] is the single representation of "a thing the knowledge base
//! knows about" — a person, a Linear issue, a merge request, and so on — shared
//! across extraction, storage, and resolution.
//!
//! `EntityRef` has a lossless canonical string form (`kind:id` with an optional
//! `@project` suffix) and round-trips through it via [`std::str::FromStr`], so it
//! can be stored in `Record.entities`, persisted in the graph, and reconstructed
//! without loss.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The type of an entity in the evidence graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,
    LinearIssue,
    MergeRequest,
    Commit,
    Document,
    Meeting,
    SlackThread,
    Project,
}

impl EntityKind {
    /// Canonical lowercase tag used in the string form.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::LinearIssue => "linear",
            Self::MergeRequest => "mr",
            Self::Commit => "commit",
            Self::Document => "document",
            Self::Meeting => "meeting",
            Self::SlackThread => "thread",
            Self::Project => "project",
        }
    }

    /// Parse a canonical tag back into a kind.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "person" => Self::Person,
            "linear" => Self::LinearIssue,
            "mr" => Self::MergeRequest,
            "commit" => Self::Commit,
            "document" => Self::Document,
            "meeting" => Self::Meeting,
            "thread" => Self::SlackThread,
            "project" => Self::Project,
            _ => return None,
        })
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_tag())
    }
}

/// A canonical, typed reference to an entity.
///
/// The canonical string form is `"<kind>:<id>"`, with an optional `"@<project>"`
/// suffix when project context is present, e.g. `"linear:AB-123"` or
/// `"mr:2935@group/repo"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: String,
    pub project: Option<String>,
}

impl EntityRef {
    /// Construct a reference without project context.
    #[must_use]
    pub fn new(kind: EntityKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            project: None,
        }
    }

    /// Construct a reference with project context.
    #[must_use]
    pub fn with_project(
        kind: EntityKind,
        id: impl Into<String>,
        project: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            project: Some(project.into()),
        }
    }

    /// The canonical string form used for storage and round-tripping.
    #[must_use]
    pub fn canonical(&self) -> String {
        self.project.as_ref().map_or_else(
            || format!("{}:{}", self.kind.as_tag(), self.id),
            |p| format!("{}:{}@{}", self.kind.as_tag(), self.id, p),
        )
    }
}

impl fmt::Display for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

/// Error returned when a string cannot be parsed as an [`EntityRef`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEntityRefError(pub String);

impl fmt::Display for ParseEntityRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid EntityRef: {}", self.0)
    }
}

impl std::error::Error for ParseEntityRefError {}

impl FromStr for EntityRef {
    type Err = ParseEntityRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Split kind off the front; the id may itself contain ':' (rare) so only
        // split once. Project is an optional '@<project>' suffix on the remainder.
        let (tag, rest) = s
            .split_once(':')
            .ok_or_else(|| ParseEntityRefError(s.to_string()))?;
        let kind = EntityKind::from_tag(tag).ok_or_else(|| ParseEntityRefError(s.to_string()))?;
        let (id, project) = match rest.split_once('@') {
            Some((id, proj)) if !proj.is_empty() => (id, Some(proj.to_string())),
            _ => (rest, None),
        };
        if id.is_empty() {
            return Err(ParseEntityRefError(s.to_string()));
        }
        Ok(Self {
            kind,
            id: id.to_string(),
            project,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_without_project() {
        let e = EntityRef::new(EntityKind::LinearIssue, "AB-123");
        let s = e.canonical();
        assert_eq!(s, "linear:AB-123");
        assert_eq!(EntityRef::from_str(&s).unwrap(), e);
    }

    #[test]
    fn round_trip_with_project() {
        let e = EntityRef::with_project(EntityKind::MergeRequest, "2935", "group/repo");
        let s = e.canonical();
        assert_eq!(s, "mr:2935@group/repo");
        assert_eq!(EntityRef::from_str(&s).unwrap(), e);
    }

    #[test]
    fn all_kinds_round_trip() {
        for kind in [
            EntityKind::Person,
            EntityKind::LinearIssue,
            EntityKind::MergeRequest,
            EntityKind::Commit,
            EntityKind::Document,
            EntityKind::Meeting,
            EntityKind::SlackThread,
            EntityKind::Project,
        ] {
            let e = EntityRef::new(kind, "x1");
            assert_eq!(EntityRef::from_str(&e.canonical()).unwrap(), e);
        }
    }

    #[test]
    fn invalid_parse() {
        assert!(EntityRef::from_str("nope").is_err()); // no ':'
        assert!(EntityRef::from_str("bogus:x").is_err()); // unknown kind
        assert!(EntityRef::from_str("linear:").is_err()); // empty id
    }
}
