use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{Kind, Record, Source};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

pub struct VaultFetcher {
    vault_path: PathBuf,
}

impl VaultFetcher {
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self { vault_path: vault_path.into() }
    }

    /// Read Tasks/ and key vault folders into Records.
    ///
    /// # Errors
    /// Returns error if directory reading fails.
    pub fn fetch(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        let layout = pasta_common::vault::VaultLayout::new(&self.vault_path);

        // Tasks
        let tasks_dir = layout.tasks();
        if tasks_dir.exists() {
            for path in walk_md(&tasks_dir) {
                if let Some(r) = Self::file_to_record(&path, "task") {
                    records.push(r);
                }
            }
        }

        // People
        let people_dir = layout.people();
        if people_dir.exists() {
            for path in walk_md(&people_dir) {
                if let Some(r) = Self::file_to_record(&path, "person") {
                    records.push(r);
                }
            }
        }

        // Projects (1. Projects/)
        let projects_dir = layout.projects();
        if projects_dir.exists() {
            for path in walk_md(&projects_dir) {
                if let Some(r) = Self::file_to_record(&path, "project") {
                    records.push(r);
                }
            }
        }

        // Areas (2. Areas/) and Resources (3. Resources/) — PARA docs used by
        // vault organization (link repair, PARA audit).
        for (dir, tag) in [(layout.areas(), "area"), (layout.resources(), "resource")] {
            if dir.exists() {
                for path in walk_md(&dir) {
                    if let Some(r) = Self::file_to_record(&path, tag) {
                        records.push(r);
                    }
                }
            }
        }

        info!(total = records.len(), "vault fetch complete");
        Ok(records)
    }

    fn file_to_record(path: &Path, tag: &str) -> Option<Record> {
        let content = fs::read_to_string(path).ok()?;
        if content.trim().is_empty() { return None; }

        let stem = path.file_stem()?.to_string_lossy().to_string();
        let id = Record::make_id(Source::Vault, &format!("{}-{}", tag, slug(&stem)));

        let modified = fs::metadata(path).ok()
            .and_then(|m| m.modified().ok()).map_or_else(Utc::now, DateTime::<Utc>::from);

        Some(Record {
            id,
            source: Source::Vault,
            kind: Kind::Doc,
            title: stem,
            content,
            author: String::new(),
            participants: vec![],
            created_at: modified,
            updated_at: modified,
            url: path.to_string_lossy().to_string(),
            thread_id: tag.to_string(),
            entities: vec![],
            tags: vec![tag.to_string()],
        })
    }
}

fn walk_md(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, files);
            } else if path.extension().is_some_and(|e| e == "md") {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, &mut files);
    files
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
