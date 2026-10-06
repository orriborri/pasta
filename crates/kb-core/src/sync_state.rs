use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::KbConfig;

type CursorWindow = (String, String, String);

/// SQLite-backed sync state: cursors per channel, content hashes per record, locked model name.
/// Thread-safe via `Mutex` wrapping the connection.
pub struct SyncState {
    conn: Mutex<Connection>,
    staged_windows: Option<Mutex<Vec<CursorWindow>>>,
}

// SAFETY: Connection is only accessed through Mutex, ensuring exclusive access.
unsafe impl Sync for SyncState {}

impl SyncState {
    /// Open (or create) the sync state database.
    ///
    /// # Errors
    /// Returns error if the database cannot be opened or tables cannot be created.
    ///
    /// # Panics
    /// Panics if the state database path has no parent directory.
    pub fn open(config: &KbConfig) -> Result<Self> {
        let path = state_path(config);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let conn = Connection::open(&path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS cursors (
                channel_id TEXT PRIMARY KEY,
                newest_ts TEXT NOT NULL,
                oldest_ts TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS content_hashes (
                record_id TEXT PRIMARY KEY,
                hash TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ")?;
        Ok(Self { conn: Mutex::new(conn), staged_windows: None })
    }

    /// Open fetch state with deferred cursor writes. The caller must first
    /// durably save the fetched records, then call `commit_windows`.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened.
    pub fn open_staged(config: &KbConfig) -> Result<Self> {
        let mut state = Self::open(config)?;
        state.staged_windows = Some(Mutex::new(Vec::new()));
        Ok(state)
    }

    /// Commit staged fetch cursors together, after source records are durable.
    ///
    /// # Errors
    /// Returns an error if the database transaction fails.
    ///
    /// # Panics
    /// Panics if a state mutex is poisoned.
    pub fn commit_windows(&self) -> Result<()> {
        let Some(windows) = &self.staged_windows else { return Ok(()) };
        let mut windows = windows.lock().unwrap();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (channel, oldest, newest) in windows.iter() {
            tx.execute(
                "INSERT INTO cursors (channel_id, newest_ts, oldest_ts) VALUES (?1, ?2, ?3)
                 ON CONFLICT(channel_id) DO UPDATE SET
                   newest_ts = MAX(excluded.newest_ts, cursors.newest_ts),
                   oldest_ts = MIN(excluded.oldest_ts, cursors.oldest_ts)",
                rusqlite::params![channel, newest, oldest],
            )?;
        }
        tx.commit()?;
        drop(conn);
        windows.clear();
        Ok(())
    }

    /// Get the forward cursor for a channel (fetch only newer than this).
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn cursor(&self, channel_id: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT newest_ts FROM cursors WHERE channel_id = ?1",
            [channel_id],
            |row| row.get(0),
        ).ok()
    }

    /// Update the time window for a channel.
    ///
    /// # Errors
    /// Returns error if the database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn update_window(&self, channel_id: &str, oldest_ts: &str, newest_ts: &str) -> Result<()> {
        if let Some(windows) = &self.staged_windows {
            windows.lock().unwrap().push((channel_id.into(), oldest_ts.into(), newest_ts.into()));
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO cursors (channel_id, newest_ts, oldest_ts) VALUES (?1, ?2, ?3)
             ON CONFLICT(channel_id) DO UPDATE SET
               newest_ts = MAX(excluded.newest_ts, cursors.newest_ts),
               oldest_ts = MIN(excluded.oldest_ts, cursors.oldest_ts)",
            rusqlite::params![channel_id, newest_ts, oldest_ts],
        )?;
        drop(conn);
        Ok(())
    }

    /// Get stored content hash for a record (for skip-unchanged logic).
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn get_hash(&self, record_id: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT hash FROM content_hashes WHERE record_id = ?1",
            [record_id],
            |row| row.get(0),
        ).ok()
    }

    /// Store content hash for a record.
    ///
    /// # Errors
    /// Returns error if the database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn set_hash(&self, record_id: &str, hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO content_hashes (record_id, hash) VALUES (?1, ?2)",
            rusqlite::params![record_id, hash],
        )?;
        drop(conn);
        Ok(())
    }

    /// Get the locked model name.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn model_name(&self) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM meta WHERE key = 'model_name'",
            [],
            |row| row.get(0),
        ).ok()
    }

    /// Set the locked model name. Refuses if a different model is already stored.
    ///
    /// # Errors
    /// Returns error if a different model is already locked, or if database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn lock_model(&self, name: &str) -> Result<()> {
        if let Some(existing) = self.model_name() {
            if existing != name {
                anyhow::bail!(
                    "Model mismatch: stored '{existing}', trying to use '{name}'. Run `kb reindex` to switch models."
                );
            }
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('model_name', ?1)",
            [name],
        )?;
        drop(conn);
        Ok(())
    }

    /// Clear model lock (for reindex).
    ///
    /// # Errors
    /// Returns error if database write fails.
    ///
    /// # Panics
    /// Panics if the connection mutex is poisoned.
    pub fn clear_model(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM meta WHERE key = 'model_name'", [])?;
        drop(conn);
        Ok(())
    }
}

fn state_path(config: &KbConfig) -> PathBuf {
    config.data_dir.join("state.db")
}
