use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{schemars, tool, tool_router, transport::stdio, ErrorData, ServiceExt};

use kb_core::{EntityRef, RelationKind};
use kb_query::{ContextView, EntityView, EvidenceView, RelatedView, SearchHit, TimelineEntry};
use kb_storage::embedder;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchKnowledgeParams {
    /// Semantic search query
    query: String,
    /// Filter by source: slack, gmail, linear, git, gdocs, calendar, vault
    source: Option<String>,
    /// Max results (default 10)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EntityParams {
    /// Canonical entity reference, e.g. "linear:AB-123" or "person:alice"
    entity: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RelatedParams {
    /// Canonical entity reference, e.g. "linear:AB-123"
    entity: String,
    /// Optional relation kind filter, one of: `references`, `authored_by`,
    /// `participated_in`, `part_of_thread`, `mentions`, `linked_to`
    kind: Option<String>,
    /// Max relations (default 25)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EvidenceParams {
    /// Record ids to resolve to evidence
    record_ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ContextParams {
    /// Canonical entity reference
    entity: String,
    /// Max relations to include (default 25)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TimelineParams {
    /// Canonical entity reference
    entity: String,
    /// Max entries (default 50)
    limit: Option<usize>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct SearchResponse {
    hits: Vec<SearchHit>,
}

fn parse_entity(s: &str) -> Result<EntityRef, ErrorData> {
    s.parse::<EntityRef>().map_err(|e| {
        ErrorData::invalid_params(format!("invalid entity reference '{s}': {e}"), None)
    })
}

fn parse_kind(s: Option<&str>) -> Result<Option<RelationKind>, ErrorData> {
    s.map_or(Ok(None), |k| {
        RelationKind::from_tag(k)
            .map(Some)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown relation kind '{k}'"), None))
    })
}

fn query_err(e: &anyhow::Error) -> ErrorData {
    ErrorData::internal_error(format!("query error: {e}"), None)
}

#[derive(Clone)]
struct KbServer;

#[tool_router(server_handler)]
impl KbServer {
    #[tool(
        description = "Search the knowledge base using hybrid search (semantic + full-text). Returns structured hits, each with record_id, source, title, snippet, url, timestamp, and score."
    )]
    async fn search_knowledge(
        &self,
        Parameters(p): Parameters<SearchKnowledgeParams>,
    ) -> Result<Json<SearchResponse>, ErrorData> {
        let config = pasta_common::config::kb_config();
        embedder::init()
            .await
            .map_err(|e| ErrorData::internal_error(format!("embedder init error: {e}"), None))?;
        let limit = p.limit.unwrap_or(10);
        let hits = kb_query::search(&config, &p.query, p.source.as_deref(), limit)
            .await
            .map_err(|e| query_err(&e))?;
        Ok(Json(SearchResponse { hits }))
    }

    #[tool(
        description = "Resolve a canonical entity reference (e.g. \"linear:AB-123\") to a typed view: whether a backing record exists and its in/out relation degree."
    )]
    async fn get_entity(
        &self,
        Parameters(p): Parameters<EntityParams>,
    ) -> Result<Json<EntityView>, ErrorData> {
        let config = pasta_common::config::kb_config();
        let entity = parse_entity(&p.entity)?;
        let view = kb_query::get_entity(&config, &entity).map_err(|e| query_err(&e))?;
        Ok(Json(view))
    }

    #[tool(
        description = "List the relations connected to an entity (both directions), each with its evidence_record_id and derivation. Optionally filter by relation kind."
    )]
    async fn get_related(
        &self,
        Parameters(p): Parameters<RelatedParams>,
    ) -> Result<Json<RelatedView>, ErrorData> {
        let config = pasta_common::config::kb_config();
        let entity = parse_entity(&p.entity)?;
        let kind = parse_kind(p.kind.as_deref())?;
        let limit = p.limit.unwrap_or(25);
        let view =
            kb_query::get_related(&config, &entity, kind, limit).map_err(|e| query_err(&e))?;
        Ok(Json(view))
    }

    #[tool(
        description = "Resolve evidence record ids to the underlying records (source, kind, title, snippet, url, timestamp). Records absent from the source of truth are omitted."
    )]
    async fn get_evidence(
        &self,
        Parameters(p): Parameters<EvidenceParams>,
    ) -> Result<Json<Vec<EvidenceView>>, ErrorData> {
        let config = pasta_common::config::kb_config();
        let ids: Vec<&str> = p.record_ids.iter().map(String::as_str).collect();
        let views = kb_query::get_evidence(&config, &ids).map_err(|e| query_err(&e))?;
        Ok(Json(views))
    }

    #[tool(
        description = "Assemble an entity context bundle: the entity, its top relations, and the evidence records behind them, in one structured payload."
    )]
    async fn get_context(
        &self,
        Parameters(p): Parameters<ContextParams>,
    ) -> Result<Json<ContextView>, ErrorData> {
        let config = pasta_common::config::kb_config();
        let entity = parse_entity(&p.entity)?;
        let limit = p.limit.unwrap_or(25);
        let view = kb_query::get_context(&config, &entity, limit).map_err(|e| query_err(&e))?;
        Ok(Json(view))
    }

    #[tool(
        description = "Produce a time-ordered list of the evidence records touching an entity (what happened with this entity, in order)."
    )]
    async fn get_timeline(
        &self,
        Parameters(p): Parameters<TimelineParams>,
    ) -> Result<Json<Vec<TimelineEntry>>, ErrorData> {
        let config = pasta_common::config::kb_config();
        let entity = parse_entity(&p.entity)?;
        let limit = p.limit.unwrap_or(50);
        let entries = kb_query::get_timeline(&config, &entity, limit).map_err(|e| query_err(&e))?;
        Ok(Json(entries))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr so it doesn't interfere with MCP stdio
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let service = KbServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
