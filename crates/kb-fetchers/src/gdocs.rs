use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use tracing::{info, warn};

/// Fetches Google Docs via the `gog` CLI.
///
/// Lists changed Docs from Drive, then exports each to Markdown. Reuses `gog`'s
/// stored OAuth credentials (same auth path as the Gmail and Calendar fetchers),
/// so no token handling lives here.
pub struct GdocsFetcher {
    lookback_days: i64,
    max_docs: usize,
}

impl Default for GdocsFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl GdocsFetcher {
    #[must_use]
    pub const fn new() -> Self {
        Self { lookback_days: 30, max_docs: 200 }
    }

    /// Fetch Google Docs modified since the last cursor and export them to Markdown.
    ///
    /// Pagination follows Drive's `nextPageToken` up to `max_docs`. A doc that
    /// fails to export is logged and skipped rather than aborting the whole sync.
    ///
    /// # Errors
    /// Returns an error if the `gog drive ls` invocation cannot be spawned.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let since = state.cursor("gdocs_forward").unwrap_or_else(|| {
            (Utc::now() - chrono::Duration::days(self.lookback_days)).to_rfc3339()
        });

        // Drive query: only Google Docs changed since the cursor.
        let query = format!(
            "mimeType='application/vnd.google-apps.document' and modifiedTime > '{since}'"
        );

        let files = self.list_docs(&query).await?;
        info!(count = files.len(), "gdocs files listed");

        let mut records = Vec::with_capacity(files.len());
        for file in &files {
            match export_doc(&file.id).await {
                Ok(content) if !content.trim().is_empty() => {
                    records.push(file_to_record(file, content));
                }
                Ok(_) => warn!(doc = %file.id, "gdoc export empty, skipped"),
                Err(e) => {
                    warn!(doc = %file.id, error = %e, "gdoc export failed, using metadata-only");
                    let title = file.name.as_deref().unwrap_or("Untitled");
                    let content = format!("[Large document — export limit exceeded]\n\nTitle: {title}");
                    records.push(file_to_record(file, content));
                }
            }
        }

        let now = Utc::now().to_rfc3339();
        state.update_window("gdocs_forward", &since, &now).ok();

        info!(count = records.len(), "gdocs exported");
        Ok(records)
    }

    /// List matching Docs, following `nextPageToken` up to `max_docs`.
    async fn list_docs(&self, query: &str) -> Result<Vec<DriveFile>> {
        const FIELDS: &str = "files(id,name,mimeType,modifiedTime,createdTime,owners(displayName,emailAddress),webViewLink),nextPageToken";

        let mut files: Vec<DriveFile> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut args: Vec<String> = vec![
                "drive".into(), "ls".into(), "--json".into(), "--all".into(),
                "--max".into(), "100".into(),
                "--query".into(), query.into(),
                "--fields".into(), FIELDS.into(),
            ];
            if let Some(token) = &page_token {
                args.push("--page".into());
                args.push(token.clone());
            }

            let output = tokio::process::Command::new("gog")
                .args(&args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("gog drive ls failed: {stderr}");
            }

            let json = String::from_utf8_lossy(&output.stdout);
            let resp: DriveResponse = serde_json::from_str(&json).unwrap_or_default();

            files.extend(resp.files);
            if files.len() >= self.max_docs {
                files.truncate(self.max_docs);
                break;
            }

            match resp.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(files)
    }
}

/// Export a single Doc to Markdown via a temp file, returning its content.
async fn export_doc(doc_id: &str) -> Result<String> {
    let out_path = std::env::temp_dir().join(format!("kb-gdoc-{doc_id}.md"));
    let out_str = out_path.to_string_lossy().to_string();

    let output = tokio::process::Command::new("gog")
        .args(["docs", "export", doc_id, "--format", "md", "--out", &out_str])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gog docs export failed: {stderr}");
    }

    let content = tokio::fs::read_to_string(&out_path).await?;
    tokio::fs::remove_file(&out_path).await.ok();
    Ok(content)
}

fn file_to_record(file: &DriveFile, content: String) -> Record {
    let id = Record::make_id(Source::Gdocs, &file.id);
    let created = parse_time(file.created_time.as_deref());
    let updated = parse_time(file.modified_time.as_deref());

    let owners = file.owners.clone().unwrap_or_default();
    let author = owners
        .first()
        .map(|o| o.display_name.clone().or_else(|| o.email_address.clone()).unwrap_or_default())
        .unwrap_or_default();
    let participants: Vec<String> = owners
        .iter()
        .filter_map(|o| o.email_address.clone())
        .collect();

    Record {
        id,
        source: Source::Gdocs,
        kind: Kind::Doc,
        title: file.name.clone().unwrap_or_default(),
        content,
        author,
        participants,
        created_at: created,
        updated_at: updated,
        url: file.web_view_link.clone().unwrap_or_default(),
        thread_id: String::new(),
        entities: vec![],
        tags: vec![],
    }
}

fn parse_time(s: Option<&str>) -> DateTime<Utc> {
    s.and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map_or_else(Utc::now, |d| d.with_timezone(&Utc))
}

// --- gog Drive JSON response types ---

#[derive(Deserialize, Default)]
struct DriveResponse {
    #[serde(default)]
    files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct DriveFile {
    id: String,
    name: Option<String>,
    #[serde(rename = "modifiedTime")]
    modified_time: Option<String>,
    #[serde(rename = "createdTime")]
    created_time: Option<String>,
    owners: Option<Vec<Owner>>,
    #[serde(rename = "webViewLink")]
    web_view_link: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Owner {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
}
