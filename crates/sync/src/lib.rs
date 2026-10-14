//! Synchronisierung über austauschbare Transporte – ohne eigenen Server.
//!
//! Jeder Datensatz (Ordner, Tag, Dokument, Dokument-Tag) liegt als JSON-Objekt im Transport.
//! Ein Lauf holt zuerst fremde Änderungen (Pull), dann eigene (Push). Grundlage ist
//! `sync_state`: Hash des Datensatzes und `etag` des Objekts beim letzten Abgleich.
//!
//! Konflikte (beide Seiten seit dem letzten Abgleich geändert):
//! - Dokumente: nie still überschreiben – der fremde Stand wird als Konfliktkopie angelegt,
//!   der eigene gewinnt im Objekt. Eine fremde Löschung verliert gegen eigene Änderungen.
//! - Ordner, Tags, Zuordnungen: der jüngere Stand (`updated_at`) gewinnt.
//!
//! Endgültig gelöschte Datensätze verschwinden als Objekt; Geräte, die den Datensatz
//! unverändert haben, löschen ihn dann ebenfalls.

pub mod transport;

use std::collections::HashMap;
use std::fmt;

use devmark_storage::{Store, new_id, now_ms};
use rusqlite::{OptionalExtension, params, params_from_iter, types::ValueRef};
use serde_json::Value;

pub use transport::{FolderTransport, RemoteObject, SyncTransport};

struct Entity {
    /// Name im Transport und in `sync_state`.
    name: &'static str,
    table: &'static str,
    key: &'static [&'static str],
    columns: &'static [&'static str],
    /// Spalte für „jünger gewinnt“.
    updated: &'static str,
}

/// Reihenfolge wegen Fremdschlüsseln: erst Ordner und Tags, dann Dokumente, dann Zuordnungen.
const ENTITIES: &[Entity] = &[
    Entity {
        name: "folders",
        table: "folders",
        key: &["id"],
        columns: &["id", "parent_id", "name", "position", "created_at", "updated_at", "deleted_at", "version", "device_id"],
        updated: "updated_at",
    },
    Entity {
        name: "tags",
        table: "tags",
        key: &["id"],
        columns: &["id", "name", "created_at", "updated_at", "deleted_at", "version", "device_id"],
        updated: "updated_at",
    },
    Entity {
        name: "documents",
        table: "documents",
        key: &["id"],
        columns: &[
            "id", "kind", "format", "parent_id", "folder_id", "title", "content", "favorite", "position",
            "created_at", "updated_at", "deleted_at", "version", "device_id",
        ],
        updated: "updated_at",
    },
    Entity {
        name: "document_tags",
        table: "document_tags",
        key: &["document_id", "tag_id"],
        columns: &["document_id", "tag_id", "created_at", "deleted_at", "version", "device_id"],
        updated: "created_at",
    },
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub pulled: usize,
    pub pushed: usize,
    pub deleted_local: usize,
    pub deleted_remote: usize,
    /// Titel der angelegten Konfliktkopien.
    pub conflicts: Vec<String>,
    /// IDs der Dokumente, die lokal geändert oder gelöscht wurden (offene Editoren neu laden).
    pub changed_documents: Vec<String>,
}

impl SyncReport {
    pub fn is_empty(&self) -> bool {
        self.pulled == 0 && self.pushed == 0 && self.deleted_local == 0 && self.deleted_remote == 0
    }
}

#[derive(Debug)]
pub enum SyncError {
    Db(rusqlite::Error),
    Io(std::io::Error),
    Format(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SyncError::Db(err) => write!(f, "Datenbank: {err}"),
            SyncError::Io(err) => write!(f, "Sync-Ordner: {err}"),
            SyncError::Format(err) => write!(f, "Ungültiges Sync-Objekt: {err}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<rusqlite::Error> for SyncError {
    fn from(err: rusqlite::Error) -> Self {
        SyncError::Db(err)
    }
}

impl From<std::io::Error> for SyncError {
    fn from(err: std::io::Error) -> Self {
        SyncError::Io(err)
    }
}

type Row = Vec<Value>;

struct Local {
    row: Row,
    hash: String,
}

struct State {
    hash: String,
    etag: String,
}

/// Ein vollständiger Abgleich mit dem Transport.
pub fn sync(store: &mut Store, transport: &dyn SyncTransport) -> Result<SyncReport, SyncError> {
    let mut report = SyncReport::default();
    // Pull in Abhängigkeitsreihenfolge, lokale Löschungen umgekehrt.
    let mut pending_local_deletes: Vec<(&Entity, String)> = Vec::new();
    for entity in ENTITIES {
        pull(store, transport, entity, &mut report, &mut pending_local_deletes)?;
    }
    for (entity, key) in pending_local_deletes.iter().rev() {
        delete_local(store, entity, key)?;
        report.deleted_local += 1;
        if entity.name == "documents" {
            report.changed_documents.push(key.clone());
        }
    }
    for entity in ENTITIES {
        push(store, transport, entity, &mut report)?;
    }
    Ok(report)
}

fn pull(
    store: &mut Store,
    transport: &dyn SyncTransport,
    entity: &'static Entity,
    report: &mut SyncReport,
    pending_local_deletes: &mut Vec<(&'static Entity, String)>,
) -> Result<(), SyncError> {
    let remote = transport.list(entity.name)?;
    let local = local_rows(store, entity)?;
    let mut states = states(store, entity)?;
    let device = store.device_id().to_string();

    let tx = store.transaction()?;
    tx.pragma_update(None, "defer_foreign_keys", "ON")?;
    let mut seen = Vec::with_capacity(remote.len());
    for object in &remote {
        let Some(key) = object.key.strip_prefix(entity.name).and_then(|k| k.strip_prefix('/')).and_then(|k| k.strip_suffix(".json")).map(decode_key) else {
            continue;
        };
        seen.push(key.clone());
        let state = states.get(&key);
        if state.is_some_and(|s| s.etag == object.etag) {
            continue;
        }
        let Some(bytes) = transport.get(&object.key)? else {
            continue; // zwischen list und get gelöscht – beim nächsten Lauf
        };
        let remote_row = parse_row(entity, &bytes)?;
        let remote_hash = hash_row(&remote_row);
        let local_row = local.get(&key);
        let local_changed = match (local_row, state) {
            (Some(l), Some(s)) => l.hash != s.hash,
            (Some(_), None) => true,
            (None, _) => false,
        };
        let set_state = |tx: &rusqlite::Transaction, hash: &str| -> rusqlite::Result<()> {
            tx.execute(
                "INSERT INTO sync_state(entity, entity_id, synced_hash, remote_etag) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(entity, entity_id) DO UPDATE SET synced_hash = excluded.synced_hash, remote_etag = excluded.remote_etag",
                params![entity.name, key, hash, object.etag],
            )?;
            Ok(())
        };

        if local_row.is_some_and(|l| l.hash == remote_hash) {
            set_state(&tx, &remote_hash)?;
        } else if !local_changed {
            apply_row(&tx, entity, &remote_row)?;
            set_state(&tx, &remote_hash)?;
            report.pulled += 1;
            if entity.name == "documents" {
                report.changed_documents.push(key.clone());
            }
        } else if entity.name == "documents" {
            // Beide geändert: fremden Stand als Kopie behalten, eigener wird gleich hochgeladen.
            let deleted_remotely = !value_is_null(&remote_row, entity, "deleted_at");
            if !deleted_remotely {
                let title = conflict_copy(&tx, &remote_row, &device)?;
                report.conflicts.push(title);
            }
            set_state(&tx, &remote_hash)?;
        } else {
            let remote_newer = int_value(&remote_row, entity, entity.updated)
                > int_value(&local_row.expect("local_changed implies row").row, entity, entity.updated);
            if remote_newer {
                apply_row(&tx, entity, &remote_row)?;
                report.pulled += 1;
            }
            // Ältere lokale Fassung: Zustand auf den fremden Stand → Push überschreibt.
            set_state(&tx, &remote_hash)?;
        }
    }
    tx.commit()?;

    // Fremd endgültig gelöscht: Objekt fehlt, Zustand existiert.
    states.retain(|key, _| !seen.contains(key));
    for (key, state) in states {
        match local.get(&key) {
            Some(l) if l.hash == state.hash => pending_local_deletes.push((entity, key.clone())),
            _ => {}
        }
        store.conn().execute(
            "DELETE FROM sync_state WHERE entity = ?1 AND entity_id = ?2",
            params![entity.name, key],
        )?;
    }
    Ok(())
}

fn push(store: &mut Store, transport: &dyn SyncTransport, entity: &Entity, report: &mut SyncReport) -> Result<(), SyncError> {
    let local = local_rows(store, entity)?;
    let states = states(store, entity)?;
    for (key, row) in &local {
        if states.get(key).is_some_and(|s| s.hash == row.hash) {
            continue;
        }
        let object = serde_json::json!({ "entity": entity.name, "row": row_to_object(entity, &row.row) });
        let bytes = serde_json::to_vec_pretty(&object).map_err(|e| SyncError::Format(e.to_string()))?;
        let etag = transport.put(&object_key(entity, key), &bytes)?;
        store.conn().execute(
            "INSERT INTO sync_state(entity, entity_id, synced_hash, remote_etag) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(entity, entity_id) DO UPDATE SET synced_hash = excluded.synced_hash, remote_etag = excluded.remote_etag",
            params![entity.name, key, row.hash, etag],
        )?;
        report.pushed += 1;
    }
    // Lokal endgültig gelöscht → Objekt entfernen.
    for (key, _) in states.iter().filter(|(key, _)| !local.contains_key(*key)) {
        transport.delete(&object_key(entity, key))?;
        store.conn().execute(
            "DELETE FROM sync_state WHERE entity = ?1 AND entity_id = ?2",
            params![entity.name, key],
        )?;
        report.deleted_remote += 1;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Datenbankzugriffe

fn local_rows(store: &Store, entity: &Entity) -> Result<HashMap<String, Local>, SyncError> {
    let sql = format!("SELECT {} FROM {}", entity.columns.join(", "), entity.table);
    let mut stmt = store.conn().prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut out = HashMap::new();
    while let Some(r) = rows.next()? {
        let mut row = Vec::with_capacity(entity.columns.len());
        for ix in 0..entity.columns.len() {
            row.push(match r.get_ref(ix)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(i) => Value::from(i),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).to_string()),
                ValueRef::Blob(_) => return Err(SyncError::Format("Binärdaten werden nicht synchronisiert".into())),
            });
        }
        let key = row_key(entity, &row);
        let hash = hash_row(&row);
        out.insert(key, Local { row, hash });
    }
    Ok(out)
}

fn states(store: &Store, entity: &Entity) -> Result<HashMap<String, State>, SyncError> {
    let mut stmt = store
        .conn()
        .prepare("SELECT entity_id, synced_hash, remote_etag FROM sync_state WHERE entity = ?1")?;
    let rows = stmt.query_map(params![entity.name], |r| {
        Ok((r.get::<_, String>(0)?, State { hash: r.get(1)?, etag: r.get(2)? }))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn apply_row(tx: &rusqlite::Transaction, entity: &Entity, row: &Row) -> rusqlite::Result<()> {
    let placeholders: Vec<String> = (1..=entity.columns.len()).map(|i| format!("?{i}")).collect();
    let updates: Vec<String> = entity
        .columns
        .iter()
        .filter(|c| !entity.key.contains(c))
        .map(|c| format!("{c} = excluded.{c}"))
        .collect();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT({}) DO UPDATE SET {}",
        entity.table,
        entity.columns.join(", "),
        placeholders.join(", "),
        entity.key.join(", "),
        updates.join(", ")
    );
    tx.execute(&sql, params_from_iter(row.iter().map(json_to_sql)))?;
    Ok(())
}

fn json_to_sql(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => rusqlite::types::Value::Integer(i),
            None => rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        other => rusqlite::types::Value::Text(other.to_string()),
    }
}

/// Fremden Dokumentstand als eigenes, neues Dokument ablegen.
fn conflict_copy(tx: &rusqlite::Transaction, remote: &Row, device: &str) -> rusqlite::Result<String> {
    let entity = &ENTITIES[2];
    let get = |column: &str| remote[entity.columns.iter().position(|c| *c == column).expect("column")].clone();
    let other_device: String = get("device_id").as_str().unwrap_or("").chars().take(8).collect();
    let base_title = get("title").as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).unwrap_or_else(|| {
        get("content")
            .as_str()
            .and_then(|c| c.lines().map(|l| l.trim().trim_start_matches(['#', '=']).trim()).find(|l| !l.is_empty()).map(str::to_string))
            .unwrap_or_else(|| "Notiz".into())
    });
    let title = format!("{base_title} (Konflikt {other_device})");
    let now = now_ms();
    tx.execute(
        "INSERT INTO documents(id, kind, format, parent_id, folder_id, title, content, favorite, created_at, updated_at, device_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8, ?9)",
        params![
            new_id(),
            json_to_sql(&get("kind")),
            json_to_sql(&get("format")),
            json_to_sql(&get("parent_id")),
            json_to_sql(&get("folder_id")),
            title,
            json_to_sql(&get("content")),
            now,
            device,
        ],
    )?;
    Ok(title)
}

fn delete_local(store: &mut Store, entity: &Entity, key: &str) -> Result<(), SyncError> {
    let tx = store.transaction()?;
    match entity.name {
        "documents" => {
            tx.execute("UPDATE documents SET parent_id = NULL WHERE parent_id = ?1", params![key])?;
            tx.execute("DELETE FROM revisions WHERE document_id = ?1", params![key])?;
            tx.execute("DELETE FROM document_tags WHERE document_id = ?1", params![key])?;
            tx.execute("DELETE FROM attachments WHERE document_id = ?1", params![key])?;
            tx.execute("DELETE FROM documents WHERE id = ?1", params![key])?;
        }
        "folders" => {
            tx.execute("UPDATE documents SET folder_id = NULL WHERE folder_id = ?1", params![key])?;
            tx.execute("UPDATE folders SET parent_id = NULL WHERE parent_id = ?1", params![key])?;
            tx.execute("DELETE FROM folders WHERE id = ?1", params![key])?;
        }
        "tags" => {
            tx.execute("DELETE FROM document_tags WHERE tag_id = ?1", params![key])?;
            tx.execute("DELETE FROM tags WHERE id = ?1", params![key])?;
        }
        _ => {
            if let Some((document, tag)) = key.split_once('\u{1f}') {
                tx.execute("DELETE FROM document_tags WHERE document_id = ?1 AND tag_id = ?2", params![document, tag])?;
            }
        }
    }
    // Geänderte Datensätze nicht erneut hochladen, gelöschte Zustände entfernen.
    tx.execute("DELETE FROM sync_state WHERE entity = ?1 AND entity_id = ?2", params![entity.name, key])?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Objekte, Schlüssel, Hash

fn row_key(entity: &Entity, row: &Row) -> String {
    entity
        .key
        .iter()
        .map(|k| {
            let ix = entity.columns.iter().position(|c| c == k).expect("key column");
            row[ix].as_str().map(str::to_string).unwrap_or_else(|| row[ix].to_string())
        })
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn row_to_object(entity: &Entity, row: &Row) -> Value {
    let mut map = serde_json::Map::new();
    for (column, value) in entity.columns.iter().zip(row) {
        map.insert((*column).to_string(), value.clone());
    }
    Value::Object(map)
}

fn parse_row(entity: &Entity, bytes: &[u8]) -> Result<Row, SyncError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| SyncError::Format(e.to_string()))?;
    let row = value
        .get("row")
        .and_then(Value::as_object)
        .ok_or_else(|| SyncError::Format("Feld „row“ fehlt".into()))?;
    for key in entity.key {
        if !row.get(*key).is_some_and(Value::is_string) {
            return Err(SyncError::Format(format!("Schlüssel „{key}“ fehlt")));
        }
    }
    Ok(entity.columns.iter().map(|c| row.get(*c).cloned().unwrap_or(Value::Null)).collect())
}

fn value_is_null(row: &Row, entity: &Entity, column: &str) -> bool {
    entity.columns.iter().position(|c| *c == column).is_none_or(|ix| row[ix].is_null())
}

fn int_value(row: &Row, entity: &Entity, column: &str) -> i64 {
    entity
        .columns
        .iter()
        .position(|c| *c == column)
        .and_then(|ix| row[ix].as_i64())
        .unwrap_or(0)
}

/// FNV-1a über die kanonische JSON-Form (stabil über Versionen und Geräte).
fn hash_row(row: &Row) -> String {
    let text = serde_json::to_string(row).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Dateinamensicher: `[A-Za-z0-9-_]` bleibt, alles andere wird `~XX`.
fn encode_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for byte in key.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("~{byte:02x}"));
        }
    }
    out
}

fn decode_key(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'~' && i + 2 < bytes.len()
            && let Some(byte) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(byte);
                i += 3;
                continue;
            }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn object_key(entity: &Entity, key: &str) -> String {
    format!("{}/{}.json", entity.name, encode_key(key))
}

/// Letzter erfolgreicher Sync (ms) – für die Anzeige.
pub fn last_sync(store: &Store) -> Option<i64> {
    store
        .conn()
        .query_row("SELECT value FROM meta WHERE key = 'last_sync'", [], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
}

pub fn set_last_sync(store: &Store, ms: i64) -> rusqlite::Result<()> {
    store.conn().execute(
        "INSERT INTO meta(key, value) VALUES ('last_sync', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![ms.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
