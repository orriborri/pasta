//! Reviewed `JDex` label deltas. Approval is an operator action, never inferred from evidence.
use crate::{Registry, Store};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, future::Future, path::PathBuf};

mod gog;
pub use gog::Gog;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LabelRequest {
    pub request_key: String,
    pub account: String,
    pub message_id: String,
    pub evidence_record_id: String,
    pub jd_id: String,
    pub rationale: String,
}
#[derive(Clone, Debug)]
pub struct LabelContext {
    pub registry: PathBuf,
    pub evidence_dir: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub content_version: String,
    pub label_ids: Vec<String>,
    pub labels: Vec<Label>,
}

/// Provider contract: reads return full message version and the label catalog;
/// writes modify only the supplied native label ID delta on one message.
pub trait Provider: Sync {
    fn snapshot(
        &self,
        account: &str,
        message: &str,
    ) -> impl Future<Output = Result<Snapshot>> + Send;
    fn modify(
        &self,
        account: &str,
        message: &str,
        add: &[String],
        remove: &[String],
    ) -> impl Future<Output = Result<()>> + Send;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Preview,
    Approved,
    Executing,
    Uncertain,
    Applied,
    Rejected,
}
impl State {
    const fn tag(&self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Uncertain => "uncertain",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub request: LabelRequest,
    pub registry_hash: String,
    pub source_version: String,
    pub before_label_ids: Vec<String>,
    pub target: Label,
    pub add_label_ids: Vec<String>,
    pub remove: Vec<Label>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub observed_at: String,
    pub verified: bool,
    pub command_acknowledged: bool,
    pub observed: Option<Snapshot>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub digest: String,
    pub state: State,
    pub plan: Plan,
    pub receipt: Option<Receipt>,
}

impl Store {
    fn label_schema(&self) -> Result<()> {
        self.0.execute_batch(
            "CREATE TABLE IF NOT EXISTS gmail_label_actions (
          id TEXT PRIMARY KEY, request_key TEXT UNIQUE NOT NULL, input TEXT NOT NULL,
          digest TEXT NOT NULL, plan TEXT NOT NULL, state TEXT NOT NULL, receipt TEXT);
          CREATE TABLE IF NOT EXISTS gmail_label_events (
          id INTEGER PRIMARY KEY, action_id TEXT NOT NULL REFERENCES gmail_label_actions(id),
          event TEXT NOT NULL, occurred_at TEXT NOT NULL, detail TEXT NOT NULL);",
        )?;
        Ok(())
    }

    /// Preview a proposed primary `JDex` classification; makes no provider writes.
    /// # Errors
    /// Rejects unknown/disabled routes, missing source evidence/labels, mismatched
    /// source versions, conflicting retry keys, or provider/storage failures.
    pub async fn preview_gmail_label(
        &mut self,
        context: &LabelContext,
        request: &LabelRequest,
        provider: &impl Provider,
    ) -> Result<Action> {
        self.label_schema()?;
        let request = normalize_request(request)?;
        let input = serde_json::to_string(&request)?;
        let id = crate::digest(&["gmail-label", &request.request_key]);
        if let Some(existing) = self
            .0
            .query_row(
                "SELECT input FROM gmail_label_actions WHERE id=?1",
                [&id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            if existing != input {
                bail!("request key reused with different classification");
            }
            return read_action(&self.0, &id);
        }
        let bytes = std::fs::read(&context.registry)?;
        let registry = Registry::parse(&bytes)?;
        allow(&registry, &request.jd_id)?;
        let source_version = evidence_version(context, &request)?;
        let snapshot = provider
            .snapshot(&request.account, &request.message_id)
            .await?
            .normalize()?;
        if snapshot.content_version != source_version {
            bail!("retained evidence does not match current Gmail message; refresh evidence");
        }
        let plan = plan_labels(request, &registry, hash(&bytes), snapshot)?;
        let digest = hash(&serde_json::to_vec(&plan)?);
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO gmail_label_actions VALUES (?1,?2,?3,?4,?5,'preview',NULL)",
            params![
                id,
                plan.request.request_key,
                input,
                digest,
                serde_json::to_string(&plan)?
            ],
        )?;
        let saved: String = tx.query_row(
            "SELECT input FROM gmail_label_actions WHERE id=?1",
            [&id],
            |row| row.get(0),
        )?;
        if saved != input {
            bail!("request key reused with different classification");
        }
        if inserted == 1 {
            event(&tx, &id, "preview", &digest)?;
        }
        let result = read_action(&tx, &id)?;
        tx.commit()?;
        Ok(result)
    }

    /// Operator-only approval of the exact reviewed plan digest. Not an MCP tool.
    /// # Errors
    /// Rejects wrong digests, unknown actions, or states other than preview/approved.
    pub fn approve_gmail_label(&mut self, id: &str, digest: &str) -> Result<Action> {
        self.label_schema()?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let action = read_action(&tx, id)?;
        if action.digest != digest {
            bail!("approval digest does not match reviewed plan");
        }
        if ![State::Preview, State::Approved].contains(&action.state) {
            bail!("action cannot be approved in this state");
        }
        if action.state == State::Preview {
            tx.execute(
                "UPDATE gmail_label_actions SET state='approved' WHERE id=?1",
                [id],
            )?;
            event(&tx, id, "approved", digest)?;
        }
        let result = read_action(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }

    /// Read a durable plan, execution state, and any read-back receipt.
    /// # Errors
    /// Returns errors for missing actions or corrupt plans.
    pub fn gmail_label_action(&self, id: &str) -> Result<Action> {
        self.label_schema()?;
        read_action(&self.0, id)
    }

    /// List recent durable classification proposals for operator review.
    /// # Errors
    /// Returns database/corrupt-plan errors.
    pub fn gmail_label_actions(&self, limit: usize) -> Result<Vec<Action>> {
        self.label_schema()?;
        let mut query = self
            .0
            .prepare("SELECT id FROM gmail_label_actions ORDER BY rowid DESC LIMIT ?1")?;
        let ids = query
            .query_map([limit.min(100)], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter().map(|id| read_action(&self.0, id)).collect()
    }

    /// Reject or revoke a plan before execution. Cannot cancel an unknown outcome.
    /// # Errors
    /// Rejects executing/uncertain/applied actions; these require reconciliation.
    pub fn reject_gmail_label(&mut self, id: &str) -> Result<Action> {
        self.label_schema()?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let action = read_action(&tx, id)?;
        if ![State::Preview, State::Approved, State::Rejected].contains(&action.state) {
            bail!("execution already started; reconcile instead of rejecting");
        }
        if action.state != State::Rejected {
            tx.execute(
                "UPDATE gmail_label_actions SET state='rejected' WHERE id=?1",
                [id],
            )?;
            event(&tx, id, "rejected", &action.digest)?;
        }
        let result = read_action(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }

    /// Apply one already-approved delta. An uncertain outcome is never retried.
    /// # Errors
    /// Rejects missing approval or stale registry/evidence/provider preconditions.
    /// Provider write failures are recorded as uncertain unless read-back verifies
    /// the approved result; callers must inspect the returned state and receipt.
    pub async fn apply_gmail_label(
        &mut self,
        context: &LabelContext,
        id: &str,
        provider: &impl Provider,
    ) -> Result<Action> {
        let action = self.gmail_label_action(id)?;
        if action.state == State::Applied {
            return Ok(action);
        }
        if action.state != State::Approved {
            bail!("action needs operator approval, or read-only reconciliation if execution is uncertain");
        }
        check_context(context, &action.plan)?;
        let snapshot = provider
            .snapshot(
                &action.plan.request.account,
                &action.plan.request.message_id,
            )
            .await?
            .normalize()?;
        if !action.plan.matches(&snapshot, false) {
            bail!("Gmail preconditions changed; create a fresh preview and approval");
        }
        check_context(context, &action.plan)?;
        {
            let tx = self
                .0
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            if tx.execute(
                "UPDATE gmail_label_actions SET state='executing' WHERE id=?1 AND state='approved'",
                [id],
            )? != 1
            {
                bail!("another caller already claimed this action");
            }
            event(&tx, id, "executing", &action.digest)?;
            tx.commit()?;
        }
        let remove: Vec<_> = action
            .plan
            .remove
            .iter()
            .map(|label| label.id.clone())
            .collect();
        let result = if action.plan.add_label_ids.is_empty() && remove.is_empty() {
            Ok(())
        } else {
            provider
                .modify(
                    &action.plan.request.account,
                    &action.plan.request.message_id,
                    &action.plan.add_label_ids,
                    &remove,
                )
                .await
        };
        self.observe_action(
            &action,
            provider,
            result.is_ok(),
            result.err().map(|e| e.to_string()),
        )
        .await
    }

    /// Read-only provider reconciliation after a crash or ambiguous write result.
    /// Never resets an action to approved or sends a second mutation.
    /// # Errors
    /// Rejects actions that have not entered execution or cannot be read.
    pub async fn reconcile_gmail_label(
        &mut self,
        id: &str,
        provider: &impl Provider,
    ) -> Result<Action> {
        let action = self.gmail_label_action(id)?;
        if action.state == State::Applied {
            return Ok(action);
        }
        if ![State::Executing, State::Uncertain].contains(&action.state) {
            bail!("action has not entered execution");
        }
        self.observe_action(&action, provider, false, None).await
    }

    async fn observe_action(
        &mut self,
        action: &Action,
        provider: &impl Provider,
        acknowledged: bool,
        mut error: Option<String>,
    ) -> Result<Action> {
        let observed = match provider
            .snapshot(
                &action.plan.request.account,
                &action.plan.request.message_id,
            )
            .await
            .and_then(Snapshot::normalize)
        {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                error = Some(e.to_string());
                None
            }
        };
        let verified = observed
            .as_ref()
            .is_some_and(|snapshot| action.plan.matches(snapshot, true));
        let receipt = Receipt {
            observed_at: Utc::now().to_rfc3339(),
            verified,
            command_acknowledged: acknowledged,
            observed,
            error,
        };
        let state = if verified {
            State::Applied
        } else {
            State::Uncertain
        };
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE gmail_label_actions SET state=?1,receipt=?2 WHERE id=?3 AND state IN ('executing','uncertain')", params![state.tag(),serde_json::to_string(&receipt)?,action.id])? == 1 {
            event(&tx,&action.id,state.tag(),&serde_json::to_string(&receipt)?)?;
        }
        let result = read_action(&tx, &action.id)?;
        tx.commit()?;
        Ok(result)
    }
}

fn plan_labels(
    request: LabelRequest,
    registry: &Registry,
    registry_hash: String,
    snapshot: Snapshot,
) -> Result<Plan> {
    let target_name = format!("JDex/{}", request.jd_id);
    let target = snapshot
        .labels
        .iter()
        .find(|label| label.name == target_name)
        .context("target JDex label must already exist in Gmail")?
        .clone();
    if target.kind != "user" {
        bail!("JDex target must be a user label");
    }
    let mut remove = Vec::new();
    for id in &snapshot.label_ids {
        let label = snapshot
            .labels
            .iter()
            .find(|label| label.id == *id)
            .context("attached label absent from catalog; refresh preview")?;
        if label.id != target.id {
            if let Some(jd) = canonical_jd(&label.name) {
                allow(registry, jd)?;
                if label.kind != "user" {
                    bail!("cannot remove system labels");
                }
                remove.push(label.clone());
            }
        }
    }
    remove.sort_by(|a, b| a.id.cmp(&b.id));
    let add_label_ids = if snapshot.label_ids.contains(&target.id) {
        vec![]
    } else {
        vec![target.id.clone()]
    };
    Ok(Plan {
        request,
        registry_hash,
        source_version: snapshot.content_version,
        before_label_ids: snapshot.label_ids,
        target,
        add_label_ids,
        remove,
    })
}

impl Snapshot {
    fn normalize(mut self) -> Result<Self> {
        self.label_ids.sort();
        self.label_ids.dedup();
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for label in &self.labels {
            valid_id(&label.id)?;
            if !ids.insert(&label.id) || !names.insert(&label.name) {
                bail!("ambiguous Gmail label catalog");
            }
        }
        for id in &self.label_ids {
            valid_id(id)?;
        }
        self.labels.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(self)
    }
}
impl Plan {
    fn matches(&self, snapshot: &Snapshot, after: bool) -> bool {
        let mut expected = self.before_label_ids.clone();
        if after {
            expected.retain(|id| !self.remove.iter().any(|label| label.id == *id));
            expected.extend_from_slice(&self.add_label_ids);
            expected.sort();
            expected.dedup();
        }
        snapshot.content_version == self.source_version
            && snapshot.label_ids == expected
            && std::iter::once(&self.target)
                .chain(&self.remove)
                .all(|binding| snapshot.labels.contains(binding))
    }
}
fn check_context(context: &LabelContext, plan: &Plan) -> Result<()> {
    let bytes = std::fs::read(&context.registry)?;
    if hash(&bytes) != plan.registry_hash {
        bail!("JDex registry changed after preview");
    }
    let registry = Registry::parse(&bytes)?;
    allow(&registry, &plan.request.jd_id)?;
    for label in &plan.remove {
        allow(
            &registry,
            canonical_jd(&label.name).context("invalid planned JDex removal")?,
        )?;
    }
    if evidence_version(context, &plan.request)? != plan.source_version {
        bail!("retained source version changed");
    }
    Ok(())
}
fn allow(registry: &Registry, jd: &str) -> Result<()> {
    if !registry
        .entries
        .iter()
        .any(|entry| entry.jd_id == jd && entry.gmail_enabled())
    {
        bail!("JD ID is unknown or not explicitly enabled for Gmail labeling");
    }
    Ok(())
}
fn canonical_jd(name: &str) -> Option<&str> {
    let id = name.strip_prefix("JDex/")?;
    let b = id.as_bytes();
    (b.len() == 5
        && b[2] == b'.'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 2 || c.is_ascii_digit()))
    .then_some(id)
}
fn normalize_request(request: &LabelRequest) -> Result<LabelRequest> {
    let mut request = request.clone();
    request.account = request.account.trim().to_ascii_lowercase();
    if !request.account.contains('@')
        || request.account.starts_with('-')
        || request
            .account
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        || request.request_key.trim().is_empty()
        || request.rationale.trim().is_empty()
        || request.rationale.len() > 16384
    {
        bail!("explicit mailbox email, request key, and bounded rationale are required");
    }
    valid_id(&request.message_id)?;
    Ok(request)
}
fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.starts_with('-')
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        bail!("invalid Gmail native ID");
    }
    Ok(())
}
fn evidence_version(context: &LabelContext, request: &LabelRequest) -> Result<String> {
    let suffix = request
        .evidence_record_id
        .strip_prefix("gmail-message-")
        .context("expected retained Gmail message record ID")?;
    if suffix.len() != 64
        || !suffix.bytes().all(|b| b.is_ascii_hexdigit())
        || !context.evidence_dir.is_absolute()
    {
        bail!("invalid retained Gmail evidence reference or root");
    }
    let manifest: Value = serde_json::from_slice(&std::fs::read(
        context
            .evidence_dir
            .join("messages")
            .join(format!("{}.json", request.evidence_record_id)),
    )?)?;
    let body_json = manifest["message"]["content"]
        .as_str()
        .context("missing retained message content")?;
    if manifest["version"] != 1
        || manifest["message"]["id"] != request.evidence_record_id
        || manifest["message"]["source"] != "gmail"
        || manifest["message"]["kind"] != "message"
        || hash(body_json.as_bytes()) != suffix
    {
        bail!("retained message identity/checksum mismatch");
    }
    let source: Value = serde_json::from_str(body_json)?;
    if source["schema"] != "pasta.gmail.evidence.v1"
        || source["account"] != request.account
        || source["message_id"] != request.message_id
    {
        bail!("evidence belongs to a different account or message");
    }
    message_version(&source["full_message"], &request.message_id)
}
fn message_version(full: &Value, expected: &str) -> Result<String> {
    let mut message = full["message"]
        .as_object()
        .context("missing full Gmail message")?
        .clone();
    if message.get("id").and_then(Value::as_str) != Some(expected)
        || !message.get("payload").is_some_and(Value::is_object)
    {
        bail!("wrong/incomplete Gmail message");
    }
    message.remove("labelIds");
    message.remove("historyId");
    Ok(hash(&serde_json::to_vec(&message)?))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn event(db: &rusqlite::Connection, id: &str, state: &str, detail: &str) -> Result<()> {
    db.execute(
        "INSERT INTO gmail_label_events(action_id,event,occurred_at,detail) VALUES (?1,?2,?3,?4)",
        params![id, state, Utc::now().to_rfc3339(), detail],
    )?;
    Ok(())
}
fn read_action(db: &rusqlite::Connection, id: &str) -> Result<Action> {
    let (digest, plan, state, receipt): (String, String, String, Option<String>) = db
        .query_row(
            "SELECT digest,plan,state,receipt FROM gmail_label_actions WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .context("unknown Gmail label action")?;
    let plan: Plan = serde_json::from_str(&plan)?;
    if hash(&serde_json::to_vec(&plan)?) != digest {
        bail!("stored Gmail label plan digest mismatch");
    }
    Ok(Action {
        id: id.into(),
        digest,
        plan,
        state: serde_json::from_value(json!(state))?,
        receipt: receipt.map(|r| serde_json::from_str(&r)).transpose()?,
    })
}

#[cfg(test)]
mod tests;
