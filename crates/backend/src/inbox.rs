//! Inbox processing: query kb-engine for recent records and apply the
//! configured `task_rules` to propose tasks or skip noise.
//!
//! Tasks derived from kb records are written as `status: pending` and must be
//! approved by the user (TUI Tasks tab) before they become active work. Setting
//! `kb.require_task_approval = false` lets `auto_create` matches go straight to
//! `status: open`; `ask` matches always land in the pending queue.
//!
//! Records are read directly from the kb-engine Parquet store, with date filtering
//! applied in-process.

use std::path::Path;

use chrono::Duration;
use kb_core::Record;
use kb_storage::ParquetStore;
use pasta_common::config;
use pasta_common::task_match::{classify, Action, MatchAttrs};
use pasta_common::vault::{STATUS_OPEN, STATUS_PENDING, VaultLayout};

use crate::util::log;

/// Query kb-engine for records from the last `days` and apply `task_rules`.
///
/// # Errors
/// Returns an error if reading from Parquet store fails.
pub async fn process(days: i64) -> anyhow::Result<()> {
    let config = pasta_common::config::kb_config();
    let store = ParquetStore::new(&config);
    let all_records = store.read_all()?;

    // Filter to records within the specified days
    let cutoff = chrono::Utc::now() - Duration::days(days);
    let records: Vec<Record> = all_records
        .into_iter()
        .filter(|r| r.created_at > cutoff)
        .collect();

    let kb = &config::get().kb;
    let rules = &kb.task_rules;

    let (mut created, mut proposed, mut skipped) = (0u32, 0u32, 0u32);
    for r in &records {
        let attrs = build_attrs(r);
        let (action, signal) = classify(rules, &r.source.to_string(), &attrs);
        // Approval mode downgrades auto-creation to a pending proposal; `ask`
        // matches are always proposals.
        let status = match action {
            Action::AutoCreate if !kb.require_task_approval => STATUS_OPEN,
            Action::AutoCreate | Action::Ask => STATUS_PENDING,
            Action::Skip | Action::None => {
                skipped += 1;
                continue;
            }
        };
        if create_task(r, signal.as_deref(), status) {
            if status == STATUS_PENDING { proposed += 1 } else { created += 1 }
        }
    }

    log(
        "inbox-processor",
        &format!(
            "kb records: {} | created {created} | pending approval {proposed} | skip/none {skipped}",
            records.len()
        ),
    );
    Ok(())
}

/// Build matchable attributes from a record. Field names mirror the `field:value`
/// tokens used in `task_rules` match expressions.
fn build_attrs(r: &Record) -> MatchAttrs {
    let source_str = r.source.to_string();
    let kind_str = kind_to_string(&r.kind);
    MatchAttrs::new()
        .with("source", &source_str)
        .with("kind", &kind_str)
        .with("is", &kind_str)
        .with("author", &r.author)
        .with_many("label", &r.tags)
        .with_many("labels", &r.tags)
        .with_many("participants", &r.participants)
}

/// Convert Kind enum to string for matching
fn kind_to_string(kind: &kb_core::Kind) -> String {
    match kind {
        kb_core::Kind::Message => "message",
        kb_core::Kind::Thread => "thread",
        kb_core::Kind::Issue => "issue",
        kb_core::Kind::Commit => "commit",
        kb_core::Kind::Doc => "doc",
        kb_core::Kind::Event => "event",
    }
    .to_string()
}

/// Write a task file with the given status. Deterministic filename derived from
/// the kb id makes this idempotent across re-runs — including for tasks the user
/// rejected, which keep their file so they are not proposed again.
fn create_task(r: &Record, signal: Option<&str>, status: &str) -> bool {
    let vault = pasta_common::vault::vault_path();
    let layout = VaultLayout::new(Path::new(vault));
    let path = layout.tasks().join(format!("kb-{}.md", slugify(&r.id)));
    if path.exists() {
        return false;
    }

    let url_line = if r.url.is_empty() {
        String::new()
    } else {
        format!("source_url: \"{}\"\n", r.url)
    };
    let body = format!(
        "---\ntitle: \"{title}\"\nstatus: {status}\nsource: {source}\nkb_id: \"{id}\"\n{url_line}signal: {signal}\n---\n\n{content}\n",
        title = r.title.replace('"', "'"),
        source = r.source,
        id = r.id,
        signal = signal.unwrap_or("auto"),
        content = r.content,
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, body).is_ok()
}

/// Lowercase, dash-separated slug safe for filenames.
fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
