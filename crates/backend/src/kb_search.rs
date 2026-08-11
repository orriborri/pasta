//! Backend search delegated to kb-engine hybrid search.
//!
//! Replaces the old pasta-internal `history::indexer::search`. Source, date, and
//! participant filters are applied pre-limit (over-fetch then filter) since
//! `hybrid_search` itself takes no filter arguments.

use kb_storage::{embedder, hybrid_search};
use pasta_common::ipc::SearchResultItem;

/// Search kb-engine, mapping hits to the TUI's `SearchResultItem`.
///
/// # Errors
/// Returns an error if the embedder cannot initialize or the hybrid search fails.
pub async fn search(
    query: &str,
    source: Option<&str>,
    participant: Option<&str>,
    after: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<SearchResultItem>> {
    embedder::init().await?;
    let cfg = pasta_common::config::kb_config();

    // Over-fetch so post-filtering still yields up to `limit` results.
    let overfetch = limit.saturating_mul(3).max(limit);
    let hits = hybrid_search(&cfg, query, overfetch).await?;

    let participant_lc = participant.map(str::to_lowercase);
    let items = hits
        .into_iter()
        .filter(|r| source.is_none_or(|s| r.source == s))
        .filter(|r| after.is_none_or(|a| r.created_at.as_str() >= a))
        .filter(|r| {
            participant_lc.as_ref().is_none_or(|p| {
                r.content.to_lowercase().contains(p) || r.title.to_lowercase().contains(p)
            })
        })
        .take(limit)
        .map(|r| SearchResultItem {
            path: if r.url.is_empty() { r.id } else { r.url },
            source: r.source,
            participants: String::new(),
            date: r.created_at,
            content: if r.title.is_empty() {
                r.content
            } else {
                format!("{}\n{}", r.title, r.content)
            },
        })
        .collect();

    Ok(items)
}
