use anyhow::{Context, Result};
use arrow_array::{ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use kb_core::KbConfig;
use lance_arrow::FixedSizeListArrayExt;
use lancedb::connect;
use lancedb::query::{ExecutableQuery, QueryBase};
use std::sync::Arc;

use crate::embedder;

const TABLE_NAME: &str = "records";

pub struct VectorStore {
    db_path: String,
}

/// A search result from the vector store.
pub struct VectorResult {
    pub id: String,
    pub content: String,
    pub source: String,
    pub title: String,
    pub created_at: String,
    pub url: String,
    pub score: f32,
}

impl VectorStore {
    #[must_use]
    pub fn new(config: &KbConfig) -> Self {
        Self {
            db_path: config.vectors_dir().to_string_lossy().to_string(),
        }
    }

    /// Upsert records with their embeddings into `LanceDB`.
    ///
    /// # Errors
    /// Returns error if the embedding dimensions overflow `i32`, or any `LanceDB` operation fails.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(&self, ids: &[String], contents: &[String], sources: &[String], titles: &[String], created_ats: &[String], urls: &[String], embeddings: Vec<Vec<f32>>) -> Result<()> {
        if ids.is_empty() { return Ok(()); }

        let dims = i32::try_from(embedder::dims()).context("embedding dims exceed i32")?;
        let db = connect(&self.db_path).execute().await?;

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("source", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("url", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dims),
                false,
            ),
        ]));

        let flat: Vec<f32> = embeddings.into_iter().flatten().collect();
        let values = Float32Array::from(flat);
        let vector_array = FixedSizeListArray::try_new_from_values(values, dims)?;

        let batch = RecordBatch::try_new(schema.clone(), vec![
            Arc::new(StringArray::from(ids.to_vec())) as ArrayRef,
            Arc::new(StringArray::from(contents.to_vec())) as ArrayRef,
            Arc::new(StringArray::from(sources.to_vec())) as ArrayRef,
            Arc::new(StringArray::from(titles.to_vec())) as ArrayRef,
            Arc::new(StringArray::from(created_ats.to_vec())) as ArrayRef,
            Arc::new(StringArray::from(urls.to_vec())) as ArrayRef,
            Arc::new(vector_array) as ArrayRef,
        ])?;

        // Create or overwrite table
        let tables = db.table_names().execute().await?;
        if tables.contains(&TABLE_NAME.to_string()) {
            let table = db.open_table(TABLE_NAME).execute().await?;
            // If schema changed (missing created_at or url), drop and recreate
            let existing_schema = table.schema().await?;
            if existing_schema.field_with_name("created_at").is_err()
                || existing_schema.field_with_name("url").is_err() {
                db.drop_table(TABLE_NAME, &[]).await?;
                db.create_table(TABLE_NAME, vec![batch]).execute().await?;
            } else {
                // Delete existing records by ID for true upsert
                let id_filter = ids.iter()
                    .map(|id| format!("'{}'", id.replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(", ");
                if !id_filter.is_empty() {
                    table.delete(&format!("id IN ({id_filter})")).await.ok();
                }
                table.add(vec![batch]).execute().await?;
            }
        } else {
            db.create_table(TABLE_NAME, vec![batch]).execute().await?;
        }

        Ok(())
    }

    /// Vector search returning top-k results.
    ///
    /// # Errors
    /// Returns error if the query cannot be embedded or the `LanceDB` search fails.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<VectorResult>> {
        let qv = embedder::embed(query).await.context("embed query")?;
        let db = connect(&self.db_path).execute().await?;
        let table = db.open_table(TABLE_NAME).execute().await?;

        let results = table.vector_search(qv)?.limit(limit).execute().await?;
        let batches: Vec<RecordBatch> = results.try_collect().await?;

        let mut out = Vec::new();
        for batch in &batches {
            let ids = batch.column_by_name("id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let contents = batch.column_by_name("content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let sources = batch.column_by_name("source").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let titles = batch.column_by_name("title").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_ats = batch.column_by_name("created_at").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let urls = batch.column_by_name("url").and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let (Some(ids), Some(contents), Some(sources), Some(titles)) = (ids, contents, sources, titles) else { continue };

            for i in 0..batch.num_rows() {
                out.push(VectorResult {
                    id: ids.value(i).to_string(),
                    content: contents.value(i).to_string(),
                    source: sources.value(i).to_string(),
                    title: titles.value(i).to_string(),
                    created_at: created_ats.map(|c| c.value(i).to_string()).unwrap_or_default(),
                    url: urls.map(|c| c.value(i).to_string()).unwrap_or_default(),
                    score: 0.0,
                });
            }
        }
        Ok(out)
    }
}
