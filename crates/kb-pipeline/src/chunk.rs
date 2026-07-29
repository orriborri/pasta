use anyhow::Result;
use kb_core::{Kind, Record};
use std::collections::HashMap;

use crate::PipelineStage;

/// Groups individual messages into thread-level records by `thread_id`.
/// Keeps thread replies together so search finds conversations, not fragments.
pub struct ChunkStage;

impl PipelineStage for ChunkStage {
    fn name(&self) -> &'static str { "chunk" }

    /// Group messages sharing a `thread_id` into single records.
    ///
    /// # Errors
    /// This stage does not fail.
    fn process(&self, records: Vec<Record>) -> Result<Vec<Record>> {
        let mut threads: HashMap<String, Vec<Record>> = HashMap::new();
        let mut standalone: Vec<Record> = Vec::new();

        for r in records {
            if r.thread_id.is_empty() || r.kind != Kind::Message {
                standalone.push(r);
            } else {
                threads.entry(r.thread_id.clone()).or_default().push(r);
            }
        }

        let mut result = standalone;

        for (thread_id, mut msgs) in threads {
            if msgs.len() == 1 {
                result.push(msgs.remove(0));
                continue;
            }

            // Sort by timestamp
            msgs.sort_by_key(|m| m.created_at);

            // Merge into one thread record
            let first = &msgs[0];
            let last = &msgs[msgs.len() - 1];

            let content = msgs.iter()
                .map(|m| format!("**{}**: {}", m.author, m.content))
                .collect::<Vec<_>>()
                .join("\n\n");

            let participants: Vec<String> = msgs.iter()
                .map(|m| m.author.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            let mut entities: Vec<String> = Vec::new();
            for m in &msgs {
                for e in &m.entities {
                    if !entities.contains(e) {
                        entities.push(e.clone());
                    }
                }
            }

            result.push(Record {
                id: first.id.clone(),
                source: first.source,
                kind: Kind::Thread,
                title: first.title.clone(),
                content,
                author: first.author.clone(),
                participants,
                created_at: first.created_at,
                updated_at: last.updated_at,
                url: first.url.clone(),
                thread_id,
                entities,
                tags: first.tags.clone(),
            });
        }

        Ok(result)
    }
}
