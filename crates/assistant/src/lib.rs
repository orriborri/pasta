//! Durable NAS-backed tools for an external personal-assistant runtime.
//! Evidence history and attention state are durable; neither is a disposable index.
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path, time::Duration};

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub entries: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
pub struct Entry {
    pub jd_id: String,
    pub label: String,
}
impl Registry {
    /// Load the existing exported `JDex`. Presence does not grant routing approval.
    /// # Errors
    /// Returns an error for unreadable JSON or invalid/duplicate IDs.
    pub fn load(path: &Path) -> Result<Self> {
        let registry: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        let mut seen = HashSet::new();
        for entry in &registry.entries {
            let b = entry.jd_id.as_bytes();
            if b.len() != 5
                || b[2] != b'.'
                || !b
                    .iter()
                    .enumerate()
                    .all(|(i, c)| i == 2 || c.is_ascii_digit())
                || entry.label.trim().is_empty()
                || !seen.insert(&entry.jd_id)
            {
                bail!("invalid or duplicate JDex entry: {}", entry.jd_id);
            }
        }
        Ok(registry)
    }
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|entry| entry.jd_id == id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub evidence_id: String,
    pub jd_id: Option<String>,
    pub due_at: Option<String>,
    pub priority: u8,
    pub status: String,
}

pub struct Store(Connection);
fn digest(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
impl Store {
    /// Open a local NAS bind-mounted database, not a database on SMB/NFS.
    /// # Errors
    /// Returns an error if directory creation, database opening, or schema setup fails.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(10))?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS evidence (
            id TEXT PRIMARY KEY, source TEXT NOT NULL, source_id TEXT NOT NULL,
            content TEXT NOT NULL, captured_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY, request_key TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL, evidence_id TEXT NOT NULL REFERENCES evidence(id),
            jd_id TEXT, due_at TEXT, priority INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','done')));
          CREATE TABLE IF NOT EXISTS task_events (
            id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL REFERENCES tasks(id),
            event TEXT NOT NULL, occurred_at TEXT NOT NULL);",
        )?;
        Ok(Self(db))
    }
    /// Retain every distinct content version. Replaying a source version is idempotent.
    /// # Errors
    /// Returns an error for empty inputs or a database write failure.
    pub fn capture(&self, source: &str, source_id: &str, content: &str) -> Result<String> {
        if source.trim().is_empty() || source_id.trim().is_empty() || content.trim().is_empty() {
            bail!("source, source ID, and content must be nonempty");
        }
        let id = digest(&[source, source_id, content]);
        self.0.execute(
            "INSERT OR IGNORE INTO evidence VALUES (?1,?2,?3,?4,?5)",
            params![id, source, source_id, content, Utc::now().to_rfc3339()],
        )?;
        Ok(id)
    }
    /// # Errors
    /// Returns an error if the evidence ID is unknown or the database cannot be read.
    pub fn evidence(&self, id: &str) -> Result<serde_json::Value> {
        self.0
            .query_row(
                "SELECT source,source_id,content,captured_at FROM evidence WHERE id=?1",
                [id],
                |r| {
                    Ok(serde_json::json!({"id":id,"source":r.get::<_,String>(0)?,
                "source_id":r.get::<_,String>(1)?,"content":r.get::<_,String>(2)?,
                "captured_at":r.get::<_,String>(3)?}))
                },
            )
            .context("unknown evidence ID")
    }
    /// Explicit task creation only: capturing a note never creates a commitment.
    /// # Errors
    /// Returns an error for invalid inputs, missing evidence, conflicting retries, or database failures.
    pub fn add_task(
        &mut self,
        key: &str,
        title: &str,
        evidence: &str,
        jd: Option<&str>,
        due: Option<&str>,
        priority: u8,
    ) -> Result<Task> {
        if key.trim().is_empty() || title.trim().is_empty() || priority > 3 {
            bail!("request key/title must be nonempty; priority must be 0..3");
        }
        let due = due
            .map(|s| DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc).to_rfc3339()))
            .transpose()
            .context("due date must be RFC3339 with an explicit timezone")?;
        let tx = self.0.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO tasks(request_key,title,evidence_id,jd_id,due_at,priority)
            VALUES (?1,?2,?3,?4,?5,?6)",
            params![key, title, evidence, jd, due, priority],
        )?;
        let task = tx.query_row("SELECT id,title,evidence_id,jd_id,due_at,priority,status FROM tasks WHERE request_key=?1", [key], task_row)?;
        if task.title != title
            || task.evidence_id != evidence
            || task.jd_id.as_deref() != jd
            || task.due_at != due
            || task.priority != priority
        {
            bail!("request key reused with different task content");
        }
        tx.execute(
            "INSERT INTO task_events(task_id,event,occurred_at)
            SELECT ?1,'created',?2 WHERE NOT EXISTS (SELECT 1 FROM task_events WHERE task_id=?1)",
            params![task.id, Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(task)
    }
    /// Deterministic ranking: overdue, then due within a day, then priority/due/id.
    /// # Errors
    /// Returns an error if the attention register cannot be read.
    pub fn next(&self, now: DateTime<Utc>, limit: usize) -> Result<Vec<Task>> {
        let mut q = self.0.prepare("SELECT id,title,evidence_id,jd_id,due_at,priority,status FROM tasks WHERE status='open'")?;
        let mut tasks = q
            .query_map([], task_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tasks.sort_by_key(|t| {
            let due = t
                .due_at
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok());
            let bucket = due.as_ref().map_or(2, |d| {
                if *d <= now {
                    0
                } else if *d <= now + chrono::Duration::days(1) {
                    1
                } else {
                    2
                }
            });
            (
                bucket,
                std::cmp::Reverse(t.priority),
                due.is_none(),
                due,
                t.id,
            )
        });
        tasks.truncate(limit.min(100));
        Ok(tasks)
    }
    /// # Errors
    /// Returns an error for an unknown task or a database transaction failure.
    pub fn done(&mut self, id: i64) -> Result<()> {
        let tx = self.0.transaction()?;
        let status: String = tx
            .query_row("SELECT status FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .context("unknown task ID")?;
        if status != "done" {
            tx.execute("UPDATE tasks SET status='done' WHERE id=?1", [id])?;
            tx.execute(
                "INSERT INTO task_events(task_id,event,occurred_at) VALUES (?1,'done',?2)",
                params![id, Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
fn task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        evidence_id: row.get(2)?,
        jd_id: row.get(3)?,
        due_at: row.get(4)?,
        priority: row.get(5)?,
        status: row.get(6)?,
    })
}
