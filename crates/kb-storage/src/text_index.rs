use anyhow::Result;
use kb_core::KbConfig;
use std::fs;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Value, TantivyDocument, Schema, STRING, STORED, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};

pub struct TextIndex {
    index: Index,
    reader: IndexReader,
    // Field handles
    f_id: Field,
    f_source: Field,
    f_title: Field,
    f_content: Field,
    f_author: Field,
    f_created_at: Field,
    f_url: Option<Field>,
}

pub struct TextResult {
    pub id: String,
    pub source: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub url: String,
    pub score: f32,
}

impl TextIndex {
    /// Open (or create) the full-text index.
    ///
    /// # Errors
    /// Returns error if the index directory cannot be created or the index cannot be opened.
    ///
    /// # Panics
    /// Panics if the schema is missing an expected field.
    pub fn open(config: &KbConfig) -> Result<Self> {
        let dir = config.index_dir();
        fs::create_dir_all(&dir)?;

        let schema = Self::build_schema();
        let index = if dir.join("meta.json").exists() {
            Index::open_in_dir(&dir)?
        } else {
            Index::create_in_dir(&dir, schema)?
        };

        let reader = index.reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // Use the index's actual on-disk schema for field handles so opening an
        // older index (without `url`) does not mismatch field ids or panic.
        let schema = index.schema();
        let f_id = schema.get_field("id").unwrap();
        let f_source = schema.get_field("source").unwrap();
        let f_title = schema.get_field("title").unwrap();
        let f_content = schema.get_field("content").unwrap();
        let f_author = schema.get_field("author").unwrap();
        let f_created_at = schema.get_field("created_at").unwrap();
        let f_url = schema.get_field("url").ok();

        Ok(Self { index, reader, f_id, f_source, f_title, f_content, f_author, f_created_at, f_url })
    }

    /// Delete all docs and recreate a fresh index.
    ///
    /// # Errors
    /// Returns error if the existing index cannot be removed or a fresh one cannot be created.
    pub fn clear(config: &KbConfig) -> Result<Self> {
        let dir = config.index_dir();
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Self::open(config)
    }

    /// Add or replace records in the index.
    ///
    /// # Errors
    /// Returns error if the index writer fails to add documents or commit.
    pub fn upsert(&self, records: &[kb_core::Record]) -> Result<()> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        for r in records {
            // Delete existing doc with same ID
            let term = tantivy::Term::from_field_text(self.f_id, &r.id);
            writer.delete_term(term);

            let mut document = doc!(
                self.f_id => r.id.as_str(),
                self.f_source => r.source.to_string(),
                self.f_title => r.title.as_str(),
                self.f_content => r.content.as_str(),
                self.f_author => r.author.as_str(),
                self.f_created_at => r.created_at.format("%Y-%m-%d %H:%M").to_string(),
            );
            if let Some(f_url) = self.f_url {
                document.add_text(f_url, r.url.as_str());
            }
            writer.add_document(document)?;
        }

        writer.commit()?;
        Ok(())
    }

    /// Full-text search returning top-k results.
    ///
    /// # Errors
    /// Returns error if the query cannot be parsed or the search fails.
    ///
    /// # Panics
    /// Panics only if the hardcoded fallback query `"_"` fails to parse, which cannot happen.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<TextResult>> {
        let query_str = query_str.trim();
        if query_str.is_empty() {
            return Ok(vec![]);
        }
        self.reader.reload()?;
        let searcher = self.reader.searcher();

        let mut query_parser = QueryParser::for_index(&self.index, vec![self.f_title, self.f_content, self.f_author]);
        query_parser.set_conjunction_by_default();

        // Sanitize: strip characters that trigger Tantivy grammar panics
        let safe_query: String = query_str.chars()
            .filter(|c| !matches!(c, ':' | '(' | ')' | '{' | '}' | '[' | ']' | '!' | '^' | '~'))
            .collect();
        let safe_query = safe_query.trim();
        if safe_query.is_empty() {
            return Ok(vec![]);
        }

        // If query has multiple words and isn't already quoted, search as phrase AND as individual terms
        let query = if safe_query.contains(' ') && !safe_query.contains('"') {
            let phrase_q = format!("\"{safe_query}\"^3 {safe_query}");
            query_parser.parse_query(&phrase_q)
                .or_else(|_| query_parser.parse_query(safe_query))
                .unwrap_or_else(|_| query_parser.parse_query("_").unwrap())
        } else {
            query_parser.parse_query(safe_query)
                .unwrap_or_else(|_| query_parser.parse_query("_").unwrap())
        };

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            let get = |f: Field| doc.get_first(f).and_then(|v| v.as_str()).unwrap_or("").to_string();

            results.push(TextResult {
                id: get(self.f_id),
                source: get(self.f_source),
                title: get(self.f_title),
                content: get(self.f_content),
                created_at: get(self.f_created_at),
                url: self.f_url.map(get).unwrap_or_default(),
                score,
            });
        }
        Ok(results)
    }

    fn build_schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("id", STRING | STORED);
        builder.add_text_field("source", STRING | STORED);
        builder.add_text_field("title", TEXT | STORED);
        builder.add_text_field("content", TEXT | STORED);
        builder.add_text_field("author", TEXT | STORED);
        builder.add_text_field("created_at", STRING | STORED);
        builder.add_text_field("url", STRING | STORED);
        builder.build()
    }
}
