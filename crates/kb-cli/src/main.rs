use anyhow::Result;
use axum::{Json, Router, extract::Query, routing::get};
use clap::{Parser, Subcommand};
use kb_core::{KbConfig, Record, SyncState};
use kb_storage::{embedder, hybrid_search, GraphStore, ParquetStore, TextIndex, VectorStore};
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::info;

#[derive(Parser)]
#[command(name = "kb", about = "Knowledge base engine")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fetch from sources and store (Parquet → `LanceDB` + Tantivy)
    Sync {
        /// Sources to sync: slack, gmail, linear, git, vault, calendar, gdocs (comma-separated, default: all)
        #[arg(short, long)]
        source: Option<String>,
    },
    /// Hybrid search (vector + full-text + RRF merge)
    Search {
        query: Vec<String>,
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// List records created within the last N days as JSON (for pasta inbox processing)
    Recent {
        /// Look back this many days
        #[arg(long, default_value = "2")]
        days: i64,
        /// Only this source (slack, gmail, linear, ...)
        #[arg(short, long)]
        source: Option<String>,
    },
    /// Rebuild `LanceDB` + Tantivy from Parquet (source of truth)
    Reindex,
    /// Rebuild only the evidence graph (`graph.db`) from Parquet — no re-embedding
    GraphRebuild,
    /// Re-run pipeline on all Parquet data and rebuild indexes
    Reprocess,
    /// Start HTTP API server
    Serve {
        #[arg(short, long, default_value = "3030")]
        port: u16,
    },
    /// Resolve an entity to a typed view (presence + relation degree)
    Entity {
        /// Canonical entity reference, e.g. "linear:AB-123"
        entity: String,
        #[arg(long)]
        json: bool,
    },
    /// List the relations connected to an entity
    Related {
        /// Canonical entity reference
        entity: String,
        /// Relation kind filter, one of: `references`, `authored_by`, `participated_in`, `part_of_thread`, `mentions`, `linked_to`
        #[arg(short, long)]
        kind: Option<String>,
        #[arg(short, long, default_value = "25")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Resolve evidence record ids to the underlying records
    Evidence {
        /// One or more record ids
        record_ids: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show an entity context bundle (entity + relations + evidence)
    Context {
        /// Canonical entity reference
        entity: String,
        #[arg(short, long, default_value = "25")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show a time-ordered timeline of records touching an entity
    Timeline {
        /// Canonical entity reference
        entity: String,
        #[arg(short, long, default_value = "50")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = pasta_common::config::kb_config();

    match cli.command {
        Cmd::Sync { source } => cmd_sync(&config, source.as_deref()).await,
        Cmd::Search { query, limit } => cmd_search(&config, &query.join(" "), limit).await,
        Cmd::Recent { days, source } => cmd_recent(&config, days, source.as_deref()),
        Cmd::Reindex => cmd_reindex(&config).await,
        Cmd::GraphRebuild => cmd_graph_rebuild(&config),
        Cmd::Reprocess => cmd_reprocess(&config).await,
        Cmd::Serve { port } => cmd_serve(&config, port).await,
        Cmd::Entity { entity, json } => cmd_entity(&config, &entity, json),
        Cmd::Related { entity, kind, limit, json } => cmd_related(&config, &entity, kind.as_deref(), limit, json),
        Cmd::Evidence { record_ids, json } => cmd_evidence(&config, &record_ids, json),
        Cmd::Context { entity, limit, json } => cmd_context(&config, &entity, limit, json),
        Cmd::Timeline { entity, limit, json } => cmd_timeline(&config, &entity, limit, json),
    }
}

/// Shared: embed records in batches and upsert to `LanceDB`.
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

/// Rebuild the evidence graph (`graph.db`) from records. Derives relations
/// deterministically and fully rebuilds the derived index. Called by reindex and
/// reprocess so the graph never diverges from Parquet.
fn rebuild_graph(config: &KbConfig, records: &[Record]) -> Result<()> {
    kb_storage::remove_graph_db(config)?;
    let relations = kb_pipeline::relations_from_records(records);
    let graph = GraphStore::open(config)?;
    graph.rebuild(records, &relations)?;
    info!(entities = graph.entity_count()?, relations = graph.relation_count()?, "graph rebuilt");
    Ok(())
}

async fn cmd_sync(_config: &KbConfig, source_filter: Option<&str>) -> Result<()> {
    let sources: Vec<&str> = source_filter.map_or_else(
        || kb_sync::ALL_SOURCES.to_vec(),
        |s| s.split(',').collect(),
    );
    let count = kb_sync::run(&sources).await?;
    if count == 0 {
        println!("No new records to sync.");
    } else {
        println!("✓ Synced {} records (model: {})", count, embedder::model_name());
    }
    Ok(())
}

async fn cmd_search(config: &KbConfig, query: &str, limit: usize) -> Result<()> {
    embedder::init().await?;
    let results = hybrid_search(config, query, limit).await?;
    let results: Vec<_> = results.into_iter().filter(|r| !r.content.trim().is_empty()).collect();

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    for (i, r) in results.iter().enumerate() {
        println!("─── {} (score: {:.4}) ───", i + 1, r.score);
        println!("source: {} | {} | {}", r.source, r.created_at, r.title);
        println!("{}\n", r.content.trim());
    }
    Ok(())
}

fn cmd_recent(config: &KbConfig, days: i64, source: Option<&str>) -> Result<()> {
    use chrono::{Duration, Utc};
    let cutoff = Utc::now() - Duration::days(days);
    let parquet = ParquetStore::new(config);
    let mut records: Vec<Record> = parquet.read_all()?
        .into_iter()
        .filter(|r| r.created_at >= cutoff)
        .filter(|r| source.is_none_or(|s| r.source.to_string() == s))
        .collect();
    records.sort_by_key(|r| std::cmp::Reverse(r.created_at));
    println!("{}", serde_json::to_string(&records)?);
    Ok(())
}

async fn cmd_reindex(config: &KbConfig) -> Result<()> {
    let model = embedder::init().await?;
    info!(?model, "embedder initialized");

    let state = SyncState::open(config)?;
    state.clear_model()?;
    state.lock_model(embedder::model_name())?;

    let parquet = ParquetStore::new(config);
    let records = parquet.read_all()?;
    info!(count = records.len(), "records read from parquet");

    if records.is_empty() {
        println!("No records in Parquet to reindex.");
        return Ok(());
    }

    let text_index = TextIndex::clear(config)?;
    text_index.upsert(&records)?;
    info!("tantivy rebuilt");

    let vectors_dir = config.vectors_dir();
    if vectors_dir.exists() {
        fs::remove_dir_all(&vectors_dir)?;
    }
    let vector = VectorStore::new(config);
    embed_and_upsert(&vector, &records).await?;

    // Rebuild the evidence graph derived index from the same Parquet records.
    rebuild_graph(config, &records)?;

    println!("✓ Reindexed {} records (model: {})", records.len(), embedder::model_name());
    Ok(())
}

async fn cmd_reprocess(config: &KbConfig) -> Result<()> {
    let model = embedder::init().await?;
    info!(?model, "embedder initialized");

    let state = SyncState::open(config)?;
    state.clear_model()?;
    state.lock_model(embedder::model_name())?;

    // 1. Read all raw records from Parquet
    let parquet = ParquetStore::new(config);
    let records = parquet.read_all()?;
    info!(count = records.len(), "records read from parquet");

    if records.is_empty() {
        println!("No records in Parquet to reprocess.");
        return Ok(());
    }

    // 2. Run full pipeline
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
    let records = pipeline.run(records);
    info!(after_pipeline = records.len(), "pipeline complete");

    // 3. Rebuild Tantivy
    let text_index = TextIndex::clear(config)?;
    text_index.upsert(&records)?;
    info!("tantivy rebuilt");

    // 4. Rebuild LanceDB
    let vectors_dir = config.vectors_dir();
    if vectors_dir.exists() {
        fs::remove_dir_all(&vectors_dir)?;
    }
    let vector = VectorStore::new(config);
    embed_and_upsert(&vector, &records).await?;

    // Rebuild the evidence graph derived index from the post-pipeline records.
    rebuild_graph(config, &records)?;

    println!("✓ Reprocessed {} records through pipeline (model: {})", records.len(), embedder::model_name());

    Ok(())
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    source: Option<String>,
}

const fn default_limit() -> usize { 10 }

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchHit>,
}

#[derive(Serialize)]
struct SearchHit {
    id: String,
    source: String,
    title: String,
    content: String,
    created_at: String,
    score: f32,
}

/// Parse a canonical `EntityRef` from a CLI string argument.
fn parse_entity(s: &str) -> Result<kb_core::EntityRef> {
    s.parse::<kb_core::EntityRef>().map_err(|e| anyhow::anyhow!("{e}"))
}

fn cmd_entity(config: &KbConfig, entity: &str, json: bool) -> Result<()> {
    let entity = parse_entity(entity)?;
    let view = kb_query::get_entity(config, &entity)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        println!(
            "{} (kind: {}) — present: {}, out: {}, in: {}",
            view.entity, view.kind, view.present, view.out_degree, view.in_degree
        );
    }
    Ok(())
}

fn cmd_related(config: &KbConfig, entity: &str, kind: Option<&str>, limit: usize, json: bool) -> Result<()> {
    let entity = parse_entity(entity)?;
    let kind_filter = match kind {
        None => None,
        Some(k) => Some(
            kb_core::RelationKind::from_tag(k)
                .ok_or_else(|| anyhow::anyhow!("unknown relation kind '{k}'"))?,
        ),
    };
    let view = kb_query::get_related(config, &entity, kind_filter, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        println!("{} — {} relation(s):", view.entity.entity, view.relations.len());
        for r in &view.relations {
            println!("  {} --{}--> {}  [evidence: {}, {}]", r.subject, r.predicate, r.object, r.evidence_record_id, r.derivation);
        }
    }
    Ok(())
}

fn cmd_evidence(config: &KbConfig, record_ids: &[String], json: bool) -> Result<()> {
    let ids: Vec<&str> = record_ids.iter().map(String::as_str).collect();
    let views = kb_query::get_evidence(config, &ids)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&views)?);
    } else if views.is_empty() {
        println!("No evidence records found.");
    } else {
        for e in &views {
            println!("─── {} [{}] {} ───", e.record_id, e.source, e.created_at);
            println!("{}", e.title);
            if !e.snippet.trim().is_empty() {
                println!("{}", e.snippet.trim());
            }
            println!();
        }
    }
    Ok(())
}

fn cmd_context(config: &KbConfig, entity: &str, limit: usize, json: bool) -> Result<()> {
    let entity = parse_entity(entity)?;
    let ctx = kb_query::get_context(config, &entity, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ctx)?);
    } else {
        println!(
            "{} (present: {}) — {} relation(s), {} evidence record(s)",
            ctx.entity.entity, ctx.entity.present, ctx.relations.len(), ctx.evidence.len()
        );
        for r in &ctx.relations {
            println!("  {} --{}--> {}  [{}]", r.subject, r.predicate, r.object, r.evidence_record_id);
        }
        for e in &ctx.evidence {
            println!("  evidence {} [{}]: {}", e.record_id, e.source, e.title);
        }
    }
    Ok(())
}

fn cmd_timeline(config: &KbConfig, entity: &str, limit: usize, json: bool) -> Result<()> {
    let entity = parse_entity(entity)?;
    let entries = kb_query::get_timeline(config, &entity, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        println!("No timeline entries.");
    } else {
        for e in &entries {
            println!("{}  {} [{}] {}", e.at, e.record.record_id, e.record.source, e.record.title);
        }
    }
    Ok(())
}

/// Rebuild the evidence graph (`graph.db`) from Parquet without touching the
/// vector/text indexes or re-embedding. The graph is a derived index, so this is
/// always safe to run.
fn cmd_graph_rebuild(config: &KbConfig) -> Result<()> {
    let parquet = ParquetStore::new(config);
    let records = parquet.read_all()?;
    info!(count = records.len(), "records read from parquet");
    if records.is_empty() {
        println!("No records in Parquet to build a graph from.");
        return Ok(());
    }
    rebuild_graph(config, &records)?;
    let graph = GraphStore::open(config)?;
    println!(
        "✓ Rebuilt evidence graph: {} entities, {} relations",
        graph.entity_count()?,
        graph.relation_count()?
    );
    Ok(())
}

async fn cmd_serve(config: &KbConfig, port: u16) -> Result<()> {
    embedder::init().await?;

    let cfg = config.clone();
    let app = Router::new()
        .route("/search", get(move |q: Query<SearchQuery>| {
            let cfg = cfg.clone();
            async move {
                let results = hybrid_search(&cfg, &q.q, q.limit).await.unwrap_or_default();
                let hits: Vec<SearchHit> = results.into_iter()
                    .filter(|r| {
                        if let Some(ref src) = q.source {
                            return r.source == *src;
                        }
                        true
                    })
                    .map(|r| SearchHit {
                        id: r.id, source: r.source, title: r.title,
                        content: r.content, created_at: r.created_at, score: r.score,
                    })
                    .collect();
                Json(SearchResponse { results: hits })
            }
        }))
        .route("/health", get(|| async { "ok" }));

    let addr = format!("0.0.0.0:{port}");
    println!("kb serve listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
