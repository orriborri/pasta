use anyhow::Result;
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use kb_core::{KbConfig, Record};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

/// Builds a `work_items.parquet` cross-reference table from processed records.
/// Each row represents a work item (Linear issue, MR) with all related record IDs.
///
/// # Errors
/// Returns error if the Parquet file cannot be written.
///
/// # Panics
/// Panics if the data directory path has no parent (should not happen with valid config).
pub fn build_work_items(config: &KbConfig, records: &[Record]) -> Result<()> {
    // Group records by their primary entity
    let mut items: HashMap<String, WorkItem> = HashMap::new();

    for r in records {
        for entity in &r.entities {
            let item = items.entry(entity.clone()).or_insert_with(|| WorkItem {
                entity: entity.clone(),
                record_ids: Vec::new(),
                sources: Vec::new(),
                first_seen: r.created_at.to_rfc3339(),
                last_seen: r.created_at.to_rfc3339(),
            });
            if !item.record_ids.contains(&r.id) {
                item.record_ids.push(r.id.clone());
            }
            let src = r.source.to_string();
            if !item.sources.contains(&src) {
                item.sources.push(src);
            }
            if r.created_at.to_rfc3339() > item.last_seen {
                item.last_seen = r.created_at.to_rfc3339();
            }
            if r.created_at.to_rfc3339() < item.first_seen {
                item.first_seen = r.created_at.to_rfc3339();
            }
        }
    }

    if items.is_empty() {
        return Ok(());
    }

    let path = config.data_dir.join("work_items.parquet");
    fs::create_dir_all(path.parent().unwrap())?;

    let values: Vec<&WorkItem> = items.values().collect();
    let schema = Arc::new(Schema::new(vec![
        Field::new("entity", DataType::Utf8, false),
        Field::new("record_ids", DataType::Utf8, false),
        Field::new("sources", DataType::Utf8, false),
        Field::new("first_seen", DataType::Utf8, false),
        Field::new("last_seen", DataType::Utf8, false),
    ]));

    let batch = RecordBatch::try_new(schema.clone(), vec![
        Arc::new(StringArray::from(values.iter().map(|w| w.entity.as_str()).collect::<Vec<_>>())) as ArrayRef,
        Arc::new(StringArray::from(values.iter().map(|w| w.record_ids.join(",")).collect::<Vec<_>>())) as ArrayRef,
        Arc::new(StringArray::from(values.iter().map(|w| w.sources.join(",")).collect::<Vec<_>>())) as ArrayRef,
        Arc::new(StringArray::from(values.iter().map(|w| w.first_seen.as_str()).collect::<Vec<_>>())) as ArrayRef,
        Arc::new(StringArray::from(values.iter().map(|w| w.last_seen.as_str()).collect::<Vec<_>>())) as ArrayRef,
    ])?;

    let file = fs::File::create(&path)?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    tracing::info!(items = items.len(), path = %path.display(), "work_items.parquet written");
    Ok(())
}

struct WorkItem {
    entity: String,
    record_ids: Vec<String>,
    sources: Vec<String>,
    first_seen: String,
    last_seen: String,
}
