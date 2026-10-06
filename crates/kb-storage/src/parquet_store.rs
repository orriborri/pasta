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
use std::sync::atomic::{AtomicU64, Ordering};
use chrono::{DateTime, NaiveDateTime, Utc};

static BATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

        let batch_ts = Utc::now().format("%Y%m%d%H%M%S%9f").to_string();
        let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);

        // Group by source + month
        let mut groups: std::collections::HashMap<String, Vec<&Record>> = std::collections::HashMap::new();
        for r in records {
            let key = format!("{}/{}", r.source, r.created_at.format("%Y-%m"));
            groups.entry(key).or_default().push(r);
        }

        for (partition, recs) in &groups {
            let path = self.raw_dir.join(format!("{partition}/{batch_ts}-{}-{sequence}.parquet", std::process::id()));
            fs::create_dir_all(path.parent().unwrap())?;
            Self::write_batch(&path, recs)?;
        }
        Ok(())
    }

    fn write_batch(path: &Path, records: &[&Record]) -> Result<()> {
        let schema = Self::schema();
        let batch = Self::records_to_batch(records, &schema)?;

        let temporary = path.with_extension("parquet.tmp");
        let file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, Arc::new(schema), Some(props))?;
        writer.write(&batch)?;
        writer.close()?;
        fs::File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, path)?;
        fs::File::open(path.parent().expect("partition has a parent"))?.sync_all()?;
        Ok(())
    }

    fn records_to_batch(records: &[&Record], schema: &Schema) -> Result<RecordBatch> {
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let sources: Vec<&str> = records.iter().map(|r| source_str(r)).collect();
        let kinds: Vec<&str> = records.iter().map(|r| kind_str(r)).collect();
        let titles: Vec<&str> = records.iter().map(|r| r.title.as_str()).collect();
        let contents: Vec<&str> = records.iter().map(|r| r.content.as_str()).collect();
        let authors: Vec<&str> = records.iter().map(|r| r.author.as_str()).collect();
        let participants: Vec<String> = records.iter().map(|r| serde_json::to_string(&r.participants)).collect::<std::result::Result<_, _>>()?;
        let participants_ref: Vec<&str> = participants.iter().map(std::string::String::as_str).collect();
        let created: Vec<String> = records.iter().map(|r| r.created_at.to_rfc3339()).collect();
        let created_ref: Vec<&str> = created.iter().map(std::string::String::as_str).collect();
        let updated: Vec<String> = records.iter().map(|r| r.updated_at.to_rfc3339()).collect();
        let updated_ref: Vec<&str> = updated.iter().map(std::string::String::as_str).collect();
        let urls: Vec<&str> = records.iter().map(|r| r.url.as_str()).collect();
        let thread_ids: Vec<&str> = records.iter().map(|r| r.thread_id.as_str()).collect();
        let entities: Vec<String> = records.iter().map(|r| serde_json::to_string(&r.entities)).collect::<std::result::Result<_, _>>()?;
        let entities_ref: Vec<&str> = entities.iter().map(std::string::String::as_str).collect();
        let tags: Vec<String> = records.iter().map(|r| serde_json::to_string(&r.tags)).collect::<std::result::Result<_, _>>()?;
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
        Ok(self.read_observed()?.into_iter().map(|(record, _)| record).collect())
    }

    /// Read immutable snapshots in ingestion order, including observation time.
    ///
    /// # Errors
    /// Returns an error if any committed Parquet file cannot be read.
    pub fn read_observed(&self) -> Result<Vec<(Record, DateTime<Utc>)>> {
        let mut records = Vec::new();
        let mut files = walkdir(&self.raw_dir)?;
        files.sort_by(|a, b| observed_at(a).cmp(&observed_at(b)).then_with(|| a.cmp(b)));
        for path in files {
            if path.extension().is_some_and(|e| e == "parquet") {
                let file = fs::File::open(&path)?;
                let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
                let reader = builder.build()?;
                for batch in reader {
                    let batch = batch?;
                    records.extend(batch_to_records(&batch).into_iter().map(|r| (r, observed_at(&path))));
                }
            }
        }
        Ok(records)
    }

    /// Current state is the last durably observed snapshot of each record ID.
    /// `read_all` remains the explicit history API.
    ///
    /// # Errors
    /// Returns an error if committed evidence cannot be read.
    pub fn read_latest(&self) -> Result<Vec<Record>> {
        let mut latest = std::collections::BTreeMap::new();
        for record in self.read_all()? {
            latest.insert(record.id.clone(), record);
        }
        Ok(latest.into_values().collect())
    }

    /// Current records, retiring legacy vault basename IDs when the same file
    /// has been observed under its collision-free relative-path ID. Historical
    /// IDs remain resolvable through `get_by_ids` and `read_all`.
    ///
    /// # Errors
    /// Returns an error if evidence cannot be read.
    pub fn read_current(&self) -> Result<Vec<Record>> {
        let mut current = std::collections::BTreeMap::new();
        for record in self.read_all()? {
            current.insert(current_key(&record), record);
        }
        Ok(current.into_values().collect())
    }

    /// Persist only new snapshots, retaining unchanged observations and history.
    ///
    /// # Errors
    /// Returns an error if reading or durable writing fails.
    pub fn write_changed(&self, records: &[Record]) -> Result<usize> {
        let mut latest: std::collections::HashMap<_, _> = self.read_latest()?.into_iter()
            .map(|r| (r.id.clone(), r.content_hash())).collect();
        let changed: Vec<_> = records.iter().filter(|record| {
            let hash = record.content_hash();
            if latest.get(&record.id) == Some(&hash) { return false; }
            latest.insert(record.id.clone(), hash);
            true
        }).cloned().collect();
        self.write(&changed)?;
        Ok(changed.len())
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
        Ok(self.read_latest()?.into_iter().filter(|r| want.contains(r.id.as_str())).collect())
    }
}

/// Stable current-state identity, including aliases from legacy vault IDs.
#[must_use]
pub fn current_key(record: &Record) -> String {
    if record.source == Source::Vault && !record.url.is_empty() {
        format!("vault-file:{}", record.url)
    } else {
        record.id.clone()
    }
}

const fn source_str(r: &Record) -> &'static str {
    match r.source {
        kb_core::Source::Slack => "slack",
        kb_core::Source::Gmail => "gmail",
        kb_core::Source::Linear => "linear",
        kb_core::Source::GitLab => "gitlab",
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

fn walkdir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() { return Ok(files); }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(walkdir(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "parquet") {
            files.push(path);
        }
    }
    Ok(files)
}

fn observed_at(path: &Path) -> DateTime<Utc> {
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    let timestamp = name.split('-').next().unwrap_or_default();
    let Some(seconds) = timestamp.get(..14) else { return DateTime::<Utc>::UNIX_EPOCH };
    let Ok(time) = NaiveDateTime::parse_from_str(seconds, "%Y%m%d%H%M%S") else {
        return DateTime::<Utc>::UNIX_EPOCH;
    };
    let nanos = timestamp.get(14..23).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    DateTime::from_timestamp(time.and_utc().timestamp(), nanos).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

fn parse_list(value: &str) -> Vec<String> {
    // Read legacy comma-separated fields as well as lossless JSON arrays.
    serde_json::from_str(value).unwrap_or_else(|_| value.split(',')
        .filter(|item| !item.is_empty()).map(str::to_string).collect())
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
            participants: parse_list(&s(participants)),
            created_at: parse_dt(&s(created_ats)),
            updated_at: parse_dt(&s(updated_ats)),
            url: s(urls),
            thread_id: s(thread_ids),
            entities: parse_list(&s(entities)),
            tags: parse_list(&s(tags)),
        }
    }).collect()
}

fn parse_source(s: &str) -> Source {
    match s {
        "slack" => Source::Slack,
        "gmail" => Source::Gmail,
        "linear" => Source::Linear,
        "gitlab" => Source::GitLab,
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitlab_source_string_round_trips() {
        let now = Utc::now();
        let record = Record {
            id: "gitlab-mr-1-2".to_string(),
            source: Source::GitLab,
            kind: Kind::Issue,
            title: String::new(),
            content: String::new(),
            author: String::new(),
            participants: vec![],
            created_at: now,
            updated_at: now,
            url: String::new(),
            thread_id: String::new(),
            entities: vec![],
            tags: vec![],
        };

        assert_eq!(source_str(&record), "gitlab");
        assert_eq!(parse_source(source_str(&record)), Source::GitLab);
    }
}
