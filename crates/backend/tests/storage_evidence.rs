//! Tests for the storage evidence record (Requirement 7 / tasks.md 6.1-6.4).
//!
//! This change deliberately does not consolidate the four storage engines
//! (Tantivy, `LanceDB`, Parquet, SQLite). Instead it produces the evidence
//! needed to decide that separately: which code paths query which store,
//! whether the `vault` source is actually indexed, and what retrieval
//! quality/latency and full-corpus-read cost look like today.
//!
//! That evidence is a written record, not a code change, so there is no
//! production function to call directly. These tests read the record from
//! disk and assert on its required content instead, the same pattern used
//! for the CI workflow gate in `ci_workflow.rs`. They MUST fail until the
//! evidence document is written.

use std::path::PathBuf;

/// Resolve the workspace root from `CARGO_MANIFEST_DIR` (which is
/// `<repo>/crates/backend` for this crate), rather than assuming CWD.
fn workspace_root() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by cargo");
    PathBuf::from(manifest_dir)
        .parent() // crates
        .and_then(|p| p.parent()) // repo root
        .expect("crates/backend must have two parent directories")
        .to_path_buf()
}

fn evidence_path() -> PathBuf {
    workspace_root()
        .join("openspec/changes/remove-accreted-architecture/STORAGE_EVIDENCE.md")
}

fn read_evidence() -> String {
    let path = evidence_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read storage evidence record at {}: {e}", path.display()))
}

#[test]
fn storage_evidence_file_exists() {
    let path = evidence_path();
    assert!(
        path.is_file(),
        "expected a storage evidence record at {}, but it does not exist",
        path.display()
    );
}

/// Requirement 7.1: the record must name which code paths query each of the
/// four stores, not just assert that four stores exist.
#[test]
fn records_which_code_paths_query_tantivy() {
    let content = read_evidence();
    assert!(
        content.contains("text_index.rs") || content.contains("TextIndex"),
        "evidence must name the Tantivy code path (text_index.rs / TextIndex)"
    );
}

#[test]
fn records_which_code_paths_query_lancedb() {
    let content = read_evidence();
    assert!(
        content.contains("vector_store.rs") || content.contains("VectorStore"),
        "evidence must name the LanceDB code path (vector_store.rs / VectorStore)"
    );
}

#[test]
fn records_which_code_paths_read_parquet_directly() {
    let content = read_evidence();
    assert!(
        content.contains("parquet_store.rs") || content.contains("ParquetStore"),
        "evidence must name the Parquet code path (parquet_store.rs / ParquetStore)"
    );
    // Direct-read call sites the design doc identifies, distinct from the
    // stores queried only through hybrid_search.
    assert!(
        content.contains("inbox.rs"),
        "evidence must name inbox.rs as a direct ParquetStore::read_all caller"
    );
    assert!(
        content.contains("vault_organize.rs"),
        "evidence must name vault_organize.rs (PARA audit / daily feed) as a direct ParquetStore::read_all caller"
    );
}

#[test]
fn records_the_sqlite_store() {
    let content = read_evidence();
    assert!(
        content.contains("sync_state.rs") || content.contains("state.db") || content.contains("SQLite") || content.contains("sqlite"),
        "evidence must name the SQLite store (sync_state.rs / state.db)"
    );
}

/// Requirement 7.2: the vault source must be indexed by the daemon, or the
/// reason for excluding it must be recorded. As of this change, `fetch_cycle`
/// only fetches `["gmail", "linear", "calendar", "slack"]` — vault is absent.
/// The evidence must say so explicitly, since `audit_para` and
/// `route_new_tasks_semantic` both filter on `source == "vault"` and are
/// silently inert without it.
#[test]
fn records_vault_source_indexing_status() {
    let content = read_evidence();
    assert!(
        content.contains("vault"),
        "evidence must discuss the `vault` source's indexing status"
    );
    assert!(
        content.contains("fetch_cycle") || content.contains("gmail\", \"linear\", \"calendar\", \"slack\"") || content.contains("gmail, linear, calendar, slack"),
        "evidence must reference fetch_cycle's source list to show whether vault is included"
    );
    assert!(
        content.contains("audit_para") || content.contains("route_new_tasks_semantic"),
        "evidence must name the consumers left silently inert without a vault index (audit_para, route_new_tasks_semantic)"
    );
}

/// Requirement 7.3: latency and results measured for the same queries,
/// Tantivy-only vs the hybrid path, on the real corpus.
#[test]
fn records_latency_and_quality_comparison() {
    let content = read_evidence();
    assert!(
        content.contains("Tantivy-only") || content.contains("Tantivy only") || content.contains("text-only") || content.contains("full-text only"),
        "evidence must describe a Tantivy-only measurement"
    );
    assert!(
        content.contains("hybrid"),
        "evidence must describe a hybrid (vector + full-text) measurement"
    );
    assert!(
        content.to_lowercase().contains("latency") || content.to_lowercase().contains("ms")  || content.to_lowercase().contains("seconds"),
        "evidence must report a latency figure for the comparison"
    );
}

/// Requirement 7.4: the cost of `read_all` across the Parquet file set.
#[test]
fn records_full_corpus_read_all_cost() {
    let content = read_evidence();
    assert!(
        content.contains("read_all"),
        "evidence must name read_all as the measured operation"
    );
    assert!(
        content.to_lowercase().contains("latency") || content.to_lowercase().contains("ms") || content.to_lowercase().contains("seconds") || content.to_lowercase().contains("cost"),
        "evidence must report a cost/latency figure for read_all across the Parquet file set"
    );
}

/// The evidence phase must not itself claim to make the consolidation
/// decision — that is explicitly a follow-up change (tasks.md 6.5).
#[test]
fn does_not_claim_to_decide_consolidation() {
    let content = read_evidence();
    assert!(
        content.to_lowercase().contains("follow-up") || content.to_lowercase().contains("follow up"),
        "evidence must point to a follow-up change for the consolidation decision, not decide it here"
    );
}
