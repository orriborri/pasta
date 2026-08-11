//! Reproduces `STORAGE_EVIDENCE.md` §6.3 (Tantivy-only vs hybrid latency/quality)
//! and §6.4 (full-corpus `read_all` cost) against the configured kb store.
//!
//! This is a measurement harness, NOT a test: it reads the live `~/.kb` store,
//! so its numbers depend on the local corpus and are deliberately not asserted
//! in CI. It exists so the figures in `STORAGE_EVIDENCE.md` §6.3/§6.4 are
//! reproducible rather than resting on a throwaway benchmark.
//!
//! Run (release build, index warmed):
//!
//! ```text
//! cargo run -p kb-storage --example storage_evidence --release
//! ```
//!
//! Point it at a store other than the default `~/.kb` via an env var:
//!
//! ```text
//! KB_DATA_DIR=/path/to/.kb cargo run -p kb-storage --example storage_evidence --release
//! ```

use std::collections::HashSet;
use std::time::Instant;

use kb_core::KbConfig;
use kb_storage::{embedder, hybrid_search, ParquetStore, TextIndex};

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Same resolution rule as production (honours a configured data dir),
    // defaulting to ~/.kb — the store the evidence document measured.
    let config = std::env::var("KB_DATA_DIR")
        .map_or_else(|_| KbConfig::default(), |dir| KbConfig { data_dir: dir.into() });
    println!("kb data_dir: {}", config.data_dir.display());

    embedder::init().await?;

    // The same five terms §6.3 used: all appear in indexed content except
    // `standup`, which is the zero-lexical-match case the hybrid leg rescues.
    let queries = ["meeting", "deploy", "pipeline", "review", "standup"];
    let limit = 10usize;

    // Warm both read paths once; first-call cost is excluded from the table.
    let text = TextIndex::open(&config)?;
    let _ = text.search(queries[0], limit)?;
    let _ = hybrid_search(&config, queries[0], limit).await?;

    println!("\n§6.3  Tantivy-only vs hybrid  (limit {limit}, warmed)");
    println!(
        "{:<10} {:>7} {:>9} {:>7} {:>9} {:>18}",
        "query", "t-hits", "t-ms", "h-hits", "h-ms", "hybrid-only hits"
    );
    for q in queries {
        let t0 = Instant::now();
        let t_hits = text.search(q, limit)?;
        let t_ms = ms_since(t0);

        let h0 = Instant::now();
        let h_hits = hybrid_search(&config, q, limit).await?;
        let h_ms = ms_since(h0);

        // Hybrid results whose id is not in Tantivy's own top-`limit`.
        let t_ids: HashSet<&str> = t_hits.iter().map(|r| r.id.as_str()).collect();
        let hybrid_only = h_hits.iter().filter(|r| !t_ids.contains(r.id.as_str())).count();

        println!(
            "{:<10} {:>7} {:>8.1} {:>7} {:>8.1} {:>18}",
            q, t_hits.len(), t_ms, h_hits.len(), h_ms, hybrid_only
        );
    }

    // §6.4: full-corpus read_all, no caching / no file-level pushdown. Run 0 is
    // cold-ish (whatever the OS page cache holds); later runs are warm.
    println!("\n§6.4  ParquetStore::read_all  (run 0 = cold, 1.. = warm)");
    for i in 0..4 {
        let r0 = Instant::now();
        let n = ParquetStore::new(&config).read_all()?.len();
        println!("  run {i}: {n} records in {:.1} ms", ms_since(r0));
    }

    Ok(())
}
