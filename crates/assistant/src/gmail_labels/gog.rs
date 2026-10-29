//! Narrow gog adapter. Reads are explicitly read-only; the only mutation is one
//! native-message label delta. Approval/replay handling lives in the action store.
use super::{message_version, valid_id, Label, Provider, Snapshot};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{path::PathBuf, time::Duration};

pub struct Gog {
    executable: PathBuf,
}
impl Default for Gog {
    fn default() -> Self {
        Self {
            executable: std::env::var_os("PASTA_GOG_BIN")
                .map_or_else(|| PathBuf::from("gog"), PathBuf::from),
        }
    }
}
impl Gog {
    /// Select an operator-installed/pinned gog binary, never a model argument.
    #[must_use]
    pub const fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
    async fn request(&self, account: &str, args: &[String], read_only: bool) -> Result<Value> {
        let mut command = tokio::process::Command::new(&self.executable);
        command.args([
            format!("--account={account}"),
            "--json".into(),
            "--no-input".into(),
            "--wrap-untrusted=false".into(),
        ]);
        if read_only {
            command.arg("--readonly");
        }
        command.args(args).kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(120), command.output())
            .await
            .context("gog label operation timed out")??;
        if !output.status.success() {
            bail!("gog label operation failed ({})", output.status);
        }
        serde_json::from_slice(&output.stdout).context("invalid gog label JSON response")
    }
}
impl Provider for Gog {
    async fn snapshot(&self, account: &str, message: &str) -> Result<Snapshot> {
        valid_id(message)?;
        let full = self
            .request(
                account,
                &[
                    "gmail".into(),
                    "get".into(),
                    message.into(),
                    "--format".into(),
                    "full".into(),
                    "--use-indexed-attachment-ids=false".into(),
                ],
                true,
            )
            .await?;
        let content_version = message_version(&full, message)?;
        let label_ids: Vec<String> = serde_json::from_value(
            full["message"]
                .get("labelIds")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )?;
        let catalog = self
            .request(
                account,
                &["gmail".into(), "labels".into(), "list".into()],
                true,
            )
            .await?;
        let labels: Vec<Label> = serde_json::from_value(
            catalog
                .get("labels")
                .cloned()
                .context("missing Gmail label catalog")?,
        )?;
        Ok(Snapshot {
            content_version,
            label_ids,
            labels,
        })
    }
    async fn modify(
        &self,
        account: &str,
        message: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<()> {
        valid_id(message)?;
        for id in add.iter().chain(remove) {
            valid_id(id)?;
            // gog resolves names case-insensitively. Canonical user IDs retain
            // an uppercase L, so its exact-ID map wins over lowercase names.
            if !id.starts_with("Label_") {
                bail!("unsupported Gmail user-label ID format; no mutation sent");
            }
        }
        let mut args = vec![
            "gmail".into(),
            "messages".into(),
            "modify".into(),
            message.into(),
        ];
        if !add.is_empty() {
            args.extend(["--add".into(), add.join(",")]);
        }
        if !remove.is_empty() {
            args.extend(["--remove".into(), remove.join(",")]);
        }
        if add.is_empty() && remove.is_empty() {
            return Ok(());
        }
        let response = self.request(account, &args, false).await?;
        if response["modified"] != message {
            bail!("gog returned an unexpected modified-message identity");
        }
        Ok(())
    }
}
