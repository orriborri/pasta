mod mcp;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use pasta_assistant::{
    gmail_labels::{Gog, LabelContext, LabelRequest},
    Registry, Store, TaskEdit,
};
use std::{io::Read, path::PathBuf};

#[derive(Parser)]
#[command(about = "Durable capture and attention tools for an external assistant; JSON output")]
struct Cli {
    #[arg(long, default_value = "/data/assistant.sqlite")]
    database: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Serve tools over local MCP stdio for an authenticated assistant runtime.
    Mcp {
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Opt in to Gmail label proposal/apply tools; approval stays CLI-only.
        #[arg(long)]
        gmail_label_actions: bool,
    },
    /// Capture UTF-8 evidence from stdin (does not create a task).
    Capture {
        #[arg(long)]
        source: String,
        #[arg(long)]
        source_id: String,
    },
    Evidence {
        id: String,
    },
    /// Add an explicitly requested commitment, linked to captured evidence.
    Task {
        #[arg(long)]
        request_key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        jd_id: Option<String>,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long, default_value_t = 1)]
        priority: u8,
    },
    /// Return open tasks in deterministic urgency order; no model call.
    Next {
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        at: Option<DateTime<Utc>>,
    },
    /// Read a full desired `TaskEdit` JSON object from stdin.
    Edit,
    Get {
        id: i64,
    },
    Backup {
        destination: PathBuf,
    },
    Done {
        id: i64,
    },
    Labels {
        #[command(subcommand)]
        command: LabelCommand,
    },
    RegistryCheck {
        path: PathBuf,
    },
}
#[derive(Subcommand)]
enum LabelCommand {
    /// Preview a `LabelRequest` JSON from stdin; no Gmail writes.
    Preview {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        evidence_dir: PathBuf,
    },
    /// Operator approval of the exact reviewed digest (not exposed through MCP).
    Approve {
        id: String,
        #[arg(long)]
        digest: String,
    },
    /// Execute an already-approved exact delta and verify the result.
    Apply {
        id: String,
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        evidence_dir: PathBuf,
    },
    /// Read-only reconciliation; never retries a provider mutation.
    Reconcile {
        id: String,
    },
    Show {
        id: String,
    },
    Reject {
        id: String,
    },
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}
async fn label_command(store: &mut Store, command: LabelCommand) -> Result<serde_json::Value> {
    if let LabelCommand::List { limit } = &command {
        return Ok(serde_json::to_value(store.gmail_label_actions(*limit)?)?);
    }
    let provider = Gog::default();
    let action = match command {
        LabelCommand::Preview {
            registry,
            evidence_dir,
        } => {
            let mut input = String::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_string(&mut input)?;
            if input.len() > 1024 * 1024 {
                bail!("classification request exceeds 1 MiB");
            }
            let request: LabelRequest = serde_json::from_str(&input)?;
            store
                .preview_gmail_label(
                    &LabelContext {
                        registry,
                        evidence_dir,
                    },
                    &request,
                    &provider,
                )
                .await?
        }
        LabelCommand::Approve { id, digest } => store.approve_gmail_label(&id, &digest)?,
        LabelCommand::Apply {
            id,
            registry,
            evidence_dir,
        } => {
            store
                .apply_gmail_label(
                    &LabelContext {
                        registry,
                        evidence_dir,
                    },
                    &id,
                    &provider,
                )
                .await?
        }
        LabelCommand::Reconcile { id } => store.reconcile_gmail_label(&id, &provider).await?,
        LabelCommand::Show { id } => store.gmail_label_action(&id)?,
        LabelCommand::Reject { id } => store.reject_gmail_label(&id)?,
        LabelCommand::List { .. } => unreachable!(),
    };
    Ok(serde_json::to_value(action)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::RegistryCheck { path } = &cli.command {
        let registry = Registry::load(path)?;
        println!(
            "{}",
            serde_json::json!({"valid":true,"entries":registry.entries.len()})
        );
        return Ok(());
    }
    if let Command::Mcp {
        registry,
        gmail_label_actions,
    } = &cli.command
    {
        return mcp::run(cli.database, registry.clone(), *gmail_label_actions).await;
    }
    let mut store = Store::open(&cli.database)?;
    let result = match cli.command {
        Command::Labels { command } => label_command(&mut store, command).await?,
        Command::Capture { source, source_id } => {
            let mut text = String::new();
            std::io::stdin()
                .take(10 * 1024 * 1024 + 1)
                .read_to_string(&mut text)?;
            if text.len() > 10 * 1024 * 1024 {
                bail!("capture exceeds 10 MiB");
            }
            let id = store.capture(&source, &source_id, &text)?;
            serde_json::json!({"evidence_id":id,"pasta_record_id":format!("vault-assistant-{id}"),"retained":true,"indexing":"not_verified"})
        }
        Command::Evidence { id } => store.evidence(&id)?,
        Command::Task {
            request_key,
            title,
            evidence,
            jd_id,
            registry,
            due,
            priority,
        } => {
            if let Some(id) = &jd_id {
                let path = registry
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("--jd-id requires --registry"))?;
                if !Registry::load(path)?.contains(id) {
                    bail!("JD ID is not in the registry: {id}");
                }
            }
            serde_json::to_value(store.add_task(
                &request_key,
                &title,
                &evidence,
                jd_id.as_deref(),
                due.as_deref(),
                priority,
            )?)?
        }
        Command::Next { limit, at } => store.recommendations(at.unwrap_or_else(Utc::now), limit)?,
        Command::Edit => {
            let mut text = String::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_string(&mut text)?;
            if text.len() > 1024 * 1024 {
                bail!("edit exceeds 1 MiB");
            }
            let edit: TaskEdit = serde_json::from_str(&text)?;
            serde_json::to_value(store.edit_task(&edit)?)?
        }
        Command::Get { id } => serde_json::to_value(store.task(id)?)?,
        Command::Backup { destination } => {
            store.backup(&destination)?;
            serde_json::json!({"backup":destination})
        }
        Command::Done { id } => {
            store.done(id)?;
            serde_json::json!({"id":id,"status":"done"})
        }
        Command::RegistryCheck { .. } | Command::Mcp { .. } => unreachable!(),
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
