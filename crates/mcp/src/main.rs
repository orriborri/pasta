use rmcp::{ServiceExt, schemars, tool, tool_router, transport::stdio};
use rmcp::handler::server::wrapper::Parameters;

use std::path::Path;

use pasta_common::config;
use pasta_common::vault;

use kb_core::KbConfig;
use kb_storage::{embedder, hybrid_search};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchHistoryParams {
    /// Semantic search query
    query: String,
    /// Filter by source: slack, gmail, linear, wiki, git, repos
    source: Option<String>,
    /// Filter by participant name
    participant: Option<String>,
    /// Only results after this date (YYYY-MM-DD)
    after: Option<String>,
    /// Max results (default 10)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchTasksParams {
    /// Filter by status: open, done, blocked
    status: Option<String>,
    /// Filter by priority: high, medium, low
    priority: Option<String>,
    /// Filter by project name
    project: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetPeopleParams {
    /// Filter by person name
    name: Option<String>,
}

#[derive(Clone)]
struct PastaServer;

#[tool_router(server_handler)]
impl PastaServer {
    #[tool(description = "Semantic search over indexed history (Slack, Gmail, Linear, wiki, git commits, code)")]
    async fn search_history(&self, Parameters(p): Parameters<SearchHistoryParams>) -> String {
        let kb_config = KbConfig::default();
        if let Err(e) = embedder::init().await {
            return format!("Embedder init error: {e}");
        }

        let limit = p.limit.unwrap_or(10);
        match hybrid_search(&kb_config, &p.query, limit).await {
            Ok(results) => {
                let filtered: Vec<_> = results.into_iter()
                    .filter(|r| {
                        if let Some(ref sf) = p.source {
                            if !sf.split(',').any(|s| s == r.source) { return false; }
                        }
                        if let Some(ref af) = p.after {
                            if r.created_at < *af { return false; }
                        }
                        // Participant filter on content (substring)
                        if let Some(ref pf) = p.participant {
                            if !r.content.to_lowercase().contains(&pf.to_lowercase())
                                && !r.title.to_lowercase().contains(&pf.to_lowercase()) {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();

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

    #[tool(description = "List vault tasks, optionally filtered by status/priority/project")]
    fn search_tasks(&self, Parameters(p): Parameters<SearchTasksParams>) -> String {
        let tasks = vault::load_tasks();
        let filtered: Vec<_> = tasks.iter().filter(|t| {
            if let Some(ref s) = p.status {
                if !t.status.to_lowercase().contains(&s.to_lowercase()) { return false; }
            }
            if let Some(ref pr) = p.priority {
                if !t.priority.to_lowercase().contains(&pr.to_lowercase()) { return false; }
            }
            if let Some(ref proj) = p.project {
                if !t.project.to_lowercase().contains(&proj.to_lowercase()) { return false; }
            }
            true
        }).collect();

        if filtered.is_empty() { return "No tasks found.".to_string(); }
        filtered.iter().map(|t| {
            format!("- [{}] {} (priority: {}, project: {})", t.status, t.title, t.priority, t.project)
        }).collect::<Vec<_>>().join("\n")
    }

    #[tool(description = "Get current feed state from all sources (GitLab, Linear, Slack, Gmail)")]
    fn get_feeds(&self) -> String {
        let feeds = vault::load_feeds();
        if feeds.is_empty() { return "No feeds available.".to_string(); }
        feeds.iter().map(|f| {
            format!("## {} (fetched: {})\n{}", f.source, f.fetched, f.body.chars().take(500).collect::<String>())
        }).collect::<Vec<_>>().join("\n\n")
    }

    #[tool(description = "Get people with their identifiers and items you're waiting on from them")]
    fn get_people(&self, Parameters(p): Parameters<GetPeopleParams>) -> String {
        let people = vault::load_people();
        let filtered: Vec<_> = people.iter().filter(|person| {
            if let Some(ref n) = p.name {
                person.name.to_lowercase().contains(&n.to_lowercase())
            } else { true }
        }).collect();

        if filtered.is_empty() { return "No people found.".to_string(); }
        filtered.iter().map(|person| {
            let ids = person.identifiers.iter()
                .map(|(svc, handle)| format!("{svc}:{handle}")).collect::<Vec<_>>().join(", ");
            let waiting = if person.waiting_on.is_empty() { String::new() }
                else { format!("\n  Waiting on: {}", person.waiting_on.join(", ")) };
            format!("**{}** ({ids}){waiting}", person.name)
        }).collect::<Vec<_>>().join("\n")
    }

    #[tool(description = "Get today's daily note content")]
    fn get_daily(&self) -> String {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let path = Path::new(vault::vault_path()).join(format!("0. Inbox/Daily/{today}.md"));
        std::fs::read_to_string(&path).unwrap_or_else(|_| "No daily note found for today.".to_string())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    config::init();
    let service = PastaServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
