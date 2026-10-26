//! Import immutable assistant capture versions without modifying the source store.
use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

/// Read every retained version. Missing stores fail without creating an empty database.
/// # Errors
/// Returns errors for unreadable stores, incompatible schema, or malformed timestamps.
pub fn fetch(path: &Path) -> Result<Vec<Record>> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(Duration::from_secs(10))?;
    let mut query = db.prepare(
        "SELECT id,source,source_id,content,captured_at FROM evidence ORDER BY captured_at,id",
    )?;
    let rows = query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, origin, native, content, captured) = row?;
        let at = DateTime::parse_from_rfc3339(&captured)?.with_timezone(&Utc);
        let thread = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&origin, &native))?)
        );
        Ok(Record {
            id: Record::make_id(Source::Vault, &format!("assistant-{id}")),
            source: Source::Vault,
            kind: Kind::Message,
            title: content
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(120)
                .collect(),
            content,
            author: String::new(),
            participants: vec![],
            created_at: at,
            updated_at: at,
            url: format!("assistant:evidence:{id}"),
            thread_id: thread,
            entities: vec![],
            tags: vec!["original-capture".into(), format!("origin:{origin}")],
        })
    })
    .collect()
}
