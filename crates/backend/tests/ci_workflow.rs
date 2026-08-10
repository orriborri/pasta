//! Tests for the CI gate (Requirement 8.5 / tasks.md 7.2).
//!
//! These tests verify that a CI workflow exists and enforces the three
//! commands this change relies on as its safety net:
//!   - `cargo build --workspace`
//!   - `cargo clippy --workspace -- -D warnings`
//!   - `cargo test --workspace`
//!
//! The workflow file itself is authored as plain YAML, so there is no
//! production Rust code to call directly. These tests read the workflow
//! file from disk and assert on its structure/content instead. They MUST
//! fail until the workflow file is added by the implementation.

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

fn ci_workflow_path() -> PathBuf {
    workspace_root().join(".github/workflows/ci.yml")
}

#[test]
fn ci_workflow_file_exists() {
    let path = ci_workflow_path();
    assert!(
        path.is_file(),
        "expected a CI workflow at {}, but it does not exist",
        path.display()
    );
}

#[test]
fn ci_workflow_runs_cargo_build_workspace() {
    let content = read_ci_workflow();
    assert!(
        content.contains("cargo build --workspace"),
        "CI workflow must run `cargo build --workspace`"
    );
}

#[test]
fn ci_workflow_runs_cargo_clippy_with_warnings_denied() {
    let content = read_ci_workflow();
    // Accept either a single combined flag or separated tokens, but the
    // workflow must deny warnings across the whole workspace.
    assert!(
        content.contains("cargo clippy --workspace"),
        "CI workflow must run `cargo clippy --workspace`"
    );
    assert!(
        content.contains("-D warnings"),
        "CI workflow's clippy invocation must deny warnings (`-D warnings`)"
    );
}

#[test]
fn ci_workflow_runs_cargo_test_workspace() {
    let content = read_ci_workflow();
    assert!(
        content.contains("cargo test --workspace"),
        "CI workflow must run `cargo test --workspace`"
    );
}

#[test]
fn ci_workflow_triggers_on_push_and_pull_request() {
    let content = read_ci_workflow();
    let on_section = section(&content, "on:").unwrap_or_else(|| {
        panic!("CI workflow must declare an `on:` trigger section");
    });

    assert!(
        on_section.contains("push"),
        "CI workflow must trigger on push, so regressions on the main branch are caught"
    );
    assert!(
        on_section.contains("pull_request"),
        "CI workflow must trigger on pull_request, so it gates merges"
    );
}

#[test]
fn ci_workflow_declares_a_jobs_section() {
    let content = read_ci_workflow();
    assert!(
        content.contains("jobs:"),
        "CI workflow must declare a `jobs:` section"
    );
}

/// Return the block of lines under a top-level `key:` line, up to (but not
/// including) the next top-level (non-indented) key. Avoids a YAML-parser
/// dependency for what is otherwise a handful of substring assertions.
fn section<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let start = content.find(key)? + key.len();
    let rest = &content[start..];
    let end = rest
        .lines()
        .scan(0usize, |consumed, line| {
            let line_start = *consumed;
            *consumed += line.len() + 1; // +1 for the newline
            Some((line_start, line))
        })
        .find(|(_, line)| !line.trim().is_empty() && !line.starts_with([' ', '\t']))
        .map(|(offset, _)| offset);
    Some(match end {
        Some(offset) => &rest[..offset],
        None => rest,
    })
}

fn read_ci_workflow() -> String {
    let path = ci_workflow_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read CI workflow at {}: {e}", path.display()))
}
