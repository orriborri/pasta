use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use std::fmt::Write;
use tracing::info;

const DEFAULT_LINEAR_API: &str = "linear-api";

pub struct LinearFetcher {
    binary: String,
}

impl Default for LinearFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearFetcher {
    #[must_use]
    pub fn new() -> Self {
        let bin = std::env::var("LINEAR_API_PATH").unwrap_or_else(|_| DEFAULT_LINEAR_API.to_string());
        Self { binary: bin }
    }

    /// Fetch Linear issues assigned to me, updated since last cursor.
    ///
    /// # Errors
    /// Returns error if the linear-api binary fails.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let since = state.cursor("linear_forward")
            .unwrap_or_else(|| {
                (Utc::now() - chrono::Duration::days(30)).to_rfc3339()
            });

        let filter = format!(r#", filter: {{ updatedAt: {{ gte: "{since}" }} }}"#);
        let issues = self.fetch_issues(&filter).await;
        info!(count = issues.len(), "linear issues fetched");

        let now = Utc::now().to_rfc3339();
        state.update_window("linear_forward", &since, &now).ok();

        let records = issues.into_iter().map(|issue| {
            let id = Record::make_id(Source::Linear, &issue.identifier);
            let status = issue.state.as_ref().map_or("", |s| s.name.as_str());
            let project = issue.project.as_ref().map_or("", |p| p.name.as_str());

            let mut content = format!("**Status**: {status} | **Project**: {project}\n\n");
            if let Some(desc) = &issue.description {
                content.push_str(desc);
            }
            if let Some(comments) = &issue.comments {
                for c in &comments.nodes {
                    let who = c.user.as_ref().map_or("?", |u| u.name.as_str());
                    write!(content, "\n\n**{who}**: {}", c.body).ok();
                }
            }

            let updated = issue.updated_at.as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map_or_else(Utc::now, |dt| dt.with_timezone(&Utc));

            Record {
                id,
                source: Source::Linear,
                kind: Kind::Issue,
                title: format!("{} {}", issue.identifier, issue.title),
                content,
                author: String::new(),
                participants: vec![],
                created_at: updated,
                updated_at: updated,
                url: format!("https://linear.app/issue/{}", issue.identifier),
                thread_id: String::new(),
                entities: vec![issue.identifier.clone()],
                tags: vec![status.to_string(), project.to_string()],
            }
        }).collect();

        Ok(records)
    }

    async fn fetch_issues(&self, filter: &str) -> Vec<Issue> {
        let query = format!(
            r"{{ viewer {{ assignedIssues(first: 50{filter}) {{ nodes {{ identifier title description updatedAt state {{ name }} project {{ name }} priority comments {{ nodes {{ body user {{ name }} createdAt }} }} }} }} }} }}"
        );
        let output = tokio::process::Command::new(&self.binary)
            .arg(&query)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .ok();
        let Some(output) = output else { return vec![] };
        if !output.status.success() { return vec![]; }
        let json = String::from_utf8_lossy(&output.stdout);
        let resp: Resp = serde_json::from_str(&json).unwrap_or(Resp { data: None });
        resp.data.and_then(|d| d.viewer).and_then(|v| v.assigned_issues).map(|c| c.nodes).unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct Resp { data: Option<Data> }
#[derive(Deserialize)]
struct Data { viewer: Option<Viewer> }
#[derive(Deserialize)]
struct Viewer { #[serde(rename = "assignedIssues")] assigned_issues: Option<IssueConn> }
#[derive(Deserialize)]
struct IssueConn { nodes: Vec<Issue> }
#[derive(Deserialize)]
struct Issue {
    identifier: String,
    title: String,
    description: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    state: Option<St>,
    project: Option<Proj>,
    #[allow(dead_code)]
    priority: Option<u8>,
    comments: Option<CommentConn>,
}
#[derive(Deserialize)]
struct St { name: String }
#[derive(Deserialize)]
struct Proj { name: String }
#[derive(Deserialize)]
struct CommentConn { nodes: Vec<Comment> }
#[derive(Deserialize)]
struct Comment { body: String, user: Option<CommentUser>, #[allow(dead_code)] #[serde(rename = "createdAt")] created_at: Option<String> }
#[derive(Deserialize)]
struct CommentUser { name: String }
