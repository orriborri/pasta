use anyhow::Result;
use kb_core::KbConfig;
use std::collections::HashMap;

use crate::text_index::TextIndex;
use crate::vector_store::VectorStore;

/// A unified search result from hybrid (vector + full-text) search.
pub struct SearchResult {
    pub id: String,
    pub source: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub url: String,
    pub score: f32,
}

/// RRF constant: controls how steeply rank contributes to the fused score.
const K: f32 = 60.0;

/// Run vector + full-text in parallel, merge with Reciprocal Rank Fusion.
///
/// # Errors
/// Returns error if the text index cannot be opened.
#[allow(clippy::cast_precision_loss)]
pub async fn hybrid_search(config: &KbConfig, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let vector_store = VectorStore::new(config);
    let text_index = TextIndex::open(config)?;

    // Run both searches
    let vector_results = vector_store.search(query, limit * 2).await.unwrap_or_default();
    let text_results = text_index.search(query, limit * 2).unwrap_or_default();

    // RRF merge: score = sum(1 / (k + rank)) across result lists
    let mut scores: HashMap<String, (f32, SearchResult)> = HashMap::new();

    for (rank, r) in vector_results.into_iter().enumerate() {
        let rrf = 1.0 / (K + rank as f32 + 1.0);
        scores.entry(r.id.clone()).or_insert_with(|| (0.0, SearchResult {
            id: r.id, source: r.source, title: r.title, content: r.content, created_at: r.created_at, url: r.url, score: 0.0,
        })).0 += rrf;
    }

    for (rank, r) in text_results.into_iter().enumerate() {
        let rrf = 1.0 / (K + rank as f32 + 1.0);
        let entry = scores.entry(r.id.clone()).or_insert_with(|| (0.0, SearchResult {
            id: r.id, source: r.source, title: r.title, content: r.content, created_at: r.created_at, url: r.url, score: 0.0,
        }));
        entry.0 += rrf;
    }

    // Sort by RRF score descending
    let mut merged: Vec<SearchResult> = scores.into_values()
        .map(|(score, mut r)| { r.score = score; r })
        .collect();
    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(limit);

    Ok(merged)
}
