use anyhow::Result;
use chrono::Local;
use kb_core::{Record, Source, SyncState};
use kb_storage::{embedder, ParquetStore, TextIndex, VectorStore};
use pasta_common::vault::VaultLayout;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Sources that can be synced.
pub const ALL_SOURCES: &[&str] = &["slack", "gmail", "linear", "git", "vault", "calendar", "gdocs"];

/// Returns the feeds directory path using `VaultLayout`.
/// This ensures the feeds directory is always relative to the configured vault path.
#[must_use]
pub fn feeds_dir() -> PathBuf {
    let vault_path = Path::new(&pasta_common::config::get().general.vault_path);
    VaultLayout::new(vault_path).feeds()
}

/// Fetch records from specified sources. Does NOT index them.
/// `lookback_days` controls how far back to go for channels without cursors (default: 1 for scheduled, 30 for backfill).
///
/// # Errors
/// Returns error if any fetcher fails critically.
pub async fn fetch(sources: &[&str]) -> Result<Vec<Record>> {
    fetch_with_lookback(sources, 1).await
}

/// Fetch with explicit lookback for first-time channels (used by backfill).
///
/// # Errors
/// Returns error if any fetcher fails critically.
pub async fn fetch_with_lookback(sources: &[&str], lookback_days: i64) -> Result<Vec<Record>> {
    let config = pasta_common::config::kb_config();
    let state = SyncState::open(&config)?;

    let mut all_records: Vec<Record> = Vec::new();

    for src in sources {
        // Fetch each source independently. A single source failing (e.g. an
        // expired credential) must NOT abort the whole cycle — log it and
        // continue so the remaining sources still produce feeds/records.
        let result: Result<Vec<Record>> = match *src {
            "slack" => kb_fetchers::slack::SlackFetcher::new().with_lookback_days(lookback_days).fetch(&state).await,
            "gmail" => kb_fetchers::gmail::GmailFetcher::new().fetch(&state).await,
            "linear" => kb_fetchers::linear::LinearFetcher::new().fetch(&state).await,
            "git" => {
                let repos = pasta_common::config::get().repos.iter().map(|r| r.path.clone()).collect();
                kb_fetchers::git::GitFetcher::new(repos).fetch(&state)
            }
            "vault" => {
                let vault_path = &pasta_common::config::get().general.vault_path;
                kb_fetchers::vault::VaultFetcher::new(vault_path).fetch()
            }
            "calendar" => {
                let cal_ids = pasta_common::config::get().gog.calendar_ids.clone();
                kb_fetchers::calendar::CalendarFetcher::new()
                    .with_calendar_ids(cal_ids)
                    .fetch(&state).await
            }
            "gdocs" => kb_fetchers::gdocs::GdocsFetcher::new().fetch(&state).await,
            _ => continue,
        };
        match result {
            Ok(records) => {
                if !records.is_empty() {
                    info!(source = src, count = records.len(), "fetched");
                }
                all_records.extend(records);
            }
            Err(e) => {
                tracing::warn!(source = src, error = %e, "fetcher failed, skipping");
            }
        }
    }

    Ok(all_records)
}

/// Index records through the pipeline and store them.
///
/// # Errors
/// Returns error if embedding, pipeline, or storage fails.
pub async fn index(records: Vec<Record>) -> Result<usize> {
    if records.is_empty() {
        return Ok(0);
    }

    let config = pasta_common::config::kb_config();
    embedder::init().await?;

    let state = SyncState::open(&config)?;
    state.lock_model(embedder::model_name())?;

    // Run pipeline
    let vault_path = &pasta_common::config::get().general.vault_path;
    let registry = std::sync::Arc::new(kb_pipeline::registry::EntityRegistry::load(vault_path));
    let pipeline = kb_pipeline::Pipeline::new(vec![
        Box::new(kb_pipeline::normalize::NormalizeStage::new(registry.clone())),
        Box::new(kb_pipeline::filter::FilterStage::new()),
        Box::new(kb_pipeline::dedupe::DedupeStage),
        Box::new(kb_pipeline::extract::ExtractStage::new()),
        Box::new(kb_pipeline::cross_dedupe::CrossSourceDedupeStage),
        Box::new(kb_pipeline::chunk::ChunkStage),
        Box::new(kb_pipeline::summarize::SummarizeStage::new()),
        Box::new(kb_pipeline::enrich::EnrichStage::new(registry)),
    ]);
    let all_records = pipeline.run(records);
    info!(after_pipeline = all_records.len(), "pipeline complete");

    // Entity management
    let kb_cfg = &pasta_common::config::get().kb;
    let allowlist = kb_pipeline::entity_manager::InternalAllowlist {
        domains: kb_cfg.internal_domains.clone(),
        slack_is_internal: kb_cfg.internal_slack,
    };
    let entity_mgr = kb_pipeline::entity_manager::EntityManager::new(vault_path, allowlist);
    let discoveries = entity_mgr.update_from_records(&all_records);
    kb_pipeline::discoveries::surface_discoveries(vault_path, &discoveries);

    // Hash filter unchanged records
    let records: Vec<_> = all_records.into_iter().filter(|r| {
        let hash = r.content_hash();
        if state.get_hash(&r.id).as_deref() == Some(hash.as_str()) {
            return false;
        }
        state.set_hash(&r.id, &hash).ok();
        true
    }).collect();

    if records.is_empty() {
        return Ok(0);
    }
    info!(changed = records.len(), "records after hash filter");

    // Store
    let parquet = ParquetStore::new(&config);
    parquet.write(&records)?;

    let text_index = TextIndex::open(&config)?;
    text_index.upsert(&records)?;

    let vector = VectorStore::new(&config);
    embed_and_upsert(&vector, &records).await?;

    Ok(records.len())
}

/// Convenience: fetch + index in one call (full lookback for CLI use).
///
/// # Errors
/// Returns error if fetching or indexing fails.
pub async fn run(sources: &[&str]) -> Result<usize> {
    let records = fetch_with_lookback(sources, 30).await?;
    index(records).await
}

/// Write `.feeds/` markdown files from fetched records, grouped by source.
pub fn write_feeds(records: &[Record]) {
    let feeds_path = feeds_dir();
    fs::create_dir_all(&feeds_path).ok();
    let now = Local::now().format("%Y-%m-%dT%H:%M");

    for source in &[Source::Slack, Source::Gmail, Source::Linear, Source::Calendar] {
        let source_records: Vec<&Record> = records.iter().filter(|r| r.source == *source).collect();
        let name = source.to_string();
        let body = format_feed(&source_records, *source);
        let content = format!("---\nsource: {name}\nfetched: {now}\n---\n\n{body}");
        let path = feeds_path.join(format!("{name}.md"));
        fs::write(path, content).ok();
    }
}

fn format_feed(records: &[&Record], source: Source) -> String {
    match source {
        Source::Slack => format_slack_feed(records),
        Source::Gmail => format_gmail_feed(records),
        Source::Linear => format_linear_feed(records),
        Source::Calendar => format_calendar_feed(records),
        _ => String::new(),
    }
}

fn format_slack_feed(records: &[&Record]) -> String {
    let mut body = String::new();

    let dms: Vec<&&Record> = records.iter().filter(|r| r.tags.contains(&"dm".to_string())).collect();
    let channels: Vec<&&Record> = records.iter().filter(|r| r.tags.contains(&"channel".to_string())).collect();

    body.push_str("## Unread DMs\n\n");
    if dms.is_empty() {
        body.push_str("No open DMs.\n");
    } else {
        // Group by thread_id (conversation)
        let mut seen_threads = std::collections::HashSet::new();
        for r in &dms {
            if !seen_threads.insert(&r.thread_id) { continue; }
            let thread_msgs: Vec<&&&Record> = dms.iter().filter(|m| m.thread_id == r.thread_id).collect();
            let person = r.title.split(" - ").nth(1).unwrap_or(&r.author);
            let _ = writeln!(body, "**{person}**");
            for m in thread_msgs.iter().rev().take(3) {
                let text: String = m.content.chars().take(100).collect();
                let _ = writeln!(body, "  - {}: {}", m.author, text);
            }
            body.push('\n');
        }
    }

    body.push_str("## Unread Channels\n\n");
    if channels.is_empty() {
        body.push_str("No unread channels.\n");
    } else {
        let mut seen_threads = std::collections::HashSet::new();
        for r in &channels {
            if !seen_threads.insert(&r.thread_id) { continue; }
            let thread_msgs: Vec<&&&Record> = channels.iter().filter(|m| m.thread_id == r.thread_id).collect();
            let ch_name = r.title.split(" - ").next().unwrap_or("?").trim_start_matches('#');
            let _ = writeln!(body, "**#{}** ({} messages)", ch_name, thread_msgs.len());
            for m in thread_msgs.iter().rev().take(5) {
                let text: String = m.content.chars().take(100).collect();
                let _ = writeln!(body, "  - {}: {}", m.author, text);
            }
            body.push('\n');
        }
    }
    body
}

fn format_gmail_feed(records: &[&Record]) -> String {
    let mut body = String::new();

    let actionable: Vec<&&Record> = records.iter()
        .filter(|r| !r.tags.contains(&"Gitlab".to_string()))
        .take(50)
        .collect();

    body.push_str("## Actionable Unread\n\n");
    if actionable.is_empty() {
        body.push_str("No actionable unread emails.\n");
    }
    for r in &actionable {
        let _ = writeln!(body, "- **{}** — {} ({})", r.author, r.title, r.created_at.format("%b %d"));
    }

    let gitlab: Vec<&&Record> = records.iter()
        .filter(|r| r.tags.contains(&"Gitlab".to_string()))
        .collect();
    if !gitlab.is_empty() {
        let _ = write!(body, "\n## GitLab Notifications ({})\n\n", gitlab.len());
        for r in gitlab.iter().take(5) {
            let _ = writeln!(body, "- {} ({})", r.title, r.created_at.format("%b %d"));
        }
        if gitlab.len() > 5 {
            let _ = writeln!(body, "- ...and {} more", gitlab.len() - 5);
        }
    }
    body
}

fn format_linear_feed(records: &[&Record]) -> String {
    let mut body = String::new();

    body.push_str("## Open Issues\n\n");
    if records.is_empty() {
        body.push_str("No assigned issues.\n");
    } else {
        body.push_str("| Issue | Status | Project |\n|-------|--------|--------|\n");
        for r in records {
            let status = r.tags.first().map_or("-", String::as_str);
            let project = r.tags.get(1).map_or("-", String::as_str);
            let link = if r.url.is_empty() { r.title.clone() } else { format!("[{}]({})", r.title, r.url) };
            let _ = writeln!(body, "| {link} | {status} | {project} |");
        }
    }
    body
}

fn format_calendar_feed(records: &[&Record]) -> String {
    use chrono::{Local, TimeZone};

    let mut body = String::new();
    let today = Local::now().date_naive();

    // Split into today vs upcoming
    let mut today_events: Vec<&Record> = Vec::new();
    let mut upcoming_events: Vec<&Record> = Vec::new();

    for r in records {
        let local_date = Local.from_utc_datetime(&r.created_at.naive_utc()).date_naive();
        if local_date == today {
            today_events.push(r);
        } else if local_date > today {
            upcoming_events.push(r);
        }
    }

    // Sort by time
    today_events.sort_by_key(|r| r.created_at);
    upcoming_events.sort_by_key(|r| r.created_at);

    body.push_str("## Today's Meetings\n\n");
    if today_events.is_empty() {
        body.push_str("No meetings today.\n");
    } else {
        for r in &today_events {
            let local_time = Local.from_utc_datetime(&r.created_at.naive_utc()).format("%H:%M");
            let _ = writeln!(body, "- **{}** — {}", r.title, local_time);
            if !r.content.is_empty() && r.content != r.title {
                let preview: String = r.content.chars().take(80).collect();
                let _ = writeln!(body, "  {preview}");
            }
        }
    }

    body.push_str("\n## Upcoming Events\n\n");
    if upcoming_events.is_empty() {
        body.push_str("No upcoming events.\n");
    } else {
        for r in upcoming_events.iter().take(15) {
            let local_dt = Local.from_utc_datetime(&r.created_at.naive_utc());
            let _ = writeln!(body, "- **{}** — {}", r.title, local_dt.format("%a %b %d %H:%M"));
        }
    }
    body
}

async fn embed_and_upsert(vector: &VectorStore, records: &[Record]) -> Result<()> {
    for (i, chunk) in records.chunks(50).enumerate() {
        let texts: Vec<String> = chunk.iter().map(|r| {
            if r.title.is_empty() { r.content.clone() }
            else { format!("{}\n{}", r.title, r.content) }
        }).collect();
        let ids: Vec<String> = chunk.iter().map(|r| r.id.clone()).collect();
        let contents: Vec<String> = chunk.iter().map(|r| r.content.clone()).collect();
        let sources: Vec<String> = chunk.iter().map(|r| r.source.to_string()).collect();
        let titles: Vec<String> = chunk.iter().map(|r| r.title.clone()).collect();
        let created_ats: Vec<String> = chunk.iter().map(|r| r.created_at.format("%Y-%m-%d %H:%M").to_string()).collect();
        let urls: Vec<String> = chunk.iter().map(|r| r.url.clone()).collect();

        let embeddings = embedder::embed_batch(&texts).await?;
        vector.upsert(&ids, &contents, &sources, &titles, &created_ats, &urls, embeddings).await?;
        info!(batch = i + 1, total_batches = records.len().div_ceil(50), "embedded");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that kb-sync's `write_feeds` uses `VaultLayout` instead of hardcoded path
    #[test]
    fn write_feeds_uses_vault_layout_for_feeds_dir() {
        // This test verifies that write_feeds uses VaultLayout::feeds() instead
        // of a hardcoded absolute path like "/home/orre/Obsidian/Readpeak/.feeds"
        
        let feeds_path = feeds_dir();
        
        // The path should end with .feeds
        assert!(feeds_path.to_string_lossy().ends_with("/.feeds"));
        
        // The path should be constructed from the vault path in config
        let config = pasta_common::config::get();
        let vault_path = Path::new(&config.general.vault_path);
        assert_eq!(feeds_path, vault_path.join(".feeds"));
    }

    /// Test that kb-sync uses configured vault path from config
    #[test]
    fn kb_sync_uses_configured_vault_path() {
        // This test verifies that kb-sync uses the vault path from config
        // rather than a hardcoded absolute path
        
        let config = pasta_common::config::get();
        let vault_path = Path::new(&config.general.vault_path);
        let feeds_path = feeds_dir();
        
        // The feeds directory should be a subdirectory of the vault path
        assert!(feeds_path.starts_with(vault_path));
        
        // The path should be exactly vault_path/.feeds
        assert_eq!(feeds_path, vault_path.join(".feeds"));
    }

    /// Test that `feeds_dir` returns a vault-relative path
    #[test]
    fn feeds_dir_is_vault_relative() {
        // This test verifies that feeds_dir() returns a path relative to
        // the configured vault path, not an absolute hardcoded path
        
        let config = pasta_common::config::get();
        let vault_path = Path::new(&config.general.vault_path);
        let feeds_path = feeds_dir();
        
        // The path should start with the vault path
        assert!(feeds_path.starts_with(vault_path));
        
        // The path should end with .feeds
        assert!(feeds_path.to_string_lossy().ends_with("/.feeds"));
    }
}
