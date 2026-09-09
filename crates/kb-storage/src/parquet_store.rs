use anyhow::Result;
use arrow_array::{ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use kb_core::{KbConfig, Kind, Record, Source};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::properties::WriterProperties;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::{DateTime, Utc};

pub struct ParquetStore {
    raw_dir: PathBuf,
}

impl ParquetStore {
    #[must_use]
    pub fn new(config: &KbConfig) -> Self {
        Self { raw_dir: config.raw_dir() }
    }

    /// Write records to Parquet, partitioned by source/month.
    /// Uses a unique filename per sync batch to avoid overwriting previous data.
    ///
    /// # Errors
    /// Returns error if directories cannot be created or Parquet files cannot be written.
    ///
    /// # Panics
    /// Panics if a partition path has no parent directory.
    pub fn write(&self, records: &[Record]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let batch_ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();

        // Group by source + month
        let mut groups: std::collections::HashMap<String, Vec<&Record>> = std::collections::HashMap::new();
        for r in records {
            let key = format!("{}/{}", r.source, r.created_at.format("%Y-%m"));
            groups.entry(key).or_default().push(r);
        }

        for (partition, recs) in &groups {
            let path = self.raw_dir.join(format!("{partition}/{batch_ts}.parquet"));
            fs::create_dir_all(path.parent().unwrap())?;
            Self::write_batch(&path, recs)?;
        }
        Ok(())
    }

    fn write_batch(path: &Path, records: &[&Record]) -> Result<()> {
        let schema = Self::schema();
        let batch = Self::records_to_batch(records, &schema)?;

        let file = fs::File::create(path)?;
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, Arc::new(schema), Some(props))?;
        writer.write(&batch)?;
        writer.close()?;
        Ok(())
    }

    fn records_to_batch(records: &[&Record], schema: &Schema) -> Result<RecordBatch> {
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let sources: Vec<&str> = records.iter().map(|r| source_str(r)).collect();
        let kinds: Vec<&str> = records.iter().map(|r| kind_str(r)).collect();
        let titles: Vec<&str> = records.iter().map(|r| r.title.as_str()).collect();
        let contents: Vec<&str> = records.iter().map(|r| r.content.as_str()).collect();
        let authors: Vec<&str> = records.iter().map(|r| r.author.as_str()).collect();
        let participants: Vec<String> = records.iter().map(|r| r.participants.join(",")).collect();
        let participants_ref: Vec<&str> = participants.iter().map(std::string::String::as_str).collect();
        let created: Vec<String> = records.iter().map(|r| r.created_at.to_rfc3339()).collect();
        let created_ref: Vec<&str> = created.iter().map(std::string::String::as_str).collect();
        let updated: Vec<String> = records.iter().map(|r| r.updated_at.to_rfc3339()).collect();
        let updated_ref: Vec<&str> = updated.iter().map(std::string::String::as_str).collect();
        let urls: Vec<&str> = records.iter().map(|r| r.url.as_str()).collect();
        let thread_ids: Vec<&str> = records.iter().map(|r| r.thread_id.as_str()).collect();
        let entities: Vec<String> = records.iter().map(|r| r.entities.join(",")).collect();
        let entities_ref: Vec<&str> = entities.iter().map(std::string::String::as_str).collect();
        let tags: Vec<String> = records.iter().map(|r| r.tags.join(",")).collect();
        let tags_ref: Vec<&str> = tags.iter().map(std::string::String::as_str).collect();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(sources)),
            Arc::new(StringArray::from(kinds)),
            Arc::new(StringArray::from(titles)),
            Arc::new(StringArray::from(contents)),
            Arc::new(StringArray::from(authors)),
            Arc::new(StringArray::from(participants_ref)),
            Arc::new(StringArray::from(created_ref)),
            Arc::new(StringArray::from(updated_ref)),
            Arc::new(StringArray::from(urls)),
            Arc::new(StringArray::from(thread_ids)),
            Arc::new(StringArray::from(entities_ref)),
            Arc::new(StringArray::from(tags_ref)),
        ];

        Ok(RecordBatch::try_new(Arc::new(schema.clone()), columns)?)
    }

    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("source", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("author", DataType::Utf8, false),
            Field::new("participants", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
            Field::new("url", DataType::Utf8, false),
            Field::new("thread_id", DataType::Utf8, false),
            Field::new("entities", DataType::Utf8, false),
            Field::new("tags", DataType::Utf8, false),
        ])
    }

    /// Read all records from all Parquet files (for reindex).
    ///
    /// # Errors
    /// Returns error if a Parquet file cannot be opened or parsed.
    pub fn read_all(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        for entry in walkdir(&self.raw_dir) {
            let path = entry?;
            if path.extension().is_some_and(|e| e == "parquet") {
                let file = fs::File::open(&path)?;
                let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
                let reader = builder.build()?;
                for batch in reader {
                    let batch = batch?;
                    records.extend(batch_to_records(&batch));
                }
            }
        }
        Ok(records)
    }

    /// Read all records whose id is in the given set. Scans Parquet and filters.
    /// Parquet is the source of truth for record bodies; this reuses the same
    /// read machinery as `read_all` with an id filter.
    ///
    /// # Errors
    /// Returns error if a Parquet file cannot be opened or parsed.
    pub fn get_by_ids(&self, ids: &[&str]) -> Result<Vec<Record>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let want: std::collections::HashSet<&str> = ids.iter().copied().collect();
        Ok(self.read_all()?.into_iter().filter(|r| want.contains(r.id.as_str())).collect())
    }
}

const fn source_str(r: &Record) -> &'static str {
    match r.source {
        kb_core::Source::Slack => "slack",
        kb_core::Source::Gmail => "gmail",
        kb_core::Source::Linear => "linear",
        kb_core::Source::Git => "git",
        kb_core::Source::Gdocs => "gdocs",
        kb_core::Source::Calendar => "calendar",
        kb_core::Source::Vault => "vault",
    }
}

const fn kind_str(r: &Record) -> &'static str {
    match r.kind {
        kb_core::Kind::Message => "message",
        kb_core::Kind::Thread => "thread",
        kb_core::Kind::Issue => "issue",
        kb_core::Kind::Commit => "commit",
        kb_core::Kind::Doc => "doc",
        kb_core::Kind::Event => "event",
    }
}

fn walkdir(dir: &Path) -> Vec<Result<PathBuf>> {
    fn walk(dir: &Path, files: &mut Vec<Result<PathBuf>>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else {
                    files.push(Ok(path));
                }
            }
        }
    }
    let mut files = Vec::new();
    if !dir.exists() { return files; }
    walk(dir, &mut files);
    files
}

fn batch_to_records(batch: &RecordBatch) -> Vec<Record> {
    let get_str = |name: &str| -> Option<&StringArray> {
        batch.column_by_name(name).and_then(|c| c.as_any().downcast_ref::<StringArray>())
    };

    let Some(ids) = get_str("id") else { return vec![] };
    let sources = get_str("source");
    let kinds = get_str("kind");
    let titles = get_str("title");
    let contents = get_str("content");
    let authors = get_str("author");
    let participants = get_str("participants");
    let created_ats = get_str("created_at");
    let updated_ats = get_str("updated_at");
    let urls = get_str("url");
    let thread_ids = get_str("thread_id");
    let entities = get_str("entities");
    let tags = get_str("tags");

    (0..batch.num_rows()).map(|i| {
        let s = |arr: Option<&StringArray>| arr.map(|a| a.value(i).to_string()).unwrap_or_default();
        Record {
            id: ids.value(i).to_string(),
            source: parse_source(sources.map_or("slack", |a| a.value(i))),
            kind: parse_kind(kinds.map_or("message", |a| a.value(i))),
            title: s(titles),
            content: s(contents),
            author: s(authors),
            participants: s(participants).split(',').filter(|p| !p.is_empty()).map(std::string::ToString::to_string).collect(),
            created_at: parse_dt(&s(created_ats)),
            updated_at: parse_dt(&s(updated_ats)),
            url: s(urls),
            thread_id: s(thread_ids),
            entities: s(entities).split(',').filter(|e| !e.is_empty()).map(std::string::ToString::to_string).collect(),
            tags: s(tags).split(',').filter(|t| !t.is_empty()).map(std::string::ToString::to_string).collect(),
        }
    }).collect()
}

fn parse_source(s: &str) -> Source {
    match s {
        "slack" => Source::Slack,
        "gmail" => Source::Gmail,
        "linear" => Source::Linear,
        "git" => Source::Git,
        "gdocs" => Source::Gdocs,
        "calendar" => Source::Calendar,
        _ => Source::Vault,
    }
}

fn parse_kind(s: &str) -> Kind {
    match s {
        "thread" => Kind::Thread,
        "issue" => Kind::Issue,
        "commit" => Kind::Commit,
        "doc" => Kind::Doc,
        "event" => Kind::Event,
        _ => Kind::Message,
    }
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc))
}
