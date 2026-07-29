use anyhow::Result;
use kb_core::Record;
use std::collections::HashSet;

use crate::PipelineStage;

/// Deduplicates records by content hash within a batch.
pub struct DedupeStage;

impl PipelineStage for DedupeStage {
    fn name(&self) -> &'static str { "dedupe" }

    fn process(&self, records: Vec<Record>) -> Result<Vec<Record>> {
        let mut seen = HashSet::new();
        Ok(records.into_iter().filter(|r| {
            seen.insert(r.content_hash())
        }).collect())
    }
}
