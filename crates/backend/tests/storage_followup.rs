//! Tests for the storage consolidation follow-up change (tasks.md 6.5).
//!
//! `remove-accreted-architecture` deliberately does not consolidate the four
//! storage engines (Tantivy, `LanceDB`, Parquet, SQLite) — `STORAGE_EVIDENCE.md`
//! (tasks 6.1-6.4) records the evidence that decision needs, but stops short
//! of deciding it. Task 6.5 is to open a new openspec change proposal that
//! carries that evidence forward into a recommendation, so the storage
//! question is a decision made from a follow-up change, not a guess.
//!
//! Like `storage_evidence.rs` and `ci_workflow.rs`, there is no production
//! function to call: the deliverable is a written artifact (an openspec
//! change proposal). These tests read it from disk and assert on its
//! required content. They MUST fail until the follow-up change is opened.

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

/// The follow-up change's directory under `openspec/changes/`. The exact
/// slug is an implementation detail, but it must be discoverable by name —
/// tests search for any change directory whose proposal names storage
/// consolidation, rather than hardcoding one slug, so the author has latitude
/// to name it descriptively.
fn find_storage_followup_dir() -> Option<PathBuf> {
    let changes_dir = workspace_root().join("openspec/changes");
    let entries = std::fs::read_dir(&changes_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip the archive directory and the change this task is part of.
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "archive" || name == "remove-accreted-architecture" {
            continue;
        }
        let proposal_path = path.join("proposal.md");
        if let Ok(content) = std::fs::read_to_string(&proposal_path) {
            let lower = content.to_lowercase();
            let mentions_storage_engines = lower.contains("tantivy")
                && lower.contains("lancedb")
                && (lower.contains("parquet") || lower.contains("sqlite"));
            if mentions_storage_engines {
                return Some(path);
            }
        }
    }
    None
}

#[test]
fn storage_followup_change_exists() {
    let found = find_storage_followup_dir();
    assert!(
        found.is_some(),
        "expected an openspec change under openspec/changes/ proposing storage \
         consolidation (naming Tantivy, LanceDB, and Parquet/SQLite in its \
         proposal.md), but none was found"
    );
}

#[test]
fn storage_followup_proposal_references_the_evidence_document() {
    let dir = find_storage_followup_dir().expect("storage follow-up change must exist");
    let content = std::fs::read_to_string(dir.join("proposal.md"))
        .expect("follow-up change must have a proposal.md");
    assert!(
        content.contains("STORAGE_EVIDENCE.md") || content.contains("storage evidence"),
        "follow-up proposal must reference the evidence document its recommendation is based on"
    );
}

#[test]
fn storage_followup_proposal_carries_a_recommendation() {
    let dir = find_storage_followup_dir().expect("storage follow-up change must exist");
    let content = std::fs::read_to_string(dir.join("proposal.md"))
        .expect("follow-up change must have a proposal.md");
    let lower = content.to_lowercase();
    assert!(
        lower.contains("recommend"),
        "follow-up proposal must carry a recommendation, not just restate the evidence"
    );
}

/// The evidence document names three items a follow-up must address:
/// whether to index the `vault` source, whether LanceDB should be persistent
/// or rebuilt in-memory, and whether Parquet reads need file-level pushdown.
/// The follow-up proposal must engage with at least the vault-indexing
/// question, since that is the one the evidence flags as silently masking
/// stale results today.
#[test]
fn storage_followup_addresses_vault_indexing_question() {
    let dir = find_storage_followup_dir().expect("storage follow-up change must exist");
    let content = std::fs::read_to_string(dir.join("proposal.md"))
        .expect("follow-up change must have a proposal.md");
    assert!(
        content.to_lowercase().contains("vault"),
        "follow-up proposal must address whether the vault source should be indexed \
         by the automated fetch cycle, per STORAGE_EVIDENCE.md 6.2"
    );
}

/// The follow-up change must not have been silently implemented: this
/// change (`remove-accreted-architecture`) explicitly scopes storage
/// consolidation as a non-goal, so the follow-up should exist as a proposal
/// only, not as a completed/archived change.
#[test]
fn storage_followup_is_not_archived() {
    let dir = find_storage_followup_dir().expect("storage follow-up change must exist");
    let path_str = dir.to_string_lossy();
    assert!(
        !path_str.contains("/changes/archive/"),
        "the storage follow-up must be an open proposal, not archived — \
         this change does not decide or implement storage consolidation"
    );
}
