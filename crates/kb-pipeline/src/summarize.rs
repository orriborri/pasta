use anyhow::Result;
use kb_core::Record;

use crate::PipelineStage;

/// Threshold in characters above which content gets summarized.
const SUMMARIZE_THRESHOLD: usize = 2000;

/// Summarizes long content using a heuristic (first + last paragraphs).
/// LLM summarization can be added later as an optional upgrade.
pub struct SummarizeStage {
    threshold: usize,
}

impl Default for SummarizeStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SummarizeStage {
    #[must_use]
    pub const fn new() -> Self {
        Self { threshold: SUMMARIZE_THRESHOLD }
    }
}

impl PipelineStage for SummarizeStage {
    fn name(&self) -> &'static str { "summarize" }

    /// Summarize records with content exceeding the threshold.
    ///
    /// # Errors
    /// This stage does not fail — always returns Ok.
    fn process(&self, mut records: Vec<Record>) -> Result<Vec<Record>> {
        for r in &mut records {
            if r.content.len() <= self.threshold {
                continue;
            }
            r.content = heuristic_summarize(&r.content);
        }
        Ok(records)
    }
}

/// Heuristic: keep first paragraph + last paragraph + a size note.
fn heuristic_summarize(content: &str) -> String {
    let paragraphs: Vec<&str> = content
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    if paragraphs.len() <= 2 {
        // Already short enough by paragraph count
        return content.to_string();
    }

    let first = paragraphs[0];
    let last = paragraphs[paragraphs.len() - 1];
    let skipped = paragraphs.len() - 2;

    format!("{first}\n\n[...{skipped} paragraphs omitted...]\n\n{last}")
}
