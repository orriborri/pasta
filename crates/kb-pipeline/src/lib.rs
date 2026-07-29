pub mod normalize;
pub mod filter;
pub mod dedupe;
pub mod extract;
pub mod summarize;
pub mod cross_dedupe;
pub mod chunk;
pub mod registry;
pub mod enrich;
pub mod entity_manager;
pub mod discoveries;

use anyhow::Result;
use kb_core::Record;
use tracing::info;

/// Each pipeline stage processes a batch of records and returns the (possibly filtered/modified) batch.
pub trait PipelineStage: Send + Sync {
    fn name(&self) -> &str;
    /// Process a batch of records, returning the transformed batch.
    ///
    /// # Errors
    /// Returns error if the stage encounters an unrecoverable processing failure.
    fn process(&self, records: Vec<Record>) -> Result<Vec<Record>>;
}

/// Composes stages and runs them in order.
/// If a stage fails, it logs the error and passes records through unchanged.
pub struct Pipeline {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl Pipeline {
    #[must_use]
    pub fn new(stages: Vec<Box<dyn PipelineStage>>) -> Self {
        Self { stages }
    }

    pub fn run(&self, records: Vec<Record>) -> Vec<Record> {
        let mut current = records;
        for stage in &self.stages {
            let before = current.len();
            let backup = current.clone();
            current = match stage.process(current) {
                Ok(result) => {
                    let after = result.len();
                    if before != after {
                        info!(stage = stage.name(), before, after, "pipeline stage");
                    }
                    result
                }
                Err(e) => {
                    tracing::warn!(stage = stage.name(), error = %e, "stage failed, passing records through unchanged");
                    backup
                }
            };
        }
        current
    }
}
