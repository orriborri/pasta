use anyhow::Result;
use kb_core::Record;
use regex::Regex;

use crate::PipelineStage;

/// Extracts structured entity references from content using regex patterns.
pub struct ExtractStage {
    linear: Regex,
    mr: Regex,
    mr_url: Regex,
}

impl Default for ExtractStage {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractStage {
    /// Create a new extract stage with default regex patterns.
    ///
    /// # Panics
    /// Panics if the hardcoded regex patterns are invalid (should never happen).
    #[must_use]
    pub fn new() -> Self {
        Self {
            linear: Regex::new(r"[A-Z]{2,5}-\d+").unwrap(),
            mr: Regex::new(r"!(\d{3,5})").unwrap(),
            mr_url: Regex::new(r"/merge_requests/(\d+)").unwrap(),
        }
    }
}

impl PipelineStage for ExtractStage {
    fn name(&self) -> &'static str { "extract" }

    fn process(&self, mut records: Vec<Record>) -> Result<Vec<Record>> {
        for r in &mut records {
            let mut entities: Vec<String> = Vec::new();

            for m in self.linear.find_iter(&r.content) {
                let e = format!("linear:{}", m.as_str());
                if !entities.contains(&e) {
                    entities.push(e);
                }
            }

            for cap in self.mr.captures_iter(&r.content) {
                let num = cap.get(1).unwrap().as_str();
                let e = format!("mr:!{num}");
                if !entities.contains(&e) {
                    entities.push(e);
                }
            }

            for cap in self.mr_url.captures_iter(&r.content) {
                let num = cap.get(1).unwrap().as_str();
                let e = format!("mr:!{num}");
                if !entities.contains(&e) {
                    entities.push(e);
                }
            }

            for e in entities {
                if !r.entities.contains(&e) {
                    r.entities.push(e);
                }
            }
        }
        Ok(records)
    }
}
