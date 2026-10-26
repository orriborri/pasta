//! Read-only Gmail ingestion. Each canonical full-message snapshot is retained
//! independently; snippets are never substituted for original message bodies.
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf, time::Duration};
use tracing::info;

pub struct GmailFetcher {
    lookback_days: i64,
    account: Option<String>,
    executable: PathBuf,
}

impl Default for GmailFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl GmailFetcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lookback_days: 30,
            account: None,
            executable: "gog".into(),
        }
    }

    /// Select the actual mailbox email, rather than an implicit default or alias.
    #[must_use]
    pub fn with_account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    fn account(&self) -> Result<String> {
        let configured = &pasta_common::config::get().gog.account;
        let account = self
            .account
            .clone()
            .or_else(|| std::env::var("GOG_ACCOUNT").ok().filter(|a| !a.is_empty()))
            .unwrap_or_else(|| configured.clone())
            .trim()
            .to_ascii_lowercase();
        if !account.contains('@')
            || account.starts_with('-')
            || account.chars().any(char::is_whitespace)
            || account.chars().any(char::is_control)
        {
            bail!("Gmail ingestion requires an explicit mailbox email in [gog] account or GOG_ACCOUNT; aliases and auto are unsupported");
        }
        Ok(account)
    }

    /// Fetch all matching messages, retaining full source payloads and versions.
    /// The account-scoped cursor is staged only after every page and get succeeds.
    /// The caller must persist returned records before committing staged cursors.
    ///
    /// Coverage is bounded: initial 30 days plus a one-day replay overlap;
    /// promotions/social are excluded. Attachment bytes are not downloaded.
    /// # Errors
    /// Returns an error for missing account, failed commands, malformed responses,
    /// repeated pagination tokens, invalid source dates, or mismatched identities.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let account = self.account()?;
        let key = cursor_key(&account);
        let upper = Utc::now();
        let since = state.cursor(&key).map_or_else(
            || Ok(upper - chrono::Duration::days(self.lookback_days)),
            |s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|d| d.with_timezone(&Utc))
                    .context("invalid Gmail cursor")
            },
        )?;
        if since > upper {
            bail!("Gmail cursor is in the future");
        }
        let lower = since - chrono::Duration::days(1);
        let query = format!(
            "after:{} before:{} -category:promotions -category:social",
            lower.timestamp(),
            upper.timestamp() + 1
        );
        let records = self.fetch_query(&account, &query).await?;
        state.update_window(&key, &since.to_rfc3339(), &upper.to_rfc3339())?;
        info!(
            count = records.len(),
            "Gmail full-message snapshots fetched; attachment metadata only"
        );
        Ok(records)
    }

    async fn fetch_query(&self, account: &str, query: &str) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        let mut page: Option<String> = None;
        let mut seen_pages = HashSet::new();
        let mut seen_messages = HashSet::new();
        loop {
            let mut args: Vec<String> = [
                "gmail",
                "messages",
                "search",
                query,
                "--max",
                "100",
                "--timezone",
                "UTC",
            ]
            .map(str::to_owned)
            .into();
            if let Some(token) = &page {
                args.extend(["--page".into(), token.clone()]);
            }
            let response: SearchResult =
                serde_json::from_value(self.request(account, &args).await?)
                    .context("expected gog Gmail messages search envelope")?;
            for message in response.messages {
                validate_id(&message.id)?;
                if !seen_messages.insert(message.id.clone()) {
                    continue;
                }
                let args = vec![
                    "gmail".into(),
                    "get".into(),
                    message.id.clone(),
                    "--format".into(),
                    "full".into(),
                    "--use-indexed-attachment-ids=false".into(),
                ];
                let full = self.request(account, &args).await?;
                records.push(message_to_record(
                    account,
                    &message.id,
                    &message.labels,
                    full,
                )?);
            }
            match response.next_page_token.filter(|s| !s.is_empty()) {
                Some(token) => {
                    if !seen_pages.insert(token.clone()) {
                        bail!("Gmail search repeated a pagination token");
                    }
                    page = Some(token);
                }
                None => break,
            }
        }
        Ok(records)
    }

    async fn request(&self, account: &str, args: &[String]) -> Result<Value> {
        let mut command = tokio::process::Command::new(&self.executable);
        command
            .args([
                format!("--account={account}"),
                "--json".into(),
                "--no-input".into(),
                "--readonly".into(),
                "--wrap-untrusted=false".into(),
            ])
            .args(args)
            .kill_on_drop(true);
        let config = &pasta_common::config::get().gog;
        for (key, value) in [
            ("GOG_HOME", &config.home),
            ("GOG_KEYRING_BACKEND", &config.keyring_backend),
            ("GOG_KEYRING_PASSWORD", &config.keyring_password),
        ] {
            if !value.is_empty() {
                command.env(key, value);
            }
        }
        let output = tokio::time::timeout(Duration::from_secs(120), command.output())
            .await
            .context("gog Gmail command timed out")??;
        if !output.status.success() {
            bail!(
                "gog Gmail read failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        serde_json::from_slice(&output.stdout).context("gog Gmail returned invalid JSON")
    }
}

fn cursor_key(account: &str) -> String {
    format!("gmail_forward:v2:{:x}", Sha256::digest(account.as_bytes()))
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.starts_with('-')
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("invalid Gmail native ID");
    }
    Ok(())
}

fn message_to_record(
    account: &str,
    expected_id: &str,
    labels: &[String],
    mut full: Value,
) -> Result<Record> {
    let source: FullMessage = serde_json::from_value(full.clone())
        .context("expected unsanitized gog Gmail full-message envelope")?;
    let message = source.message;
    validate_id(&message.id)?;
    validate_id(&message.thread_id)?;
    if message.id != expected_id {
        bail!("Gmail get returned a different message ID");
    }
    if !message.payload.is_object() {
        bail!("Gmail full message is missing its MIME payload");
    }
    let millis: i64 = message
        .internal_date
        .parse()
        .context("invalid Gmail internalDate")?;
    let created =
        DateTime::from_timestamp_millis(millis).context("Gmail internalDate is out of range")?;
    let mut label_ids = message.label_ids;
    label_ids.sort();
    label_ids.dedup();
    if full["message"].get("labelIds").is_some() {
        full["message"]["labelIds"] = json!(label_ids);
    }
    let mut labels = labels.to_vec();
    labels.sort();
    labels.dedup();
    let mut tags = labels.clone();
    tags.extend(label_ids.iter().map(|id| format!("gmail-label-id:{id}")));
    tags.extend([
        "evidence:gmail-api-full".into(),
        "attachments:metadata-only".into(),
        if source.body.as_ref().is_some_and(|body| !body.is_empty()) {
            "body:decoded"
        } else {
            "body:not-reported"
        }
        .into(),
    ]);
    tags.sort();
    tags.dedup();
    let content = serde_json::to_string(
        &json!({"schema":"pasta.gmail.evidence.v1", "account":account, "message_id":message.id, "listed_labels":labels, "full_message":full}),
    )?;
    let version = format!("{:x}", Sha256::digest(content.as_bytes()));
    let thread = format!(
        "gmail-thread-{:x}",
        Sha256::digest(serde_json::to_vec(&(account, &message.thread_id))?)
    );
    let people = project_correspondents(&source.headers);
    if people.partial {
        tags.push("participants:partial".into());
        tags.sort();
    }
    Ok(Record {
        id: Record::make_id(Source::Gmail, &format!("message-{version}")),
        source: Source::Gmail,
        kind: Kind::Message,
        title: source.headers.get("subject").cloned().unwrap_or_default(),
        content,
        author: people.author,
        participants: people.participants,
        created_at: created,
        updated_at: created,
        url: format!(
            "https://mail.google.com/mail/?authuser={}#all/{}",
            encode_component(account),
            message.id
        ),
        thread_id: thread,
        entities: vec![],
        tags,
    })
}

struct Correspondents {
    author: String,
    participants: Vec<String>,
    partial: bool,
}

fn project_correspondents(headers: &std::collections::HashMap<String, String>) -> Correspondents {
    let mut projection = Correspondents {
        author: String::new(),
        participants: vec![],
        partial: false,
    };
    for name in ["from", "to", "cc", "bcc"] {
        let Some(value) = headers.get(name).filter(|value| !value.is_empty()) else {
            continue;
        };
        let Ok(parsed) = mailparse::addrparse(value) else {
            projection.partial = true;
            continue;
        };
        let mut mailboxes = Vec::new();
        for address in parsed.iter() {
            match address {
                mailparse::MailAddr::Single(mailbox) => mailboxes.push(mailbox.addr.clone()),
                mailparse::MailAddr::Group(group) => {
                    mailboxes.extend(group.addrs.iter().map(|mailbox| mailbox.addr.clone()));
                }
            }
        }
        if name == "from" && mailboxes.len() == 1 {
            projection.author.clone_from(&mailboxes[0]);
        }
        projection.participants.extend(mailboxes);
    }
    projection.participants.sort();
    projection.participants.dedup();
    projection
}

fn encode_component(value: &str) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            result.push(char::from(byte));
        } else {
            let _ = write!(result, "%{byte:02X}");
        }
    }
    result
}

#[derive(Deserialize)]
struct SearchResult {
    messages: Vec<SearchMessage>,
    #[serde(default, rename = "nextPageToken", alias = "next_page_token")]
    next_page_token: Option<String>,
}
#[derive(Deserialize)]
struct SearchMessage {
    id: String,
    #[serde(default)]
    labels: Vec<String>,
}
#[derive(Deserialize)]
struct FullMessage {
    message: ApiMessage,
    headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMessage {
    id: String,
    thread_id: String,
    internal_date: String,
    payload: Value,
    #[serde(default)]
    label_ids: Vec<String>,
}

#[cfg(test)]
mod tests;
