use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct KbConfig {
    pub data_dir: PathBuf,
}

impl Default for KbConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::home_dir().unwrap().join(".kb"),
        }
    }
}

impl KbConfig {
    #[must_use]
    pub fn raw_dir(&self) -> PathBuf { self.data_dir.join("raw") }
    #[must_use]
    pub fn vectors_dir(&self) -> PathBuf { self.data_dir.join("vectors") }
    #[must_use]
    pub fn index_dir(&self) -> PathBuf { self.data_dir.join("index") }
    /// Path to the evidence graph derived index (`graph.db`), sibling to the
    /// vector and text indexes. Safe to delete and rebuild from Parquet.
    #[must_use]
    pub fn graph_path(&self) -> PathBuf { self.data_dir.join("graph.db") }
}
