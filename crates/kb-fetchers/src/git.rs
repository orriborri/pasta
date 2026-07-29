use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use std::path::Path;
use tracing::info;

pub struct GitFetcher {
    repos: Vec<String>,
}

impl GitFetcher {
    #[must_use]
    pub const fn new(repos: Vec<String>) -> Self {
        Self { repos }
    }

    /// Fetch git commits from configured repos since last cursor.
    ///
    /// # Errors
    /// Returns error if git log parsing fails.
    pub fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let since = state.cursor("git_forward")
            .unwrap_or_else(|| "1 week ago".to_string());

        let mut records = Vec::new();

        for repo_path in &self.repos {
            let path = Path::new(repo_path);
            if !path.exists() { continue; }
            let repo_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            let entries = git_log(path, &since);
            info!(repo = %repo_name, commits = entries.len(), "git log fetched");

            for entry in entries {
                let id = Record::make_id(Source::Git, &format!("{}-{}", repo_name, &entry.hash[..8]));
                let content = if entry.body.is_empty() {
                    entry.subject.clone()
                } else {
                    format!("{}\n\n{}", entry.subject, entry.body)
                };
                let created = parse_git_date(&entry.date);

                records.push(Record {
                    id,
                    source: Source::Git,
                    kind: Kind::Commit,
                    title: format!("[{}] {}", repo_name, entry.subject),
                    content,
                    author: entry.author,
                    participants: vec![],
                    created_at: created,
                    updated_at: created,
                    url: String::new(),
                    thread_id: repo_name.clone(),
                    entities: vec![],
                    tags: vec![repo_name.clone()],
                });
            }
        }

        let now = Utc::now().format("%Y-%m-%d").to_string();
        state.update_window("git_forward", &since, &now).ok();

        info!(total = records.len(), "git fetch complete");
        Ok(records)
    }
}

struct LogEntry {
    hash: String,
    author: String,
    date: String,
    subject: String,
    body: String,
}

fn git_log(repo_path: &Path, since: &str) -> Vec<LogEntry> {
    let output = std::process::Command::new("git")
        .args(["log", &format!("--since={since}"), "--no-merges", "--format=%H%x00%an%x00%ai%x00%s%x00%b%x01"])
        .current_dir(repo_path)
        .output()
        .ok();
    let Some(output) = output else { return vec![] };
    if !output.status.success() { return vec![]; }

    let raw = String::from_utf8_lossy(&output.stdout);
    raw.split('\x01')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|record| {
            let parts: Vec<&str> = record.trim().splitn(5, '\x00').collect();
            if parts.len() < 4 { return None; }
            let hash = parts[0].trim().to_string();
            if hash.len() < 8 { return None; }
            Some(LogEntry {
                hash,
                author: parts[1].to_string(),
                date: parts[2].to_string(),
                subject: parts[3].to_string(),
                body: parts.get(4).unwrap_or(&"").trim().to_string(),
            })
        })
        .collect()
}

fn parse_git_date(s: &str) -> DateTime<Utc> {
    // Git date format: "2026-06-25 14:30:00 +0300"
    DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z").map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc))
}
