use anyhow::Result;
use kb_core::Record;
use std::sync::Arc;

use crate::registry::EntityRegistry;
use crate::PipelineStage;

/// Resolves author IDs (Slack UIDs, emails) to canonical names using the shared `EntityRegistry`.
pub struct NormalizeStage {
    registry: Arc<EntityRegistry>,
}

impl NormalizeStage {
    #[must_use]
    pub const fn new(registry: Arc<EntityRegistry>) -> Self {
        Self { registry }
    }
}

impl PipelineStage for NormalizeStage {
    fn name(&self) -> &'static str { "normalize" }

    fn process(&self, mut records: Vec<Record>) -> Result<Vec<Record>> {
        for r in &mut records {
            if let Some(canonical) = self.registry.resolve_person(&r.author) {
                r.author = canonical.to_string();
            }
        }
        Ok(records)
    }
}
