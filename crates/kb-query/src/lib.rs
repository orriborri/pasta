//! Typed, read-only query layer over the evidence graph and Parquet.
//!
//! This crate turns the evidence graph (`graph.db`) and the record source of
//! truth (Parquet) into structured, typed results: [`get_entity`],
//! [`get_related`], [`get_evidence`], [`get_context`], [`get_timeline`], and a
//! structured [`search`]. Every relation returned carries its
//! `evidence_record_id` and [`Derivation`], so a consumer can always trace a
//! claim back to a record. Nothing here writes or fetches — it only reads.

use anyhow::Result;
use chrono::{DateTime, Utc};
use kb_core::{EntityRef, KbConfig, Record, RelationKind};
use kb_storage::{GraphStore, ParquetStore, StoredRelation};
use schemars::JsonSchema;
use serde::Serialize;

/// A resolved entity with its presence and degree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EntityView {
    /// Canonical reference string, e.g. `"linear:AB-123"`.
    pub entity: String,
    /// The typed kind tag, e.g. `"linear"`.
    pub kind: String,
    /// Whether a backing record exists for this entity in Parquet.
    pub present: bool,
    /// Number of relations where this entity is the subject.
    pub out_degree: usize,
    /// Number of relations where this entity is the object.
    pub in_degree: usize,
}

/// A relation with its evidence and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelationView {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence_record_id: String,
    /// Provenance: `"explicit"` or `"rule:<name>"`.
    pub derivation: String,
    pub created_at: String,
}

impl RelationView {
    fn from_stored(r: &StoredRelation) -> Self {
        Self {
            subject: r.subject.canonical(),
            predicate: r.predicate.as_tag().to_string(),
            object: r.object.canonical(),
            evidence_record_id: r.evidence_record_id.clone(),
            derivation: r.derivation.canonical(),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// A record resolved as evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EvidenceView {
    pub record_id: String,
    pub source: String,
    pub kind: String,
    pub title: String,
    pub snippet: String,
    pub url: String,
    pub created_at: String,
}

impl EvidenceView {
    fn from_record(r: &Record) -> Self {
        let snippet: String = r.content.chars().take(400).collect();
        Self {
            record_id: r.id.clone(),
            source: r.source.to_string(),
            kind: kind_tag(r.kind).to_string(),
            title: r.title.clone(),
            snippet,
            url: r.url.clone(),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// An entity plus its relations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedView {
    pub entity: EntityView,
    pub relations: Vec<RelationView>,
}

/// An entity, its relations, and the evidence behind them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ContextView {
    pub entity: EntityView,
    pub relations: Vec<RelationView>,
    pub evidence: Vec<EvidenceView>,
}

/// A single time-ordered entry touching an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TimelineEntry {
    pub at: String,
    pub record: EvidenceView,
    pub relation: Option<RelationView>,
}

/// A typed search hit.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SearchHit {
    pub record_id: String,
    pub source: String,
    pub title: String,
    pub snippet: String,
    pub url: String,
    pub created_at: String,
    pub score: f32,
}

const fn kind_tag(kind: kb_core::Kind) -> &'static str {
    match kind {
        kb_core::Kind::Message => "message",
        kb_core::Kind::Thread => "thread",
        kb_core::Kind::Issue => "issue",
        kb_core::Kind::Commit => "commit",
        kb_core::Kind::Doc => "doc",
        kb_core::Kind::Event => "event",
    }
}

/// Deterministic ordering key for a stored relation.
fn order_key(r: &StoredRelation) -> (DateTime<Utc>, String) {
    (
        r.created_at,
        format!(
            "{}|{}|{}",
            r.subject.canonical(),
            r.predicate.as_tag(),
            r.object.canonical()
        ),
    )
}

/// Resolve an entity to a typed view (presence + degree).
///
/// # Errors
/// Returns error if the graph cannot be read.
pub fn get_entity(config: &KbConfig, entity: &EntityRef) -> Result<EntityView> {
    let graph = GraphStore::open(config)?;
    let (out_degree, in_degree) = graph.entity_degree(entity)?;
    // "Present" means a backing record exists; a relation endpoint with no
    // record is known but not present.
    let present = record_exists(config, entity)?;
    Ok(EntityView {
        entity: entity.canonical(),
        kind: entity.kind.as_tag().to_string(),
        present,
        out_degree,
        in_degree,
    })
}

/// The relations incident to an entity, optionally filtered by kind, ordered
/// deterministically and truncated to `limit`.
///
/// # Errors
/// Returns error if the graph cannot be read.
pub fn get_related(
    config: &KbConfig,
    entity: &EntityRef,
    kind_filter: Option<RelationKind>,
    limit: usize,
) -> Result<RelatedView> {
    let graph = GraphStore::open(config)?;
    let (out_degree, in_degree) = graph.entity_degree(entity)?;
    let mut incident = graph.relations_incident(entity)?;
    if let Some(k) = kind_filter {
        incident.retain(|r| r.predicate == k);
    }
    incident.sort_by_key(order_key);
    incident.truncate(limit);
    let relations = incident.iter().map(RelationView::from_stored).collect();
    Ok(RelatedView {
        entity: EntityView {
            entity: entity.canonical(),
            kind: entity.kind.as_tag().to_string(),
            present: record_exists(config, entity)?,
            out_degree,
            in_degree,
        },
        relations,
    })
}

/// Resolve evidence record ids to structured evidence views. Records absent
/// from Parquet are simply omitted (never fabricated).
///
/// # Errors
/// Returns error if Parquet cannot be read.
pub fn get_evidence(config: &KbConfig, record_ids: &[&str]) -> Result<Vec<EvidenceView>> {
    let parquet = ParquetStore::new(config);
    let records = parquet.get_by_ids(record_ids)?;
    let mut views: Vec<EvidenceView> = records.iter().map(EvidenceView::from_record).collect();
    views.sort_by(|a, b| a.record_id.cmp(&b.record_id));
    Ok(views)
}

/// Assemble an entity context bundle: the entity, its top relations, and the
/// evidence records behind them. Introduces no data beyond graph + Parquet.
///
/// # Errors
/// Returns error if the graph or Parquet cannot be read.
pub fn get_context(config: &KbConfig, entity: &EntityRef, limit: usize) -> Result<ContextView> {
    let related = get_related(config, entity, None, limit)?;
    let ids: Vec<String> = {
        let mut v: Vec<String> = related
            .relations
            .iter()
            .map(|r| r.evidence_record_id.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let evidence = get_evidence(config, &id_refs)?;
    Ok(ContextView {
        entity: related.entity,
        relations: related.relations,
        evidence,
    })
}

/// A time-ordered list of the evidence records touching an entity, ordered by
/// the record's `created_at` with a stable tie-break on record id.
///
/// # Errors
/// Returns error if the graph or Parquet cannot be read.
pub fn get_timeline(
    config: &KbConfig,
    entity: &EntityRef,
    limit: usize,
) -> Result<Vec<TimelineEntry>> {
    let graph = GraphStore::open(config)?;
    let incident = graph.relations_incident(entity)?;

    // Resolve the distinct evidence records once.
    let mut ids: Vec<String> = incident
        .iter()
        .map(|r| r.evidence_record_id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let parquet = ParquetStore::new(config);
    let records = parquet.get_by_ids(&id_refs)?;

    // Map record id -> the first incident relation that cites it, for context.
    let mut entries: Vec<TimelineEntry> = records
        .iter()
        .map(|rec| {
            let relation = incident
                .iter()
                .find(|r| r.evidence_record_id == rec.id)
                .map(RelationView::from_stored);
            TimelineEntry {
                at: rec.created_at.to_rfc3339(),
                record: EvidenceView::from_record(rec),
                relation,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        a.at.cmp(&b.at)
            .then_with(|| a.record.record_id.cmp(&b.record.record_id))
    });
    entries.truncate(limit);
    Ok(entries)
}

/// Structured hybrid search. Reuses `hybrid_search` and maps hits to typed
/// [`SearchHit`]s (carrying `record_id`), optionally filtered by source.
///
/// # Errors
/// Returns error if search fails.
pub async fn search(
    config: &KbConfig,
    query: &str,
    source_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let results = kb_storage::hybrid_search(config, query, limit).await?;
    Ok(results
        .into_iter()
        .filter(|r| source_filter.is_none_or(|s| r.source == s))
        .map(|r| {
            let snippet: String = r.content.chars().take(400).collect();
            SearchHit {
                record_id: r.id,
                source: r.source,
                title: r.title,
                snippet,
                url: r.url,
                created_at: r.created_at,
                score: r.score,
            }
        })
        .collect())
}

/// Whether a backing record exists in Parquet for the entity. Matches the
/// record whose deterministic id or thread/native id corresponds to the ref.
fn record_exists(config: &KbConfig, entity: &EntityRef) -> Result<bool> {
    // The graph stores relation endpoints that may not have records. A record is
    // considered present when Parquet contains a record whose id ends with the
    // entity's native id (the entity id is the native portion of a record id, or
    // a thread id). This is a read-only best-effort presence check.
    let parquet = ParquetStore::new(config);
    let all = parquet.read_all()?;
    let id = &entity.id;
    Ok(all
        .iter()
        .any(|r| r.id == *id || r.id.ends_with(&format!("-{id}")) || r.thread_id == *id))
}
