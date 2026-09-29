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
                if let Some(r) = self.file_to_record(&path, "task") {
                    records.push(r);
                }
            }
        }

        // People
        let people_dir = layout.people();
        if people_dir.exists() {
            for path in walk_md(&people_dir) {
                if let Some(r) = self.file_to_record(&path, "person") {
                    records.push(r);
                }
            }
        }

        // Raw personal inbox (0. Inbox/Raw/) — append-only captures from
        // external agents. This is intentionally the only Inbox subtree indexed
        // here; generated daily notes and other operational files stay out.
        let raw_inbox_dir = layout.raw_inbox();
        if raw_inbox_dir.exists() {
            for path in walk_md(&raw_inbox_dir) {
                if let Some(r) = self.file_to_record(&path, "raw-inbox") {
                    records.push(r);
                }
            }
        }

        // Projects (1. Projects/)
        let projects_dir = layout.projects();
        if projects_dir.exists() {
            for path in walk_md(&projects_dir) {
                if let Some(r) = self.file_to_record(&path, "project") {
                    records.push(r);
                }
            }
        }

        // Areas (2. Areas/) and Resources (3. Resources/) — PARA docs used by
        // vault organization (link repair, PARA audit).
        for (dir, tag) in [(layout.areas(), "area"), (layout.resources(), "resource")] {
            if dir.exists() {
                for path in walk_md(&dir) {
                    if let Some(r) = self.file_to_record(&path, tag) {
                        records.push(r);
                    }
                }
            }
        }

        info!(total = records.len(), "vault fetch complete");
        Ok(records)
    }

    fn file_to_record(&self, path: &Path, tag: &str) -> Option<Record> {
        let content = fs::read_to_string(path).ok()?;
        if content.trim().is_empty()
            || pasta_common::vault::has_frontmatter_value(&content, "llm_wiki", "1")
        {
            return None;
        }

        let stem = path.file_stem()?.to_string_lossy().to_string();
        // Use the relative path, not the basename: separate folders can contain
        // identically named notes. Retain readable IDs without slug collisions.
        let relative = path.strip_prefix(&self.vault_path).ok()?.to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        // Citation IDs must not contain whitespace or Markdown delimiters.
        let encoded: String = relative.bytes().map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        }).collect();
        let id = Record::make_id(Source::Vault, &encoded);

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


#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "pasta-vault-fetcher-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("0. Inbox/Raw")).unwrap();
        dir
    }

    #[test]
    fn generated_pages_are_excluded_inside_para() {
        let dir = temp_vault();
        for folder in ["1. Projects", "2. Areas", "3. Resources"] {
            let root = dir.join(folder);
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("generated.md"), "---\nllm_wiki: 1\n---\nDerived prose").unwrap();
            fs::write(root.join("manual.md"), "---\nstatus: active\n---\nOriginal note\nllm_wiki: 1").unwrap();
        }
        let records = VaultFetcher::new(&dir).fetch().unwrap();
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|r| r.title == "manual"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raw_inbox_is_ingested_but_other_inbox_files_are_not() {
        let dir = temp_vault();
        std::fs::write(
            dir.join("0. Inbox/Raw/capture.md"),
            "---\nkind: preference\n---\n\nUser prefers concise updates.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("0. Inbox/operational.md"),
            "This operational inbox file must not be indexed by VaultFetcher.",
        )
        .unwrap();

        let records = VaultFetcher::new(&dir).fetch().unwrap();
        let raw: Vec<_> = records
            .iter()
            .filter(|record| record.tags.iter().any(|tag| tag == "raw-inbox"))
            .collect();

        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].title, "capture");
        assert!(!raw[0].id.contains(' '));
        assert!(raw[0].id.contains("0.%20Inbox/Raw/"));
        assert!(raw[0].content.contains("prefers concise updates"));
        assert!(!records.iter().any(|record| record.title == "operational"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn same_basename_in_different_directories_has_distinct_identity() {
        let dir = temp_vault();
        std::fs::create_dir_all(dir.join("Tasks/A")).unwrap();
        std::fs::create_dir_all(dir.join("Tasks/B")).unwrap();
        std::fs::write(dir.join("Tasks/A/Test.md"), "First task").unwrap();
        std::fs::write(dir.join("Tasks/B/Test.md"), "Second task").unwrap();
        let records = VaultFetcher::new(&dir).fetch().unwrap();
        assert_eq!(records.len(), 2);
        assert_ne!(records[0].id, records[1].id);
        let _ = std::fs::remove_dir_all(dir);
    }
}
