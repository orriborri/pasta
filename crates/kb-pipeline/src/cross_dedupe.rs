use anyhow::Result;
use kb_core::Record;
use std::collections::HashMap;

use crate::PipelineStage;

/// Merges records referencing the same work item across sources.
///
/// Runs AFTER `ExtractStage` has populated `entities`.
/// Example: a Slack notification and Gmail notification both tagged `mr:!2935`
/// get merged into one record with combined content.
pub struct CrossSourceDedupeStage;

impl PipelineStage for CrossSourceDedupeStage {
    fn name(&self) -> &'static str { "cross-source-dedupe" }

    /// Merge records sharing the same extracted entity (Linear issue or MR).
    ///
    /// # Errors
    /// This stage does not fail.
    fn process(&self, records: Vec<Record>) -> Result<Vec<Record>> {
        // Group records by their primary entity (first entity that looks like a work item)
        let mut entity_map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in records.iter().enumerate() {
            if let Some(entity) = primary_entity(r) {
                entity_map.entry(entity).or_default().push(i);
            }
        }

        let mut merged_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut result: Vec<Record> = Vec::new();

        for indices in entity_map.values() {
            if indices.len() <= 1 { continue; }
            // Merge: keep the first record, append content from others
            let primary_idx = indices[0];
            merged_indices.insert(primary_idx);
            let mut merged = records[primary_idx].clone();

            for &idx in &indices[1..] {
                merged_indices.insert(idx);
                let other = &records[idx];
                if !merged.tags.contains(&other.source.to_string()) {
                    merged.tags.push(format!("also:{}", other.source));
                }
                if other.content != merged.content {
                    use std::fmt::Write;
                    write!(merged.content, "\n\n[via {}] {}", other.source, other.content).ok();
                }
            }
            result.push(merged);
        }

        // Add all records that weren't part of a merge
        for (i, r) in records.into_iter().enumerate() {
            if !merged_indices.contains(&i) {
                result.push(r);
            }
        }

        Ok(result)
    }
}

/// Extract the primary work-item entity from a record (Linear issue or MR).
fn primary_entity(r: &Record) -> Option<String> {
    r.entities.iter()
        .find(|e| e.starts_with("linear:") || e.starts_with("mr:"))
        .cloned()
}
