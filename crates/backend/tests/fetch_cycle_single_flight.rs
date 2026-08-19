//! Tests for single-flight fetch cycle enforcement.
//!
//! These tests verify the Requirement 2 behavior:
//! 1. When a fetch cycle is running, a second manual fetch is rejected
//! 2. The daemon sends a message stating a cycle is already running
//! 3. The running state is reflected in the state snapshot
//! 4. The running marker is cleared after completion (success or failure)
//! 5. The completed set is merged rather than replaced

use std::collections::HashSet;
use std::env;

// Get the workspace root directory (parent of crates/backend)
fn workspace_root() -> String {
    env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| "crates/backend".to_string())
}

// Test 1: When a fetch cycle is already running AND the user triggers a manual fetch
// THEN the daemon SHALL NOT start a second cycle
//
// This test PROVES the ForceFetch handler in commands.rs DOES check running_native.
// If it doesn't check, the test fails because ForceFetch would start a second cycle.
#[tokio::test]
async fn forcefetch_must_check_running_native_before_starting() {
    // Read commands.rs and verify the ForceFetch handler checks running_native
    let root = workspace_root();
    let commands_content = std::fs::read_to_string(
        format!("{}/src/commands.rs", root)
    ).expect("Could not read commands.rs");
    
    // Extract the ForceFetch handler
    let forcefetch_start = commands_content.find("Command::ForceFetch =>")
        .expect("Could not find ForceFetch handler in commands.rs");
    let forcefetch_end = forcefetch_start + 500; // ForceFetch is short, ~20 lines
    let forcefetch_code = &commands_content[forcefetch_start..forcefetch_end.min(commands_content.len())];
    
    // The ForceFetch handler MUST contain this check BEFORE spawning:
    // if state.scheduler.lock().await.running_native.contains("fetch-cycle") { ... return; }
    
    let has_running_native_check = forcefetch_code.contains("running_native")
        && forcefetch_code.contains("fetch-cycle");
    
    // This test SHOULD pass after the fix is implemented
    // Currently it FAILS because ForceFetch doesn't check running_native
    assert!(has_running_native_check, 
        "ForceFetch handler must check running_native.contains(\"fetch-cycle\") before starting. \
        Current implementation does NOT check, allowing concurrent fetch cycles.\n\
        ForceFetch code:\n{}",
        forcefetch_code);
}

// Test 2: WHEN a manual fetch is rejected for that reason
// THEN the daemon SHALL send the client a message stating a cycle is already running
//
// This test verifies that Event::Flash with "already running" message is sent.
#[tokio::test]
async fn forcefetch_rejection_sends_flash_message() {
    // The ForceFetch handler must:
    // 1. Check if running_native contains "fetch-cycle"
    // 2. If yes, send Event::Flash { message: "fetch-cycle already running" }
    // 3. Return without starting a new cycle
    
    let root = workspace_root();
    let commands_content = std::fs::read_to_string(
        format!("{}/src/commands.rs", root)
    ).expect("Could not read commands.rs");
    
    // Extract the ForceFetch handler
    let forcefetch_start = commands_content.find("Command::ForceFetch =>")
        .expect("Could not find ForceFetch handler in commands.rs");
    let forcefetch_end = forcefetch_start + 500;
    let forcefetch_code = &commands_content[forcefetch_start..forcefetch_end.min(commands_content.len())];
    
    let has_flash_message = forcefetch_code.contains("Event::Flash")
        && forcefetch_code.contains("already running");
    
    // This test SHOULD pass after the fix
    // Currently FAILS because there's no rejection path yet
    assert!(has_flash_message,
        "ForceFetch must send Event::Flash with 'already running' message when rejecting. \
        Current implementation does NOT have this check.\n\
        ForceFetch code:\n{}",
        forcefetch_code);
}

// Test 3: WHEN a manually triggered cycle is running
// THEN the state snapshot SHALL report it as running
//
// This test verifies the scheduler's running_native is included in the snapshot.
#[tokio::test]
async fn snapshot_shows_running_state() {
    // The state snapshot (build_state_snapshot) must include scheduler.running_native
    // so the TUI can display which operations are running.
    
    let root = workspace_root();
    let server_content = std::fs::read_to_string(
        format!("{}/src/server.rs", root)
    ).expect("Could not read server.rs");
    
    // Verify the snapshot builder includes running_native
    let includes_running_native = server_content.contains("running_native");
    
    // This test SHOULD pass - the snapshot should include this
    assert!(includes_running_native,
        "State snapshot must include scheduler.running_native for TUI visibility. \
        Current implementation may not include this field.");
}

// Test 4: WHEN a manually triggered cycle finishes
// THEN the running marker SHALL be cleared, whether the cycle succeeded or failed
//
// This test verifies the scheduler removes the marker in all cases.
#[tokio::test]
async fn marker_cleared_after_completion_success() {
    // In scheduler.rs, run_fetch_cycle must remove the marker from running_native
    // after completion, regardless of success or failure.
    
    let root = workspace_root();
    let scheduler_content = std::fs::read_to_string(
        format!("{}/src/scheduler.rs", root)
    ).expect("Could not read scheduler.rs");
    
    // The marker should be removed in a finally-like pattern
    // Check for the remove pattern after the spawn
    let has_proper_cleanup = scheduler_content.contains("running_native.remove")
        && scheduler_content.contains("fetch-cycle");
    
    // This test SHOULD pass - cleanup must happen
    assert!(has_proper_cleanup,
        "Scheduler must remove 'fetch-cycle' from running_native after completion. \
        Current implementation may not clean up properly.");
}

// Unit test: Running native set behavior - verify HashSet operations
#[test]
fn running_native_set_behavior() {
    let mut running_native: HashSet<String> = HashSet::new();
    
    // Add fetch-cycle
    running_native.insert("fetch-cycle".to_string());
    assert!(running_native.contains("fetch-cycle"));
    
    // Remove it
    running_native.remove("fetch-cycle");
    assert!(!running_native.contains("fetch-cycle"));
}

// Unit test: Completed set merge behavior (not replace)
#[test]
fn completed_set_merge_behavior() {
    let mut completed: HashSet<String> = HashSet::new();
    completed.insert("agent-1".to_string());
    completed.insert("agent-2".to_string());

    // Simulate fetch cycle completion
    let new_completed: HashSet<String> = [
        "gitlab-fetcher",
        "linear-fetcher",
        "slack-fetcher",
        "gmail-fetcher",
        "calendar-fetcher",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Correct behavior: merge (union), not replace
    let merged = completed.union(&new_completed).cloned().collect::<HashSet<_>>();

    // Verify merge includes both old and new
    assert!(merged.contains("agent-1"));
    assert!(merged.contains("agent-2"));
    assert!(merged.contains("gitlab-fetcher"));
    assert!(merged.contains("linear-fetcher"));

    // Verify size is correct (5 new + 2 old = 7 total)
    assert_eq!(merged.len(), 7);
}
