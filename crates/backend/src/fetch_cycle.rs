use pasta_common::ipc::Event;

use std::collections::HashSet;

use crate::state::EventTx;
use crate::util::log;

/// Run a complete fetch cycle: fetch via kb-fetchers, write TUI feeds, update the
/// completed set (for agentic schedules), index into search stores, and generate
/// the daily note. Called by both the scheduler and the ForceFetch command.
pub async fn run(mut completed: HashSet<String>, event_tx: Option<EventTx>) -> HashSet<String> {
    let sources = &["gmail", "linear", "calendar", "slack"];

    // Fetch records using kb-fetchers (single pass) with timeout
    let records = match tokio::time::timeout(
        tokio::time::Duration::from_secs(300),
        kb_sync::fetch(sources),
    ).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            log("native-fetchers", &format!("fetch failed: {e}"));
            return completed;
        }
        Err(_) => {
            log("native-fetchers", "fetch timed out (5min)");
            return completed;
        }
    };

    // Write .feeds/ markdown BEFORE indexing (feeds update even if indexing fails)
    kb_sync::write_feeds(&records);

    // Mark every native fetcher in the group complete so dependent agents fire.
    // The fetch cycle runs all native fetchers as one unit, so completion is keyed
    // on the cycle finishing — NOT on whether a given source returned new records.
    // Gating on records left agents waiting forever, and gitlab arrives via the
    // gmail feed (records tagged "Gitlab") so it never produced its own records.
    for fetcher in ["gitlab-fetcher", "linear-fetcher", "slack-fetcher", "gmail-fetcher", "calendar-fetcher"] {
        completed.insert(fetcher.to_string());
    }

    // Generate daily note
    if let Err(e) = crate::vault_organize::generate_daily().await {
        tracing::warn!("daily note generation failed: {e}");
    }

    // Index into search stores (pipeline → Parquet/Tantivy/LanceDB)
    match kb_sync::index(records).await {
        Ok(0) => log("kb-sync", "no new records"),
        Ok(n) => log("kb-sync", &format!("synced {n} records")),
        Err(e) => tracing::warn!("kb-sync indexing failed: {e}"),
    }

    log("native-fetchers", "fetch cycle complete");

    if let Some(tx) = &event_tx {
        let _ = tx.send(Event::FetchComplete);
    }

    completed
}
