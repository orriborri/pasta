use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use tracing::info;

const DEFAULT_GLAB: &str = "glab";
const GITLAB_HOST: &str = "gitlab.com";
const REVIEW_REQUEST: &str = "category:review-request";
const AUTHORED: &str = "category:authored";
const TODO: &str = "category:todo";

pub struct GitLabFetcher {
    binary: String,
}

impl Default for GitLabFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl GitLabFetcher {
    #[must_use]
    pub fn new() -> Self {
        let configured = pasta_common::config::get().binaries.glab.trim();
        let binary = if configured.is_empty() {
            DEFAULT_GLAB.to_string()
        } else {
            configured.to_string()
        };
        Self { binary }
    }

    /// Fetch a complete snapshot of the current user's GitLab work.
    ///
    /// A full snapshot is intentional because `.feeds/gitlab.md` must retain
    /// unchanged open merge requests and pending todos. The indexing layer
    /// filters unchanged records by content hash.
    ///
    /// # Errors
    /// Returns an error if `glab` fails or returns invalid JSON.
    pub async fn fetch(&self, _state: &SyncState) -> Result<Vec<Record>> {
        let user: User = self.api("user", false).await?;
        let review_endpoint = format!(
            "merge_requests?scope=all&state=opened&reviewer_id={}&per_page=100",
            user.id
        );
        let authored_endpoint = format!(
            "merge_requests?scope=all&state=opened&author_id={}&per_page=100",
            user.id
        );

        let review_requests: Vec<MergeRequest> = self.api(&review_endpoint, true).await?;
        let authored: Vec<MergeRequest> = self.api(&authored_endpoint, true).await?;
        let todos: Vec<TodoItem> = self.api("todos?state=pending&per_page=100", true).await?;

        let mut records = merge_request_records(review_requests, authored);
        records.extend(todos.iter().map(todo_to_record));
        info!(count = records.len(), "gitlab work items fetched");
        Ok(records)
    }

    async fn api<T: DeserializeOwned>(&self, endpoint: &str, paginate: bool) -> Result<T> {
        let mut command = tokio::process::Command::new(&self.binary);
        command.args(["api", endpoint, "--hostname", GITLAB_HOST]);
        if paginate {
            command.arg("--paginate");
        }
        let output = command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .with_context(|| format!("failed to run {}", self.binary))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("glab api {endpoint} failed: {stderr}");
        }

        serde_json::from_slice(&output.stdout)
            .with_context(|| format!("invalid JSON from glab api {endpoint}"))
    }
}

fn merge_request_records(
    review_requests: Vec<MergeRequest>,
    authored: Vec<MergeRequest>,
) -> Vec<Record> {
    let mut records = BTreeMap::new();
    for mr in review_requests {
        insert_merge_request(&mut records, &mr, REVIEW_REQUEST);
    }
    for mr in authored {
        insert_merge_request(&mut records, &mr, AUTHORED);
    }
    for record in records.values_mut() {
        let categories = record
            .tags
            .iter()
            .filter_map(|tag| tag.strip_prefix("category:"))
            .collect::<Vec<_>>()
            .join(", ");
        write!(record.content, "\n\n**Categories**: {categories}").ok();
    }
    records.into_values().collect()
}

fn insert_merge_request(records: &mut BTreeMap<String, Record>, mr: &MergeRequest, category: &str) {
    let native_id = format!("mr-{}-{}", mr.project_id, mr.iid);
    let id = Record::make_id(Source::GitLab, &native_id);
    if let Some(record) = records.get_mut(&id) {
        if !record.tags.iter().any(|tag| tag == category) {
            record.tags.push(category.to_string());
        }
    } else {
        records.insert(id, merge_request_to_record(mr, category));
    }
}

fn merge_request_to_record(mr: &MergeRequest, category: &str) -> Record {
    let reference = mr
        .references
        .as_ref()
        .and_then(|references| references.full.as_deref())
        .map_or_else(|| format!("!{}", mr.iid), ToString::to_string);
    let author = mr
        .author
        .as_ref()
        .map(|author| author.username.clone())
        .unwrap_or_default();
    let description = mr.description.as_deref().unwrap_or_default();
    let content = format!(
        "**State**: {} | **Draft**: {}\n\n{}",
        mr.state, mr.draft, description
    );

    Record {
        id: Record::make_id(Source::GitLab, &format!("mr-{}-{}", mr.project_id, mr.iid)),
        source: Source::GitLab,
        kind: Kind::Issue,
        title: format!("{reference} — {}", mr.title),
        content,
        author,
        participants: vec![],
        created_at: mr.created_at,
        updated_at: mr.updated_at,
        url: mr.web_url.clone(),
        thread_id: String::new(),
        entities: vec![reference],
        tags: vec![category.to_string(), format!("state:{}", mr.state)],
    }
}

fn todo_to_record(todo: &TodoItem) -> Record {
    let target_title = todo
        .target
        .as_ref()
        .and_then(|target| target.title.as_deref())
        .or(todo.body.as_deref())
        .unwrap_or("GitLab todo");
    let project = todo
        .project
        .as_ref()
        .and_then(|project| project.path_with_namespace.as_deref())
        .unwrap_or("GitLab");
    let action = todo.action_name.replace('_', " ");
    let url = todo
        .target_url
        .clone()
        .or_else(|| {
            todo.target
                .as_ref()
                .and_then(|target| target.web_url.clone())
        })
        .unwrap_or_default();

    Record {
        id: Record::make_id(Source::GitLab, &format!("todo-{}", todo.id)),
        source: Source::GitLab,
        kind: Kind::Issue,
        title: format!("{action}: {project} — {target_title}"),
        content: todo.body.clone().unwrap_or_default(),
        author: todo
            .author
            .as_ref()
            .map(|author| author.username.clone())
            .unwrap_or_default(),
        participants: vec![],
        created_at: todo.created_at,
        updated_at: todo.updated_at.unwrap_or(todo.created_at),
        url,
        thread_id: String::new(),
        entities: vec![],
        tags: vec![TODO.to_string(), format!("action:{}", todo.action_name)],
    }
}

#[derive(Deserialize)]
struct User {
    id: u64,
}

#[derive(Deserialize)]
struct MergeRequest {
    iid: u64,
    project_id: u64,
    title: String,
    description: Option<String>,
    state: String,
    #[serde(default)]
    draft: bool,
    web_url: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    author: Option<GitLabUser>,
    references: Option<References>,
}

#[derive(Deserialize)]
struct References {
    full: Option<String>,
}

#[derive(Deserialize)]
struct GitLabUser {
    username: String,
}

#[derive(Deserialize)]
struct TodoItem {
    id: u64,
    action_name: String,
    body: Option<String>,
    target_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
    author: Option<GitLabUser>,
    project: Option<Project>,
    target: Option<TodoTarget>,
}

#[derive(Deserialize)]
struct Project {
    path_with_namespace: Option<String>,
}

#[derive(Deserialize)]
struct TodoTarget {
    title: Option<String>,
    web_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MR: &str = r#"{
        "iid": 42,
        "project_id": 7,
        "title": "Ship GitLab feed",
        "description": "Keep the feed current",
        "state": "opened",
        "draft": false,
        "web_url": "https://gitlab.example/group/project/-/merge_requests/42",
        "created_at": "2026-09-13T06:00:00Z",
        "updated_at": "2026-09-14T06:00:00Z",
        "author": { "username": "orre" },
        "references": { "full": "group/project!42" }
    }"#;

    #[test]
    fn overlapping_review_and_authored_merge_request_is_one_categorized_record() {
        let review: MergeRequest = serde_json::from_str(MR).unwrap();
        let authored: MergeRequest = serde_json::from_str(MR).unwrap();

        let records = merge_request_records(vec![review], vec![authored]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "gitlab-mr-7-42");
        assert_eq!(records[0].source, Source::GitLab);
        assert!(records[0].tags.contains(&REVIEW_REQUEST.to_string()));
        assert!(records[0].tags.contains(&AUTHORED.to_string()));
        assert_eq!(
            records[0].url,
            "https://gitlab.example/group/project/-/merge_requests/42"
        );
    }

    #[test]
    fn category_changes_affect_record_content_hash() {
        let authored_only: MergeRequest = serde_json::from_str(MR).unwrap();
        let both_review_and_authored: MergeRequest = serde_json::from_str(MR).unwrap();
        let also_authored: MergeRequest = serde_json::from_str(MR).unwrap();

        let authored = merge_request_records(vec![], vec![authored_only]);
        let both = merge_request_records(vec![both_review_and_authored], vec![also_authored]);

        assert_ne!(authored[0].content_hash(), both[0].content_hash());
    }

    #[test]
    fn pending_todo_maps_to_todo_category() {
        let todo: TodoItem = serde_json::from_str(
            r#"{
                "id": 99,
                "action_name": "mentioned",
                "body": "Please check this",
                "target_url": "https://gitlab.example/group/project/-/issues/8",
                "created_at": "2026-09-14T05:00:00Z",
                "author": { "username": "teammate" },
                "project": { "path_with_namespace": "group/project" },
                "target": { "title": "Investigate issue" }
            }"#,
        )
        .unwrap();

        let record = todo_to_record(&todo);

        assert_eq!(record.id, "gitlab-todo-99");
        assert!(record.tags.contains(&TODO.to_string()));
        assert_eq!(record.author, "teammate");
        assert!(record.title.contains("group/project"));
        assert_eq!(
            record.url,
            "https://gitlab.example/group/project/-/issues/8"
        );
    }
}
