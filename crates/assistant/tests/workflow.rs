use chrono::{TimeZone, Utc};
use pasta_assistant::Store;
use std::path::Path;

#[test]
fn captures_are_versioned_and_task_retries_are_safe() {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    let a = s.capture("gmail", "account/message", "original").unwrap();
    assert_eq!(
        a,
        s.capture("gmail", "account/message", "original").unwrap()
    );
    let b = s.capture("gmail", "account/message", "edited").unwrap();
    assert_ne!(a, b);
    assert_eq!(s.evidence(&a).unwrap()["content"], "original");
    let task = s.add_task("request-1", "Reply", &a, None, None, 1).unwrap();
    assert_eq!(
        task.id,
        s.add_task("request-1", "Reply", &a, None, None, 1)
            .unwrap()
            .id
    );
    assert!(s
        .add_task("request-1", "Different", &a, None, None, 1)
        .is_err());
    assert!(s
        .add_task("request-2", "Missing source", "unknown", None, None, 1)
        .is_err());
    s.done(task.id).unwrap();
    s.done(task.id).unwrap();
    assert!(s.next(Utc::now(), 5).unwrap().is_empty());
    assert!(s.done(999).is_err());
}
#[test]
fn next_prioritizes_overdue_and_capture_does_not_create_tasks() {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    let e = s
        .capture("telegram", "chat/1", "an idea, not a commitment")
        .unwrap();
    let now = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
    assert!(s.next(now, 5).unwrap().is_empty());
    s.add_task("later", "Important", &e, None, None, 3).unwrap();
    s.add_task(
        "due",
        "Reply now",
        &e,
        None,
        Some("2026-10-26T10:00:00+02:00"),
        0,
    )
    .unwrap();
    assert_eq!(s.next(now, 5).unwrap()[0].title, "Reply now");
    assert!(s
        .add_task("bad", "Ambiguous", &e, None, Some("tomorrow"), 1)
        .is_err());
}
#[test]
fn restart_retains_sources_and_commitments() {
    let path = std::env::temp_dir().join(format!(
        "pasta-assistant-test-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let id;
    {
        let mut s = Store::open(&path).unwrap();
        id = s.capture("memo", "1", "Call back").unwrap();
        s.add_task("call", "Call back", &id, None, None, 1).unwrap();
    }
    let s = Store::open(&path).unwrap();
    assert_eq!(s.next(Utc::now(), 5).unwrap().len(), 1);
    assert_eq!(s.evidence(&id).unwrap()["content"], "Call back");
    drop(s);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn registry_preserves_string_ids_and_rejects_invalid_or_duplicate_ids() {
    let path =
        std::env::temp_dir().join(format!("pasta-registry-test-{}.json", std::process::id()));
    std::fs::write(&path, r#"{"entries":[{"jd_id":"00.01","label":"Inbox"}]}"#).unwrap();
    assert!(pasta_assistant::Registry::load(&path)
        .unwrap()
        .contains("00.01"));
    for text in [
        r#"{"entries":[{"jd_id":0.01,"label":"Inbox"}]}"#,
        r#"{"entries":[{"jd_id":"0.01","label":"Inbox"}]}"#,
        r#"{"entries":[{"jd_id":"00.01","label":"A"},{"jd_id":"00.01","label":"B"}]}"#,
    ] {
        std::fs::write(&path, text).unwrap();
        assert!(pasta_assistant::Registry::load(&path).is_err());
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mcp_exposes_shared_attention_tools_over_stdio() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let path = std::env::temp_dir().join(format!("pasta-mcp-test-{}.sqlite", std::process::id()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_pasta-assistant"))
        .args(["--database", path.to_str().unwrap(), "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    writeln!(input, "{}", serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let init: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(init.get("result").is_some(), "{line}");
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    let names: Vec<_> = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"capture"));
    assert!(names.contains(&"next_actions"));
    assert!(names.contains(&"complete_task"));
    writeln!(input, "{}", serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"capture","arguments":{"source":"telegram","source_id":"owner-chat/message","content":"verbatim memo"}}})).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let capture: serde_json::Value = serde_json::from_str(&line).unwrap();
    let id = capture["result"]["structuredContent"]["result"]["evidence_id"]
        .as_str()
        .unwrap();
    assert_eq!(
        Store::open(&path).unwrap().evidence(id).unwrap()["content"],
        "verbatim memo"
    );
    child.kill().unwrap();
    child.wait().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn task_lifecycle_survives_restore_and_rejects_stale_or_cyclic_edits() {
    use pasta_assistant::TaskEdit;
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    let evidence = store
        .capture("telegram", "owner/chat/99", "Call electrician")
        .unwrap();
    let task = store
        .add_task("create", "Call electrician", &evidence, None, None, 1)
        .unwrap();
    let mut edit = TaskEdit {
        request_key: "defer".into(),
        task_id: task.id,
        expected_revision: 0,
        title: "Call electrician on Tuesday".into(),
        due_at: None,
        priority: 2,
        status: "deferred".into(),
        deferred_until: Some("2026-10-27T09:00:00+02:00".into()),
        waiting_reason: None,
        blocked_by: None,
    };
    let deferred = store.edit_task(&edit).unwrap();
    assert_eq!(store.edit_task(&edit).unwrap().revision, deferred.revision);
    let before = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
    assert!(store.next(before, 5).unwrap().is_empty());
    let after = Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap();
    assert_eq!(store.next(after, 5).unwrap().len(), 1);
    edit.request_key = "stale".into();
    assert!(store.edit_task(&edit).is_err());
    edit.expected_revision = 1;
    edit.blocked_by = Some(task.id);
    assert!(store.edit_task(&edit).is_err());
    edit.blocked_by = None;
    edit.status = "waiting".into();
    edit.deferred_until = None;
    assert!(store.edit_task(&edit).is_err());
    edit.waiting_reason = Some("Electrician reply".into());
    store.edit_task(&edit).unwrap();
    assert!(store.next(after, 5).unwrap().is_empty());
    for (revision, status) in [(2, "canceled"), (3, "open")] {
        edit.request_key = status.into();
        edit.expected_revision = revision;
        edit.status = status.into();
        edit.waiting_reason = None;
        store.edit_task(&edit).unwrap();
    }
    let backup =
        std::env::temp_dir().join(format!("assistant-restore-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&backup);
    store.backup(&backup).unwrap();
    assert!(store.backup(&backup).is_err());
    let mut restored = Store::open(&backup).unwrap();
    assert_eq!(restored.task(task.id).unwrap().revision, 4);
    assert_eq!(
        restored.evidence(&evidence).unwrap()["content"],
        "Call electrician"
    );
    restored.done(task.id).unwrap();
    assert!(restored.next(after, 5).unwrap().is_empty());
    assert_eq!(store.next(after, 5).unwrap().len(), 1);
    drop(restored);
    std::fs::remove_file(backup).unwrap();
}

#[test]
fn mcp_task_edit_and_cli_read_share_one_register() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let path = std::env::temp_dir().join(format!("pasta-mcp-edit-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pasta-assistant"))
        .args(["--database", path.to_str().unwrap(), "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    {
        let mut call = |request: serde_json::Value| {
            writeln!(input, "{request}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str::<serde_json::Value>(&line).unwrap()
        };
        call(
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"lifecycle","version":"1"}}}),
        );
    }
    // Notifications produce no response.
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let mut call = |id: u64, name: &str, args: serde_json::Value| {
        writeln!(input, "{}", serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let response: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(response["result"]["isError"] != true, "{response}");
        response["result"]["structuredContent"]["result"].clone()
    };
    let capture = call(
        2,
        "capture",
        serde_json::json!({"source":"telegram","source_id":"owner/chat/42","content":"Call back"}),
    );
    assert_eq!(capture["indexing"], "not_verified");
    let task = call(
        3,
        "add_task",
        serde_json::json!({"request_key":"mcp-create","title":"Call back","evidence_id":capture["evidence_id"],"priority":1}),
    );
    let edit = serde_json::json!({"request_key":"mcp-defer","task_id":task["id"],"expected_revision":0,"title":"Call tomorrow","priority":2,"status":"deferred","deferred_until":"2099-01-01T10:00:00+02:00"});
    let revised = call(4, "edit_task", edit.clone());
    assert_eq!(call(5, "edit_task", edit), revised);
    assert_eq!(
        call(6, "next_actions", serde_json::json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    child.kill().unwrap();
    child.wait().unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_pasta-assistant"))
        .args([
            "--database",
            path.to_str().unwrap(),
            "get",
            task["id"].as_i64().unwrap().to_string().as_str(),
        ])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let fetched: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(fetched, revised);
    std::fs::remove_file(path).unwrap();
}
