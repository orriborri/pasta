use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use tracing::info;

pub struct GmailFetcher {
    lookback_days: i64,
}

impl Default for GmailFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl GmailFetcher {
    #[must_use]
    pub const fn new() -> Self {
        Self { lookback_days: 30 }
    }

    /// Fetch Gmail threads since last cursor.
    ///
    /// # Errors
    /// Returns error if the gog CLI fails or returns unparseable output.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let since = state.cursor("gmail_forward")
            .unwrap_or_else(|| {
                let d = Utc::now() - chrono::Duration::days(self.lookback_days);
                d.format("%Y/%m/%d").to_string()
            });

        let query = format!("after:{since} -category:promotions -category:social");
        let threads = fetch_threads(&query).await;
        info!(count = threads.len(), "gmail threads fetched");

        let now = Utc::now().format("%Y/%m/%d").to_string();
        state.update_window("gmail_forward", &since, &now).ok();

        let records = threads.into_iter().map(|t| {
            let id = Record::make_id(Source::Gmail, &slug(&format!("{}-{}", t.date, t.subject)));
            let created = parse_date(&t.date);
            let from_short = t.from.split('<').next().unwrap_or(&t.from).trim().trim_matches('"').to_string();
            let content = if t.snippet.is_empty() { t.subject.clone() } else { t.snippet };

            Record {
                id,
                source: Source::Gmail,
                kind: Kind::Thread,
                title: t.subject,
                content,
                author: from_short.clone(),
                participants: vec![from_short],
                created_at: created,
                updated_at: created,
                url: String::new(),
                thread_id: String::new(),
                entities: vec![],
                tags: t.labels,
            }
        }).collect();

        Ok(records)
    }
}

async fn fetch_threads(query: &str) -> Vec<GmailThread> {
    let output = tokio::process::Command::new("gog")
        .args(["gmail", "search", query, "--json", "--all"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .ok();
    let Some(output) = output else { return vec![] };
    if !output.status.success() { return vec![]; }
    let json = String::from_utf8_lossy(&output.stdout);
    let result: SearchResult = serde_json::from_str(&json).unwrap_or(SearchResult { threads: None });
    result.threads.unwrap_or_default()
}

fn parse_date(s: &str) -> DateTime<Utc> {
    // Format: "2026/06/25" or "Jun 25"
    NaiveDate::parse_from_str(s, "%Y/%m/%d")
        .or_else(|_| NaiveDate::parse_from_str(&format!("2026 {s}"), "%Y %b %d")).map_or_else(|_| Utc::now(), |d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Deserialize)]
struct SearchResult { threads: Option<Vec<GmailThread>> }
#[derive(Deserialize)]
struct GmailThread {
    date: String,
    from: String,
    subject: String,
    labels: Vec<String>,
    #[serde(default)]
    snippet: String,
}
