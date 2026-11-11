//! Einmalige Übernahme der Notizen aus der früheren Datenbank (Notizen nur in SQLite,
//! Verknüpfungen als `[[Titel|id]]`) in den Vault. Die alte Datenbank bleibt unverändert
//! liegen, nur ein Merker verhindert eine zweite Übernahme.

use std::collections::HashMap;
use std::path::Path;

use devmark_storage::Store;
use rusqlite::{OptionalExtension, params};

use crate::{Notes, Result, display_title, file_stem, link_markup};

const MIGRATED_KEY: &str = "migrated_to_vault";

struct LegacyDocument {
    id: String,
    folder_id: Option<String>,
    title: String,
    content: String,
    typst: bool,
    favorite: bool,
    trashed: bool,
}

/// Übernimmt alle Notizen aus `legacy` (falls vorhanden und noch nicht übernommen).
/// Ordner werden zu Unterordnern, Typst-Dokumente bleiben `.typ`-Dateien, Papierkorb geht
/// nach `.trash/`, Verlauf und Favoriten kommen mit. Gibt die Anzahl der Notizen zurück.
pub fn import_legacy(legacy: &Path, notes: &mut Notes) -> Result<usize> {
    if !legacy.exists() {
        return Ok(0);
    }
    let store = Store::open(legacy)?;
    let conn = store.conn();
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![MIGRATED_KEY],
            |r| r.get(0),
        )
        .optional()?;
    if done.is_some() {
        return Ok(0);
    }
    let folders: HashMap<String, (Option<String>, String)> = {
        let mut stmt =
            conn.prepare("SELECT id, parent_id, name FROM folders WHERE deleted_at IS NULL")?;
        stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<rusqlite::Result<_>>()?
    };
    let documents: Vec<LegacyDocument> = {
        let mut stmt =
            conn.prepare("SELECT id, folder_id, title, content, format, favorite, deleted_at FROM documents ORDER BY created_at")?;
        stmt.query_map([], |r| {
            Ok(LegacyDocument {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                content: r.get(3)?,
                typst: r.get::<_, String>(4)? == "typst",
                favorite: r.get::<_, i64>(5)? != 0,
                trashed: r.get::<_, Option<i64>>(6)?.is_some(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?
    };

    // Erst alle Ziele festlegen, damit Links auf später geschriebene Notizen passen.
    let mut targets: HashMap<String, String> = HashMap::new();
    let mut taken = std::collections::HashSet::new();
    for document in &documents {
        let mut dir = folder_path(&folders, document.folder_id.as_deref());
        if document.trashed {
            dir.insert(0, ".trash".into());
        }
        let dir = (!dir.is_empty()).then(|| dir.join("/"));
        let stem = file_stem(&display_title(&document.title, &document.content));
        let extension = if document.typst { "typ" } else { "md" };
        let id = (1..)
            .map(|n| {
                let name = if n == 1 {
                    format!("{stem}.{extension}")
                } else {
                    format!("{stem} {n}.{extension}")
                };
                crate::join(dir.as_deref(), &name)
            })
            .find(|id| !taken.contains(id) && !notes.path_of(id).exists())
            .expect("unendlich viele Namen");
        taken.insert(id.clone());
        targets.insert(document.id.clone(), id);
    }
    let by_title: HashMap<String, &str> = documents
        .iter()
        .map(|d| {
            (
                display_title(&d.title, &d.content).to_lowercase(),
                targets[&d.id].as_str(),
            )
        })
        .collect();

    for document in &documents {
        let id = &targets[&document.id];
        let content = if document.typst {
            document.content.clone()
        } else {
            convert_links(&document.content, id, &targets, &by_title)
        };
        let path = notes.path_of(id);
        std::fs::create_dir_all(path.parent().expect("im Vault"))?;
        std::fs::write(&path, content)?;
    }
    notes.rescan()?;

    for document in &documents {
        let id = &targets[&document.id];
        if document.favorite {
            notes.set_favorite(id, true)?;
        }
        let mut stmt = conn.prepare(
            "SELECT id, title, content, created_at FROM revisions WHERE document_id = ?1",
        )?;
        let revisions: Vec<(String, String, String, i64)> = stmt
            .query_map(params![document.id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        if notes.get(id)?.is_some() {
            for (rev_id, title, content, created_at) in revisions {
                notes.store.conn().execute(
                    "INSERT OR IGNORE INTO revisions(id, document_id, title, content, created_at, device_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![rev_id, id, title, content, created_at, notes.store.device_id()],
                )?;
            }
        }
    }
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![MIGRATED_KEY, notes.root().to_string_lossy()],
    )?;
    Ok(documents.len())
}

/// Ordnerpfad (Teile) aus der früheren Ordnertabelle.
fn folder_path(
    folders: &HashMap<String, (Option<String>, String)>,
    id: Option<&str>,
) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = id.map(str::to_string);
    // Zyklen (fehlerhafte Daten) dürfen nicht hängen.
    for _ in 0..64 {
        let Some((parent, name)) = current.as_deref().and_then(|id| folders.get(id)) else {
            break;
        };
        parts.push(file_stem(name));
        current = parent.clone();
    }
    parts.reverse();
    parts
}

/// `[[Titel|id]]` / `[[Titel]]` → `[Titel](relativer/pfad.md)`; Unauflösbares bleibt stehen.
fn convert_links(
    content: &str,
    source: &str,
    targets: &HashMap<String, String>,
    by_title: &HashMap<String, &str>,
) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        let Some(len) = rest[start + 2..].find("]]") else {
            break;
        };
        let inner = &rest[start + 2..start + 2 + len];
        let (title, id) = match inner.rsplit_once('|') {
            Some((title, id)) => (title.trim(), Some(id.trim())),
            None => (inner.trim(), None),
        };
        let target = id
            .and_then(|id| targets.get(id).map(String::as_str))
            .or_else(|| by_title.get(&title.to_lowercase()).copied())
            .filter(|target| target.ends_with(".md") && !target.starts_with(".trash/"));
        out.push_str(&rest[..start]);
        match target {
            Some(target) if !inner.contains('\n') => {
                out.push_str(&link_markup(title, source, target))
            }
            _ => out.push_str(&rest[start..start + 4 + len]),
        }
        rest = &rest[start + 4 + len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::vault;
    use devmark_storage::{new_id, now_ms};

    #[test]
    fn imports_notes_folders_links_and_trash_once() {
        let dir = std::env::temp_dir().join(format!("devmark-legacy-{}", new_id()));
        let legacy = dir.join("old.sqlite");
        {
            let store = Store::open(&legacy).unwrap();
            let c = store.conn();
            let d = store.device_id();
            let now = now_ms();
            c.execute("INSERT INTO folders(id, name, created_at, updated_at, device_id) VALUES ('f1', 'Arbeit', ?1, ?1, ?2)", params![now, d]).unwrap();
            for (id, folder, title, content, format, fav, deleted) in [
                ("a", Some("f1"), "", "# Ziel\nText", "markdown", 1, None),
                (
                    "b",
                    None,
                    "Quelle",
                    "Siehe [[Ziel|a]] und [[ziel]] und [[Fehlt]]",
                    "markdown",
                    0,
                    None,
                ),
                ("c", None, "Satz", "= Typst", "typst", 0, None),
                ("d", None, "Alt", "weg", "markdown", 0, Some(now)),
            ] {
                c.execute(
                    "INSERT INTO documents(id, folder_id, title, content, format, favorite, created_at, updated_at, deleted_at, device_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9)",
                    params![id, folder, title, content, format, fav, now, deleted, d],
                )
                .unwrap();
            }
            c.execute("INSERT INTO revisions(id, document_id, title, content, created_at, device_id) VALUES ('r1', 'a', '', 'alt', ?1, ?2)", params![now, d]).unwrap();
        }
        let (mut notes, vault_dir) = vault();
        assert_eq!(import_legacy(&legacy, &mut notes).unwrap(), 4);
        assert_eq!(
            std::fs::read_to_string(vault_dir.join("Quelle.md")).unwrap(),
            "Siehe [Ziel](Arbeit/Ziel.md) und [ziel](Arbeit/Ziel.md) und [[Fehlt]]"
        );
        assert!(vault_dir.join("Satz.typ").exists());
        assert!(vault_dir.join(".trash/Alt.md").exists());
        let target = notes.get("Arbeit/Ziel.md").unwrap().unwrap();
        assert!(target.favorite);
        assert_eq!(notes.revisions(&target.id).unwrap()[0].content, "alt");
        assert_eq!(import_legacy(&legacy, &mut notes).unwrap(), 0, "nur einmal");
        std::fs::remove_dir_all(dir).ok();
        std::fs::remove_dir_all(vault_dir).ok();
    }
}
