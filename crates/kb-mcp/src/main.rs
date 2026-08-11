use rmcp::{ServiceExt, schemars, tool, tool_router, transport::stdio};
use rmcp::handler::server::wrapper::Parameters;

use kb_storage::{embedder, hybrid_search};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchKnowledgeParams {
    /// Semantic search query
    query: String,
    /// Filter by source: slack, gmail, linear, git, gdocs, calendar, vault
    source: Option<String>,
    /// Max results (default 10)
    limit: Option<usize>,
}

#[derive(Clone)]
struct KbServer;

#[tool_router(server_handler)]
impl KbServer {
    #[tool(description = "Search the knowledge base using hybrid search (semantic + full-text). Returns relevant messages, threads, issues, and documents.")]
    async fn search_knowledge(&self, Parameters(p): Parameters<SearchKnowledgeParams>) -> String {
        let config = pasta_common::config::kb_config();
        if let Err(e) = embedder::init().await {
            return format!("Embedder init error: {e}");
        }

        let limit = p.limit.unwrap_or(10);
        match hybrid_search(&config, &p.query, limit).await {
            Ok(results) => {
                let filtered: Vec<_> = if let Some(ref src) = p.source {
                    results.into_iter().filter(|r| r.source == *src).collect()
                } else {
                    results
                };

                if filtered.is_empty() {
                    return "No results found.".to_string();
                }

                filtered.iter().map(|r| {
                    let snippet: String = r.content.chars().take(400).collect();
                    format!("[{}] {} | {}\n{}", r.source, r.created_at, r.title, snippet)
                }).collect::<Vec<_>>().join("\n\n")
            }
            Err(e) => format!("Search error: {e}"),
        }
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
