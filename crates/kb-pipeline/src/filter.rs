use anyhow::Result;
use kb_core::Record;

use crate::PipelineStage;

/// Filters out bot messages, CI notifications, and promotional content.
pub struct FilterStage {
    bot_patterns: Vec<String>,
}

impl Default for FilterStage {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterStage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bot_patterns: vec![
                "gitlab bot".to_string(),
                "slackbot".to_string(),
                "deploybot".to_string(),
            ],
        }
    }
}

impl PipelineStage for FilterStage {
    fn name(&self) -> &'static str { "filter" }

    fn process(&self, records: Vec<Record>) -> Result<Vec<Record>> {
        Ok(records.into_iter().filter(|r| {
            // Skip empty content
            if r.content.trim().is_empty() { return false; }

            // Skip known bot authors (exact match)
            let author_lower = r.author.to_lowercase();
            if self.bot_patterns.contains(&author_lower) {
                return false;
            }

            // Skip authors ending with "bot" (e.g., "DeployBot", "cibot")
            if author_lower.ends_with("bot") && author_lower != "robert" {
                return false;
            }

            // Skip CI pipeline notifications
            if r.content.contains("Pipeline #") && r.content.contains("has") {
                return false;
            }

            true
        }).collect())
    }
}
