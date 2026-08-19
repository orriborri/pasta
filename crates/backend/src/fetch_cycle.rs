use pasta_common::ipc::Event;

use crate::state::EventTx;
use crate::util::log;

/// Run a complete fetch cycle: fetch via kb-fetchers, write `.feeds/` markdown,
/// index into search stores, and generate the daily note. Called by both the
/// scheduler and the ForceFetch command.
pub async fn run(event_tx: Option<EventTx>) {
    let sources = &["gmail", "linear", "calendar", "slack"];

    // Fetch records using kb-fetchers (single pass) with timeout
    let records = match tokio::time::timeout(
        tokio::time::Duration::from_secs(300),
        kb_sync::fetch(sources),
    ).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            log("native-fetchers", &format!("fetch failed: {e}"));
            return;
        }
        Err(_) => {
            log("native-fetchers", "fetch timed out (5min)");
            return;
        }
    };

    // Write .feeds/ markdown BEFORE indexing (feeds update even if indexing fails)
    kb_sync::write_feeds(&records);

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
}
