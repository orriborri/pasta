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
    /// Explicit permission for Gmail `JDex` label additions/removals.
    #[serde(default)]
    pub gmail_label: bool,
    #[serde(default)]
    pub state: Option<String>,
}
impl Entry {
    #[must_use]
    pub fn gmail_enabled(&self) -> bool {
        self.gmail_label && matches!(self.state.as_deref(), Some("approved" | "management"))
    }
}
impl Registry {
    /// Load the existing exported `JDex`. Presence does not grant routing approval.
    /// # Errors
    /// Returns an error for unreadable JSON or invalid/duplicate IDs.
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&std::fs::read(path)?)
    }
    /// Validate registry bytes without a second file read.
    /// # Errors
    /// Rejects malformed JSON or invalid/duplicate IDs.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let registry: Self = serde_json::from_slice(bytes)?;
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
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub deferred_until: Option<String>,
    #[serde(default)]
    pub waiting_reason: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<i64>,
}

/// Full desired state, with optimistic concurrency and a replay key.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskEdit {
    pub request_key: String,
    pub task_id: i64,
    pub expected_revision: u64,
    pub title: String,
    pub due_at: Option<String>,
    pub priority: u8,
    pub status: String,
    pub deferred_until: Option<String>,
    pub waiting_reason: Option<String>,
    pub blocked_by: Option<i64>,
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
            event TEXT NOT NULL, occurred_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS task_revisions (
            task_id INTEGER NOT NULL REFERENCES tasks(id), revision INTEGER NOT NULL,
            request_key TEXT NOT NULL UNIQUE, request TEXT NOT NULL, payload TEXT NOT NULL,
            occurred_at TEXT NOT NULL, PRIMARY KEY(task_id,revision));",
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
        self.task(task.id)
    }
    /// Deterministic ranking: overdue, then due within a day, then priority/due/id.
    /// # Errors
    /// Returns an error if the attention register cannot be read.
    pub fn next(&self, now: DateTime<Utc>, limit: usize) -> Result<Vec<Task>> {
        let mut q = self.0.prepare("SELECT id FROM tasks")?;
        let ids = q
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut tasks = Vec::new();
        for id in ids {
            let task = self.task(id)?;
            let eligible = task.status == "open"
                || (task.status == "deferred"
                    && task
                        .deferred_until
                        .as_deref()
                        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                        .is_some_and(|d| d <= now));
            let blocked = task
                .blocked_by
                .map(|id| self.task(id).map(|t| t.status != "done"))
                .transpose()?
                .unwrap_or(false);
            if eligible && !blocked {
                tasks.push(task);
            }
        }
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
        let task = self.task(id)?;
        if task.status == "done" {
            return Ok(());
        }
        self.edit_task(&TaskEdit {
            request_key: format!("complete:{}:{}", id, task.revision),
            task_id: id,
            expected_revision: task.revision,
            title: task.title,
            due_at: task.due_at,
            priority: task.priority,
            status: "done".into(),
            deferred_until: None,
            waiting_reason: None,
            blocked_by: None,
        })?;
        Ok(())
    }

    /// Read current attention state, including revisions retained across restarts.
    /// # Errors
    /// Returns an error for unknown IDs or corrupt storage.
    pub fn task(&self, id: i64) -> Result<Task> {
        use rusqlite::OptionalExtension;
        let original = self
            .0
            .query_row(
                "SELECT id,title,evidence_id,jd_id,due_at,priority,status FROM tasks WHERE id=?1",
                [id],
                task_row,
            )
            .context("unknown task ID")?;
        let payload: Option<String> = self.0.query_row("SELECT payload FROM task_revisions WHERE task_id=?1 ORDER BY revision DESC LIMIT 1", [id], |r| r.get(0)).optional()?;
        payload.map_or(Ok(original), |p| Ok(serde_json::from_str(&p)?))
    }

    /// Apply an owner-requested edit atomically. Retry keys bind to exact inputs.
    /// # Errors
    /// Rejects stale edits, conflicting retries, invalid states, or dependency cycles.
    pub fn edit_task(&mut self, edit: &TaskEdit) -> Result<Task> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.0,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let result = self.edit_in_transaction(edit)?;
        tx.commit()?;
        Ok(result)
    }

    fn edit_in_transaction(&self, edit: &TaskEdit) -> Result<Task> {
        use rusqlite::OptionalExtension;
        let request = serde_json::to_string(edit)?;
        let replay: Option<(String, String)> = self
            .0
            .query_row(
                "SELECT request,payload FROM task_revisions WHERE request_key=?1",
                [&edit.request_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((saved, payload)) = replay {
            if saved != request {
                bail!("request key reused with different edit");
            }
            return Ok(serde_json::from_str(&payload)?);
        }
        let mut task = self.task(edit.task_id)?;
        if edit.expected_revision != task.revision {
            bail!("stale task revision");
        }
        if edit.request_key.trim().is_empty() || edit.title.trim().is_empty() || edit.priority > 3 {
            bail!("invalid request key, title, or priority");
        }
        if !["open", "deferred", "waiting", "done", "canceled"].contains(&edit.status.as_str()) {
            bail!("invalid task status");
        }
        let due_at = normalize_date(edit.due_at.as_deref())?;
        let deferred_until = normalize_date(edit.deferred_until.as_deref())?;
        if (edit.status == "deferred") != deferred_until.is_some() {
            bail!("only deferred tasks require deferred_until");
        }
        if edit.status == "waiting"
            && edit
                .waiting_reason
                .as_deref()
                .is_none_or(|r| r.trim().is_empty())
            && edit.blocked_by.is_none()
        {
            bail!("waiting requires a reason or dependency");
        }
        let mut seen = HashSet::from([task.id]);
        let mut dependency = edit.blocked_by;
        while let Some(id) = dependency {
            if !seen.insert(id) {
                bail!("dependency cycle");
            }
            dependency = self.task(id)?.blocked_by;
        }
        task.title.clone_from(&edit.title);
        task.due_at = due_at;
        task.priority = edit.priority;
        task.status.clone_from(&edit.status);
        task.deferred_until = deferred_until;
        task.waiting_reason.clone_from(&edit.waiting_reason);
        task.blocked_by = edit.blocked_by;
        task.revision += 1;
        self.0.execute(
            "INSERT INTO task_revisions VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                task.id,
                task.revision,
                edit.request_key,
                request,
                serde_json::to_string(&task)?,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(task)
    }

    /// Produce a consistent SQLite backup. Destination must not already exist.
    /// # Errors
    /// Returns a database or destination error.
    pub fn backup(&self, destination: &Path) -> Result<()> {
        if destination.exists() {
            bail!("backup destination already exists");
        }
        self.0.execute(
            "VACUUM INTO ?1",
            [destination.to_str().context("invalid backup path")?],
        )?;
        Ok(())
    }

    /// Next actions with traceable evidence and a deterministic selection reason.
    /// # Errors
    /// Returns an error if attention state cannot be read.
    pub fn recommendations(&self, now: DateTime<Utc>, limit: usize) -> Result<serde_json::Value> {
        let items: Vec<_> = self.next(now, limit)?.into_iter().map(|task| {
            let reason = task.due_at.as_deref().and_then(|d| DateTime::parse_from_rfc3339(d).ok()).map_or("priority", |d| if d <= now { "overdue" } else if d <= now + chrono::Duration::days(1) { "due_within_24h" } else { "priority" });
            serde_json::json!({"reason":reason,"evidence_ref":format!("assistant:evidence:{}",task.evidence_id),"pasta_record_id":format!("vault-assistant-{}",task.evidence_id),"task":task})
        }).collect();
        Ok(serde_json::json!({"as_of":now.to_rfc3339(),"items":items}))
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
        revision: 0,
        deferred_until: None,
        waiting_reason: None,
        blocked_by: None,
    })
}

fn normalize_date(value: Option<&str>) -> Result<Option<String>> {
    value
        .map(|s| DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc).to_rfc3339()))
        .transpose()
        .context("date must be RFC3339 with an explicit timezone")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_commit_rolls_back_before_store_reuse() {
        let path =
            std::env::temp_dir().join(format!("assistant-busy-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut store = Store::open(&path).unwrap();
        store.0.busy_timeout(Duration::from_millis(20)).unwrap();
        let evidence = store.capture("memo", "1", "Call").unwrap();
        let task = store
            .add_task("create", "Call", &evidence, None, None, 1)
            .unwrap();
        let reader = Connection::open(&path).unwrap();
        reader.execute_batch("BEGIN; SELECT * FROM tasks;").unwrap();
        let edit = TaskEdit {
            request_key: "rename".into(),
            task_id: task.id,
            expected_revision: 0,
            title: "Call back".into(),
            due_at: None,
            priority: 1,
            status: "open".into(),
            deferred_until: None,
            waiting_reason: None,
            blocked_by: None,
        };
        assert!(store.edit_task(&edit).is_err());
        assert_eq!(store.task(task.id).unwrap().revision, 0);
        reader.execute_batch("ROLLBACK").unwrap();
        assert_eq!(store.edit_task(&edit).unwrap().revision, 1);
        drop(reader);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}

pub mod gmail_labels;
