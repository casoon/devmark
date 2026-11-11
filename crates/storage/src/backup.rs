//! Backup-Generationen: konsistente Kopien über die SQLite-Backup-API, älteste werden entfernt.
//! Sync ist kein Backup – das hier läuft unabhängig davon.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Store;

const PREFIX: &str = "devmark-";
const SUFFIX: &str = ".sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    pub path: PathBuf,
    /// Zeitstempel aus dem Dateinamen (`JJJJMMTT-hhmmss`, UTC).
    pub stamp: String,
    pub size: u64,
}

/// Legt eine neue Generation in `dir` an und behält höchstens `keep` Generationen.
pub fn create_backup(store: &Store, dir: &Path, keep: usize) -> std::io::Result<BackupInfo> {
    std::fs::create_dir_all(dir)?;
    let stamp = utc_stamp(SystemTime::now());
    let mut path = dir.join(format!("{PREFIX}{stamp}{SUFFIX}"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{PREFIX}{stamp}-{n}{SUFFIX}"));
        n += 1;
    }
    // Erst in temporäre Datei, dann umbenennen: halbe Backups gibt es nie.
    let partial = path.with_extension("partial");
    store
        .backup_to(&partial)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    std::fs::rename(&partial, &path)?;
    prune(dir, keep, Some(&path))?;
    let size = std::fs::metadata(&path)?.len();
    Ok(BackupInfo { path, stamp, size })
}

/// Vorhandene Generationen, neueste zuerst.
pub fn list_backups(dir: &Path) -> std::io::Result<Vec<BackupInfo>> {
    let mut backups = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(backups);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stamp) = name
            .strip_prefix(PREFIX)
            .and_then(|s| s.strip_suffix(SUFFIX))
        else {
            continue;
        };
        backups.push(BackupInfo {
            path: entry.path(),
            stamp: stamp.to_string(),
            size: entry.metadata().map(|m| m.len()).unwrap_or(0),
        });
    }
    backups.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    Ok(backups)
}

/// Löscht alle Generationen außer den `keep` neuesten (mindestens eine bleibt).
pub fn prune_backups(dir: &Path, keep: usize) -> std::io::Result<usize> {
    prune(dir, keep, None)
}

/// Wie `prune_backups`; `newest` (gerade erstellt) bleibt in jedem Fall erhalten.
fn prune(dir: &Path, keep: usize, newest: Option<&Path>) -> std::io::Result<usize> {
    let mut backups = list_backups(dir)?;
    if let Some(newest) = newest
        && let Some(ix) = backups.iter().position(|b| b.path == newest)
    {
        let current = backups.remove(ix);
        backups.insert(0, current);
    }
    let mut removed = 0;
    for backup in backups.iter().skip(keep.max(1)) {
        std::fs::remove_file(&backup.path)?;
        removed += 1;
    }
    Ok(removed)
}

fn utc_stamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamps() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "19700101-000000");
        assert_eq!(
            utc_stamp(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
            "20261014-174640"
        );
    }

    #[test]
    fn creates_and_prunes_generations() {
        let dir = std::env::temp_dir().join(format!("devmark-backup-{}", crate::new_id()));
        let store = Store::open_in_memory().unwrap();
        store
            .conn()
            .execute("INSERT INTO meta(key, value) VALUES ('probe', 'x')", [])
            .unwrap();
        for _ in 0..4 {
            create_backup(&store, &dir, 2).unwrap();
        }
        let backups = list_backups(&dir).unwrap();
        assert_eq!(backups.len(), 2);
        // Backup ist eine vollständige, lesbare Datenbank.
        let copy = rusqlite::Connection::open(&backups[0].path).unwrap();
        let value: String = copy
            .query_row("SELECT value FROM meta WHERE key = 'probe'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(value, "x");
        std::fs::remove_dir_all(dir).ok();
    }
}
