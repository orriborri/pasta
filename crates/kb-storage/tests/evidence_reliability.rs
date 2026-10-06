use chrono::{TimeZone, Utc};
use kb_core::{KbConfig, Kind, Record, Source};
use kb_storage::ParquetStore;

struct TempStore(KbConfig);
impl TempStore {
    fn new() -> Self {
        let data_dir = std::env::temp_dir().join(format!("evidence-reliability-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        Self(KbConfig { data_dir })
    }
}
impl Drop for TempStore {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0.data_dir); }
}

fn record() -> Record {
    let time = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    Record { id: "vault-test".into(), source: Source::Vault, kind: Kind::Doc,
        title: "Test".into(), content: format!("First\n\n{}\n\nLast", "Important middle fact. ".repeat(200)),
        author: "Lastname, Firstname".into(), participants: vec!["Doe, Jane".into()],
        created_at: time, updated_at: time, url: String::new(), thread_id: String::new(),
        entities: vec!["person:Doe, Jane".into()], tags: vec!["one,two".into()] }
}

#[test]
fn evidence_round_trip_preserves_full_content_and_comma_containing_metadata() {
    let temp = TempStore::new();
    let parquet = ParquetStore::new(&temp.0);
    let original = record();
    parquet.write(std::slice::from_ref(&original)).unwrap();
    let current = parquet.get_by_ids(&[&original.id]).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].content_hash(), original.content_hash());
    let prepared = kb_pipeline::prepare_evidence(current, temp.0.data_dir.join("vault"));
    assert_eq!(prepared[0].content, original.content);
}

#[test]
fn immediate_writes_keep_history_and_current_version_even_with_same_source_timestamp() {
    let temp = TempStore::new();
    let parquet = ParquetStore::new(&temp.0);
    let mut original = record();
    parquet.write(std::slice::from_ref(&original)).unwrap();
    original.content = "New version".into();
    parquet.write(std::slice::from_ref(&original)).unwrap();
    assert_eq!(parquet.read_all().unwrap().len(), 2);
    let latest = parquet.get_by_ids(&[&original.id]).unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].content, "New version");
    assert_eq!(parquet.write_changed(&[original]).unwrap(), 0);
}

#[test]
fn incomplete_temporary_files_do_not_hide_committed_evidence() {
    let temp = TempStore::new();
    let parquet = ParquetStore::new(&temp.0);
    parquet.write(&[record()]).unwrap();
    std::fs::write(temp.0.raw_dir().join("interrupted.parquet.tmp"), b"partial").unwrap();
    assert_eq!(parquet.read_latest().unwrap().len(), 1);
}

#[test]
fn metadata_changes_invalidate_completion_hash() {
    let original = record();
    let mut changed = original.clone();
    changed.participants.push("Someone else".into());
    assert_ne!(original.content_hash(), changed.content_hash());
    changed = original.clone();
    changed.entities.clear();
    assert_ne!(original.content_hash(), changed.content_hash());
    changed = original.clone();
    changed.tags.clear();
    assert_ne!(original.content_hash(), changed.content_hash());
}

#[test]
fn legacy_vault_ids_are_retired_from_current_state_but_remain_resolvable() {
    let temp = TempStore::new();
    let parquet = ParquetStore::new(&temp.0);
    let mut old = record();
    old.id = "vault-task-test".into();
    old.url = "/vault/Tasks/Test.md".into();
    parquet.write(std::slice::from_ref(&old)).unwrap();
    let mut new = old.clone();
    new.id = "vault-Tasks/Test.md".into();
    parquet.write(std::slice::from_ref(&new)).unwrap();
    let current = parquet.read_current().unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, new.id);
    assert_eq!(parquet.get_by_ids(&[&old.id]).unwrap().len(), 1);
}
