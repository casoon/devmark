//! Transporte: wo die Sync-Objekte liegen. Die App kennt nur den Trait.

use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Ein gespeichertes Objekt; `etag` ändert sich bei jedem Schreiben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteObject {
    pub key: String,
    pub etag: String,
}

/// Ablage für Sync-Objekte (Schlüssel wie `documents/<id>.json`).
pub trait SyncTransport {
    fn list(&self, prefix: &str) -> io::Result<Vec<RemoteObject>>;
    fn get(&self, key: &str) -> io::Result<Option<Vec<u8>>>;
    /// Schreibt atomar und liefert den neuen `etag`.
    fn put(&self, key: &str, bytes: &[u8]) -> io::Result<String>;
    fn delete(&self, key: &str) -> io::Result<()>;
}

/// Ordner, der von einem Dateisync-Dienst (iCloud Drive, Nextcloud, Dropbox, Syncthing …)
/// verteilt wird. Die Datenbank selbst liegt nie dort – nur einzelne JSON-Objekte.
pub struct FolderTransport {
    root: PathBuf,
}

impl FolderTransport {
    pub fn new(folder: &Path) -> Self {
        Self { root: folder.join("devmark-sync").join("v1") }
    }

    fn path(&self, key: &str) -> io::Result<PathBuf> {
        // Schlüssel kommen aus der eigenen Datenbank; trotzdem nie aus dem Ordner ausbrechen.
        if key.split('/').any(|part| part.is_empty() || part == "." || part == "..") || key.contains('\\') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("ungültiger Schlüssel {key:?}")));
        }
        Ok(self.root.join(key))
    }
}

fn etag(path: &Path) -> io::Result<String> {
    let meta = std::fs::metadata(path)?;
    let modified = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(format!("{modified}-{}", meta.len()))
}

impl SyncTransport for FolderTransport {
    fn list(&self, prefix: &str) -> io::Result<Vec<RemoteObject>> {
        let dir = self.path(prefix)?;
        let mut objects = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(objects),
            Err(err) => return Err(err),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Nur fertige Objekte; Zwischenstände und Konfliktdateien des Sync-Dienstes ignorieren.
            if !name.ends_with(".json") || name.starts_with('.') || name.contains(' ') {
                continue;
            }
            objects.push(RemoteObject { key: format!("{prefix}/{name}"), etag: etag(&entry.path())? });
        }
        objects.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(objects)
    }

    fn get(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(self.path(key)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn put(&self, key: &str, bytes: &[u8]) -> io::Result<String> {
        let path = self.path(key)?;
        let dir = path.parent().expect("key has a directory");
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{}.tmp", devmark_storage::new_id()));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
        etag(&path)
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        match std::fs::remove_file(self.path(key)?) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_roundtrip() {
        let dir = std::env::temp_dir().join(format!("devmark-transport-{}", devmark_storage::new_id()));
        let t = FolderTransport::new(&dir);
        assert!(t.list("documents").unwrap().is_empty());
        let tag = t.put("documents/a.json", b"{}").unwrap();
        assert_eq!(t.list("documents").unwrap(), [RemoteObject { key: "documents/a.json".into(), etag: tag }]);
        assert_eq!(t.get("documents/a.json").unwrap().unwrap(), b"{}");
        t.delete("documents/a.json").unwrap();
        assert!(t.get("documents/a.json").unwrap().is_none());
        assert!(t.put("../x.json", b"").is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
