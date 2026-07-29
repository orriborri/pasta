use anyhow::Result;
use kb_core::Record;
use std::sync::Arc;

use crate::registry::EntityRegistry;
use crate::PipelineStage;

/// Links records to projects using extracted entities and channel/thread context.
pub struct EnrichStage {
    registry: Arc<EntityRegistry>,
}

impl EnrichStage {
    #[must_use]
    pub const fn new(registry: Arc<EntityRegistry>) -> Self {
        Self { registry }
    }
}

impl PipelineStage for EnrichStage {
    fn name(&self) -> &'static str { "enrich" }

    fn process(&self, mut records: Vec<Record>) -> Result<Vec<Record>> {
        for r in &mut records {
            // Try to resolve project from extracted entities
            for entity in r.entities.clone() {
                if let Some(issue_id) = entity.strip_prefix("linear:") {
                    // Extract prefix from "AD-365" → "AD"
                    if let Some(prefix) = issue_id.split('-').next() {
                        if let Some(project) = self.registry.project_for_linear(prefix) {
                            add_tag(r, &format!("project:{project}"));
                        }
                    }
                }
            }

            // Try to resolve project from thread_id (channel-based)
            if r.thread_id.starts_with("slack-channel-") {
                let channel = r.title.split(" - ").next()
                    .unwrap_or("")
                    .trim_start_matches('#');
                if let Some(project) = self.registry.project_for_channel(channel) {
                    add_tag(r, &format!("project:{project}"));
                }
            }

            // Resolve author via registry (supplement NormalizeStage)
            if let Some(canonical) = self.registry.resolve_person(&r.author) {
                r.author = canonical.to_string();
            }
        }
        Ok(records)
    }
}

fn add_tag(r: &mut Record, tag: &str) {
    if !r.tags.iter().any(|t| t == tag) {
        r.tags.push(tag.to_string());
    }
}
