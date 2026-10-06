use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Slack,
    Gmail,
    Linear,
    GitLab,
    Git,
    Gdocs,
    Calendar,
    Vault,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Slack => "slack",
            Self::Gmail => "gmail",
            Self::Linear => "linear",
            Self::GitLab => "gitlab",
            Self::Git => "git",
            Self::Gdocs => "gdocs",
            Self::Calendar => "calendar",
            Self::Vault => "vault",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Message,
    Thread,
    Issue,
    Commit,
    Doc,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub source: Source,
    pub kind: Kind,
    pub title: String,
    pub content: String,
    pub author: String,
    pub participants: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub url: String,
    pub thread_id: String,
    pub entities: Vec<String>,
    pub tags: Vec<String>,
}

impl Record {
    /// Deterministic ID: `<source>-<native_id>`.
    #[must_use]
    pub fn make_id(source: Source, native_id: &str) -> String {
        format!("{source}-{native_id}")
    }

    /// Hash the complete record for change detection (skip re-embed if unchanged).
    ///
    /// # Panics
    /// Panics if serializing the plain record fields unexpectedly fails.
    #[must_use]
    pub fn content_hash(&self) -> String {
        // Include metadata used by retrieval and the graph. JSON encoding also
        // avoids ambiguous delimiter boundaries in titles/content.
        let mut h = Sha256::new();
        h.update(serde_json::to_vec(self).expect("Record serialization cannot fail"));
        format!("{:x}", h.finalize())
    }
}
