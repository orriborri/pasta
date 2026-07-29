use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Slack,
    Gmail,
    Linear,
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

    /// Content hash for change detection (skip re-embed if unchanged).
    #[must_use]
    pub fn content_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.id.as_bytes());
        h.update(b"|");
        h.update(self.title.as_bytes());
        h.update(b"|");
        h.update(self.content.as_bytes());
        format!("{:x}", h.finalize())
    }
}
