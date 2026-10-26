use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use pasta_assistant::{Registry, Store};
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
    Done {
        id: i64,
    },
    RegistryCheck {
        path: PathBuf,
    },
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::RegistryCheck { path } = &cli.command {
        let registry = Registry::load(path)?;
        println!(
            "{}",
            serde_json::json!({"valid":true,"entries":registry.entries.len()})
        );
        return Ok(());
    }
    let mut store = Store::open(&cli.database)?;
    let result = match cli.command {
        Command::Capture { source, source_id } => {
            let mut text = String::new();
            std::io::stdin()
                .take(10 * 1024 * 1024 + 1)
                .read_to_string(&mut text)?;
            if text.len() > 10 * 1024 * 1024 {
                bail!("capture exceeds 10 MiB");
            }
            serde_json::json!({"evidence_id":store.capture(&source,&source_id,&text)?})
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
        Command::Next { limit, at } => {
            serde_json::json!({"tasks":store.next(at.unwrap_or_else(Utc::now),limit)?,"ranking":"overdue, due within 24h, priority, due date, ID"})
        }
        Command::Done { id } => {
            store.done(id)?;
            serde_json::json!({"id":id,"status":"done"})
        }
        Command::RegistryCheck { .. } => unreachable!(),
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
