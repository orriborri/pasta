use anyhow::Result;
use arrow_array::{RecordBatch, StringArray};
use futures::TryStreamExt;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::{Deserialize, Serialize};
use std::path::Path;

const DIMS: usize = 1536;
const OPENAI_EMBED_URL: &str = "https://api.openai.com/v1/embeddings";
const OLLAMA_URL: &str = "http://localhost:11434/api/embed";

fn usage() {
    eprintln!("Usage: pasta-search <query> [--source <sources>] [--participant <name>] [--after <YYYY-MM-DD>] [--limit <n>]");
    std::process::exit(1);
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() { usage(); }

    let mut query = String::new();
    let mut source: Option<String> = None;
    let mut participant: Option<String> = None;
    let mut after: Option<String> = None;
    let mut limit: usize = 10;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => { i += 1; source = args.get(i).cloned(); }
            "--participant" => { i += 1; participant = args.get(i).cloned(); }
            "--after" => { i += 1; after = args.get(i).cloned(); }
            "--limit" => { i += 1; limit = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(10); }
            s if s.starts_with('-') => { eprintln!("Unknown flag: {}", s); usage(); }
            _ => {
                if !query.is_empty() { query.push(' '); }
                query.push_str(&args[i]);
            }
        }
        i += 1;
    }

    if query.is_empty() { usage(); }

    let db_path = Path::new(pasta_common::vault::vault_path()).join(".lancedb");
    let db = connect(db_path.to_str().unwrap()).execute().await?;
    let table = db.open_table("chunks").execute().await?;

    let qv = embed(&query).await?;
    let results = table.vector_search(qv)?.limit(limit).execute().await?;
    let batches: Vec<RecordBatch> = results.try_collect().await?;

    let mut printed = 0;
    for batch in &batches {
        let paths = batch.column_by_name("path").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let sources = batch.column_by_name("source").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let participants_col = batch.column_by_name("participants").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let dates = batch.column_by_name("date").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let contents = batch.column_by_name("content").and_then(|c| c.as_any().downcast_ref::<StringArray>());

        let (Some(p), Some(s), Some(c)) = (paths, sources, contents) else { continue };
        for i in 0..batch.num_rows() {
            let src = s.value(i);
            let path_val = p.value(i);
            let part = participants_col.map(|pa| pa.value(i)).unwrap_or("");
            let date = dates.map(|d| d.value(i)).unwrap_or("");

            if let Some(ref sf) = source {
                if !sf.split(',').any(|f| f == src) { continue; }
            }
            if let Some(ref pf) = participant {
                if !part.to_lowercase().contains(&pf.to_lowercase()) { continue; }
            }
            if let Some(ref af) = after {
                if !date.is_empty() && date < af.as_str() { continue; }
            }

            println!("---");
            println!("source: {} | path: {} | date: {} | participants: {}", src, path_val, date, part);
            // Print first 300 chars of content
            let snippet: String = c.value(i).chars().take(300).collect();
            println!("{}", snippet);
            printed += 1;
        }
    }

    if printed == 0 {
        println!("No results found.");
    }
    Ok(())
}

async fn embed(text: &str) -> Result<Vec<f32>> {
    match embed_openai(text).await {
        Ok(v) => Ok(v),
        Err(_) => embed_ollama(text).await,
    }
}

#[derive(Serialize)]
struct EmbedReq<'a> { model: &'a str, input: &'a [&'a str] }
#[derive(Deserialize)]
struct EmbedResp { data: Vec<EmbedD> }
#[derive(Deserialize)]
struct EmbedD { embedding: Vec<f32> }

async fn embed_openai(text: &str) -> Result<Vec<f32>> {
    let key = std::env::var("OPENAI_API_KEY").or_else(|_| {
        let p = dirs::home_dir().unwrap().join(".config/openai/api_key");
        std::fs::read_to_string(p).map(|s| s.trim().to_string())
    }).map_err(|_| anyhow::anyhow!("no key"))?;
    let resp: EmbedResp = reqwest::Client::new()
        .post(OPENAI_EMBED_URL)
        .header("Authorization", format!("Bearer {}", key))
        .json(&EmbedReq { model: "text-embedding-3-small", input: &[text] })
        .send().await?.json().await?;
    resp.data.into_iter().next().map(|d| d.embedding).ok_or_else(|| anyhow::anyhow!("empty"))
}

#[derive(Serialize)]
struct OllamaReq<'a> { model: &'a str, input: &'a [&'a str] }
#[derive(Deserialize)]
struct OllamaResp { embeddings: Vec<Vec<f32>> }

async fn embed_ollama(text: &str) -> Result<Vec<f32>> {
    let resp: OllamaResp = reqwest::Client::new()
        .post(OLLAMA_URL)
        .json(&OllamaReq { model: "nomic-embed-text", input: &[text] })
        .send().await?.json().await?;
    let mut v = resp.embeddings.into_iter().next().ok_or_else(|| anyhow::anyhow!("empty"))?;
    v.resize(DIMS, 0.0);
    Ok(v)
}
