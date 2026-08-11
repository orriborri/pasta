use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::info;

const DEFAULT_SLACK_API: &str = "slack-api";

pub struct SlackFetcher {
    slack_api: String,
    /// How many days back to fetch on first run.
    lookback_days: i64,
}

impl Default for SlackFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SlackFetcher {
    #[must_use]
    pub fn new() -> Self {
        let bin = std::env::var("SLACK_API_PATH")
            .unwrap_or_else(|_| DEFAULT_SLACK_API.to_string());
        Self { slack_api: bin, lookback_days: 30 }
    }

    #[must_use]
    pub const fn with_lookback_days(mut self, days: i64) -> Self {
        self.lookback_days = days;
        self
    }

    /// Fetch new Slack messages. Resolves user names after fetching.
    ///
    /// # Errors
    /// Returns error if Slack API is unreachable.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        let fallback_oldest = oldest_ts(self.lookback_days);

        // Fetch DMs
        if let Some(convs) = self.list_convs("im", 50).await {
            info!(count = convs.len(), "checking slack DMs for new messages");
            for conv in &convs {
                let oldest = state.cursor(&conv.id).unwrap_or_else(|| fallback_oldest.clone());
                let msgs = self.fetch_history(&conv.id, &oldest).await;
                if !msgs.is_empty() {
                    let first_ts = msgs.first().and_then(|m| m.ts.as_deref()).unwrap_or("");
                    let last_ts = msgs.last().and_then(|m| m.ts.as_deref()).unwrap_or("");
                    state.update_window(&conv.id, first_ts, last_ts).ok();
                    let person = conv.user.as_deref().unwrap_or("unknown");
                    info!(person = %person, messages = msgs.len(), "new DMs");
                    for msg in msgs {
                        records.push(msg_to_record(&conv.id, &msg, person, "dm"));
                    }
                }
            }
        }

        // Fetch channels
        if let Some(convs) = self.list_convs("public_channel,private_channel", 100).await {
            info!(count = convs.len(), "checking slack channels for new messages");
            let mut fetched_count = 0;
            for conv in &convs {
                let ch_name = conv.name.as_deref().unwrap_or("unknown");
                let oldest = state.cursor(&conv.id).unwrap_or_else(|| fallback_oldest.clone());
                let msgs = self.fetch_history(&conv.id, &oldest).await;
                if !msgs.is_empty() {
                    fetched_count += 1;
                    info!(channel = %ch_name, messages = msgs.len(), "new messages");
                    let first_ts = msgs.first().and_then(|m| m.ts.as_deref()).unwrap_or("");
                    let last_ts = msgs.last().and_then(|m| m.ts.as_deref()).unwrap_or("");
                    state.update_window(&conv.id, first_ts, last_ts).ok();
                    for msg in msgs {
                        records.push(msg_to_record(&conv.id, &msg, ch_name, "channel"));
                    }
                }
            }
            info!(channels_with_new = fetched_count, total_checked = convs.len(), "channels done");
        }

        // Resolve user IDs to names in bulk
        self.resolve_authors(&mut records).await;

        info!(total = records.len(), "slack fetch complete");
        Ok(records)
    }

    async fn list_convs(&self, types: &str, limit: usize) -> Option<Vec<Conv>> {
        let json = self.run_cmd(&[
            "conversations.list",
            &format!("types={types}"),
            &format!("limit={limit}"),
            "exclude_archived=true",
        ]).await?;
        let resp: ConvListResp = serde_json::from_str(&json).ok()?;
        if resp.ok { resp.channels } else { None }
    }

    async fn fetch_history(&self, channel: &str, oldest: &str) -> Vec<SlackMsg> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..5 {
            let mut args = vec![
                "conversations.history".to_string(),
                format!("channel={channel}"),
                format!("oldest={oldest}"),
                "limit=200".to_string(),
            ];
            if let Some(c) = &cursor {
                args.push(format!("cursor={c}"));
            }
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let Some(json) = self.run_cmd(&arg_refs).await else { break };
            let Ok(resp) = serde_json::from_str::<HistResp>(&json) else { break };
            if !resp.ok { break; }
            if let Some(msgs) = resp.messages {
                for m in msgs {
                    all.push(SlackMsg {
                        ts: m.ts,
                        author: m.user.unwrap_or_default(),
                        text: m.text.unwrap_or_default(),
                    });
                }
            }
            if resp.has_more.unwrap_or(false) {
                cursor = resp.response_metadata.and_then(|rm| rm.next_cursor).filter(|c| !c.is_empty());
                if cursor.is_none() { break; }
                tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
            } else {
                break;
            }
        }
        all.reverse();
        all
    }

    /// Resolve all unique user IDs in records to display names.
    async fn resolve_authors(&self, records: &mut [Record]) {
        let mut cache: HashMap<String, String> = HashMap::new();
        // Collect unique user IDs that look like Slack UIDs
        let uids: Vec<String> = records.iter()
            .map(|r| r.author.clone())
            .filter(|a| a.starts_with('U') && a.len() > 5)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for uid in &uids {
            let name = self.run_cmd(&["users.info", &format!("user={uid}")])
                .await
                .and_then(|j| serde_json::from_str::<UserResp>(&j).ok())
                .and_then(|r| if r.ok { r.user } else { None })
                .and_then(|u| u.profile.and_then(|p| p.display_name.filter(|n| !n.is_empty())).or(u.real_name))
                .unwrap_or_else(|| uid.clone());
            cache.insert(uid.clone(), name);
        }

        for r in records.iter_mut() {
            if let Some(name) = cache.get(&r.author) {
                r.author = name.clone();
            }
        }
    }

    async fn run_cmd(&self, args: &[&str]) -> Option<String> {
        for attempt in 0..3 {
            let output = tokio::process::Command::new(&self.slack_api)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .ok()?;
            if output.status.success() {
                let body = String::from_utf8_lossy(&output.stdout).to_string();
                if body.contains("\"error\":\"ratelimited\"") || body.contains("\"error\":\"rate_limited\"") {
                    let wait = (attempt + 1) * 5;
                    tracing::warn!(attempt, wait_secs = wait, "slack rate limited, retrying");
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                    continue;
                }
                return Some(body);
            }
            // Non-zero exit might indicate rate limit too
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("429") || stderr.contains("rate") {
                let wait = (attempt + 1) * 5;
                tracing::warn!(attempt, wait_secs = wait, "slack rate limited (stderr), retrying");
                tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                continue;
            }
            return None;
        }
        None
    }
}

fn msg_to_record(channel_id: &str, msg: &SlackMsg, context_name: &str, msg_type: &str) -> Record {
    let ts = msg.ts.as_deref().unwrap_or("0");
    let native_id = format!("{channel_id}-{ts}");
    let created = ts_to_datetime(ts);

    Record {
        id: Record::make_id(Source::Slack, &native_id),
        source: Source::Slack,
        kind: Kind::Message,
        title: format!("#{} - {}", context_name, msg.author),
        content: msg.text.clone(),
        author: msg.author.clone(),
        participants: vec![],
        created_at: created,
        updated_at: created,
        url: String::new(),
        thread_id: format!("slack-{msg_type}-{channel_id}"),
        entities: vec![],
        tags: vec![msg_type.to_string()],
    }
}

fn ts_to_datetime(ts: &str) -> DateTime<Utc> {
    let secs: f64 = ts.parse().unwrap_or(0.0);
    #[allow(clippy::cast_possible_truncation)]
    let secs_i64 = secs as i64;
    Utc.timestamp_opt(secs_i64, 0).single().unwrap_or_else(Utc::now)
}

fn oldest_ts(days: i64) -> String {
    let ts = (Utc::now() - chrono::Duration::days(days)).timestamp();
    format!("{ts}.000000")
}

// --- Slack API response types ---

#[derive(Deserialize)]
struct ConvListResp { ok: bool, channels: Option<Vec<Conv>> }
#[derive(Deserialize)]
struct Conv { id: String, user: Option<String>, name: Option<String> }
#[derive(Deserialize)]
struct HistResp { ok: bool, messages: Option<Vec<RawMsg>>, has_more: Option<bool>, response_metadata: Option<RespMeta> }
#[derive(Deserialize)]
struct RespMeta { next_cursor: Option<String> }
#[derive(Deserialize)]
struct RawMsg { user: Option<String>, text: Option<String>, ts: Option<String> }
#[derive(Deserialize)]
struct UserResp { ok: bool, user: Option<UserInfo> }
#[derive(Deserialize)]
struct UserInfo { real_name: Option<String>, profile: Option<Profile> }
#[derive(Deserialize)]
struct Profile { display_name: Option<String> }

struct SlackMsg {
    ts: Option<String>,
    author: String,
    text: String,
}
