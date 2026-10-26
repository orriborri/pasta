use kb_fetchers::assistant::fetch;
use pasta_assistant::Store;

#[test]
fn restart_preserves_original_versions_and_collision_free_logical_identity() {
    let path = std::env::temp_dir().join(format!("assistant-bridge-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    assert!(fetch(&path).is_err());
    assert!(!path.exists());
    let store = Store::open(&path).unwrap();
    let first = store
        .capture("telegram", "owner/chat/message", "Call electrician")
        .unwrap();
    assert_eq!(
        store
            .capture("telegram", "owner/chat/message", "Call electrician")
            .unwrap(),
        first
    );
    store
        .capture("telegram", "owner/chat/message", "Call plumber instead")
        .unwrap();
    store.capture("a:b", "c", "Boundary one").unwrap();
    store.capture("a", "b:c", "Boundary two").unwrap();
    drop(store);
    let records = fetch(&path).unwrap();
    assert_eq!(records.len(), 4);
    let original = records
        .iter()
        .find(|r| r.id == format!("vault-assistant-{first}"))
        .unwrap();
    assert_eq!(original.content, "Call electrician");
    assert_eq!(original.url, format!("assistant:evidence:{first}"));
    assert_eq!(
        records
            .iter()
            .find(|r| r.content == "Call plumber instead")
            .unwrap()
            .thread_id,
        original.thread_id
    );
    assert_ne!(
        records
            .iter()
            .find(|r| r.content == "Boundary one")
            .unwrap()
            .thread_id,
        records
            .iter()
            .find(|r| r.content == "Boundary two")
            .unwrap()
            .thread_id
    );
    let replay = fetch(&path).unwrap();
    assert_eq!(
        records
            .iter()
            .map(kb_core::Record::content_hash)
            .collect::<Vec<_>>(),
        replay
            .iter()
            .map(kb_core::Record::content_hash)
            .collect::<Vec<_>>()
    );
    std::fs::remove_file(path).unwrap();
}
