use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const OPENAI_EMBED_URL: &str = "https://api.openai.com/v1/embeddings";
const OPENAI_MODEL: &str = "text-embedding-3-small";
const OPENAI_DIMS: usize = 1536;

const OLLAMA_URL: &str = "http://localhost:11434/api/embed";
const OLLAMA_MODEL: &str = "nomic-embed-text";
const OLLAMA_DIMS: usize = 768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedModel {
    OpenAI,
    Ollama,
}

struct LockedModel {
    model: EmbedModel,
    key: Option<String>,
}

static MODEL: OnceLock<LockedModel> = OnceLock::new();

/// Initialize and lock the embedding model. Must be called once at startup.
/// Returns the selected model.
///
/// # Errors
/// Returns error if the model cannot be initialized (network failure on probe).
pub async fn init() -> Result<EmbedModel> {
    if let Some(m) = MODEL.get() {
        return Ok(m.model);
    }

    let key = api_key();
    let model = if let Some(ref k) = key {
        // Verify key works
        let client = reqwest::Client::new();
        let resp = client
            .post(OPENAI_EMBED_URL)
            .header("Authorization", format!("Bearer {k}"))
            .json(&OpenAIReq { model: OPENAI_MODEL, input: &["test".to_string()] })
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => EmbedModel::OpenAI,
            _ => EmbedModel::Ollama,
        }
    } else {
        EmbedModel::Ollama
    };

    MODEL.set(LockedModel { model, key }).ok();
    tracing::info!(model = ?model, "embedding model locked");
    Ok(model)
}

/// Get the dimensionality of the locked model.
pub fn dims() -> usize {
    match MODEL.get().map_or(EmbedModel::Ollama, |m| m.model) {
        EmbedModel::OpenAI => OPENAI_DIMS,
        EmbedModel::Ollama => OLLAMA_DIMS,
    }
}

/// The locked model name (for sync state).
pub fn model_name() -> &'static str {
    match MODEL.get().map_or(EmbedModel::Ollama, |m| m.model) {
        EmbedModel::OpenAI => OPENAI_MODEL,
        EmbedModel::Ollama => OLLAMA_MODEL,
    }
}

/// Embed a single text using the locked model.
///
/// # Errors
/// Returns error if the embedding API call fails or returns empty.
pub async fn embed(text: &str) -> Result<Vec<f32>> {
    let batch = embed_batch(&[text.to_string()]).await?;
    batch.into_iter().next().ok_or_else(|| anyhow!("empty embedding response"))
}

/// Embed a batch of texts using the locked model.
///
/// # Errors
/// Returns error if the embedding API call fails.
///
/// # Panics
/// Panics if called with `OpenAI` model but no API key is stored (should not happen after `init()`).
pub async fn embed_batch(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let locked = MODEL.get().ok_or_else(|| anyhow!("embedder not initialized — call init() first"))?;
    match locked.model {
        EmbedModel::OpenAI => embed_openai(texts, locked.key.as_deref().unwrap()).await,
        EmbedModel::Ollama => embed_ollama(texts).await,
    }
}

#[derive(Serialize)]
struct OpenAIReq<'a> { model: &'a str, input: &'a [String] }
#[derive(Deserialize)]
struct OpenAIResp { data: Vec<OpenAIData> }
#[derive(Deserialize)]
struct OpenAIData { embedding: Vec<f32> }

async fn embed_openai(texts: &[String], key: &str) -> Result<Vec<Vec<f32>>> {
    let resp: OpenAIResp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?
        .post(OPENAI_EMBED_URL)
        .header("Authorization", format!("Bearer {key}"))
        .json(&OpenAIReq { model: OPENAI_MODEL, input: texts })
        .send().await?
        .json().await?;
    Ok(resp.data.into_iter().map(|d| d.embedding).collect())
}

#[derive(Serialize)]
struct OllamaReq<'a> { model: &'a str, input: &'a [String] }
#[derive(Deserialize)]
struct OllamaResp { embeddings: Vec<Vec<f32>> }

async fn embed_ollama(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let resp: OllamaResp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_mins(1))
        .build()?
        .post(OLLAMA_URL)
        .json(&OllamaReq { model: OLLAMA_MODEL, input: texts })
        .send().await?
        .json().await?;
    Ok(resp.embeddings)
}

fn api_key() -> Option<String> {
    std::env::var("OPENAI_API_KEY").ok().or_else(|| {
        let path = dirs::home_dir()?.join(".config/openai/api_key");
        std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    })
}
