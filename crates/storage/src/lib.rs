//! Lokale Datenhaltung: SQLite (WAL), Migrationen, Geräte-ID, Änderungsprotokoll, Backups.
//!
//! Schema ist sync-tauglich (siehe `plan/spezifikation/03-datenhaltung.md`): UUIDs statt
//! Autoincrement, pro Zeile `updated_at`/`version`/`device_id`, Soft-Delete über `deleted_at`,
//! jede Änderung landet zusätzlich in `changes`.

mod backup;
mod schema;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub use backup::{BackupInfo, create_backup, list_backups, prune_backups};
pub use rusqlite;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub type Result<T> = rusqlite::Result<T>;

pub struct Store {
    conn: Connection,
    device_id: String,
}

/// Art einer protokollierten Änderung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOp {
    Upsert,
    Delete,
    Purge,
}

impl ChangeOp {
    fn as_str(self) -> &'static str {
        match self {
            ChangeOp::Upsert => "upsert",
            ChangeOp::Delete => "delete",
            ChangeOp::Purge => "purge",
        }
    }
}

impl Store {
    /// Öffnet (oder erstellt) die Datenbank und bringt das Schema auf den neuesten Stand.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            // Fehler zeigt sich beim Öffnen ohnehin mit besserer Meldung.
            let _ = std::fs::create_dir_all(parent);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // WAL: Absturz beschädigt die DB nicht, Leser blockieren Schreiber nicht.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        let device_id = match conn
            .query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |row| row.get(0))
            .optional()?
        {
            Some(id) => id,
            None => {
                let id = new_id();
                conn.execute("INSERT INTO meta(key, value) VALUES ('device_id', ?1)", params![id])?;
                id
            }
        };
        Ok(Self { conn, device_id })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        self.conn.transaction()
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn.pragma_query_value(None, "user_version", |row| row.get(0))
    }

    /// Konsistente Kopie der laufenden Datenbank (SQLite-Backup-API, auch während Schreibzugriffen).
    pub fn backup_to(&self, target: &Path) -> Result<()> {
        let mut dest = Connection::open(target)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest)?;
        backup.run_to_completion(256, std::time::Duration::from_millis(5), None)?;
        Ok(())
    }
}

/// Trägt eine Änderung ins Protokoll ein (Grundlage für Sync).
pub fn record_change(
    tx: &Transaction,
    device_id: &str,
    entity: &str,
    entity_id: &str,
    op: ChangeOp,
    version: i64,
) -> Result<()> {
    tx.execute(
        "INSERT INTO changes(entity, entity_id, op, version, device_id, changed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![entity, entity_id, op.as_str(), version, device_id, now_ms()],
    )?;
    Ok(())
}

/// Millisekunden seit 1970 (UTC).
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Neue, global eindeutige ID (UUID v4).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_migrates_and_keeps_device_id() {
        let dir = std::env::temp_dir().join(format!("devmark-storage-{}", new_id()));
        let path = dir.join("test.sqlite");
        let first = Store::open(&path).unwrap();
        let device = first.device_id().to_string();
        assert_eq!(first.schema_version().unwrap(), schema::LATEST);
        let mode: String = first.conn().pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
        drop(first);
        let second = Store::open(&path).unwrap();
        assert_eq!(second.device_id(), device);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn fts5_is_available() {
        let store = Store::open_in_memory().unwrap();
        let n: i64 = store
            .conn()
            .query_row("SELECT count(*) FROM documents_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
