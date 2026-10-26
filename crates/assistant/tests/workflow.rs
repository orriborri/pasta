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
