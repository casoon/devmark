//! Notizen als Markdown-Dateien in einem Ordner („Vault“): Ordner = Hierarchie, Dateiname =
//! Titel, Verknüpfungen als relative Markdown-Links. Der Ordner ist maßgeblich und ohne
//! DevMark lesbar; SQLite dient nur als Index (Volltextsuche, Tags, Favoriten,
//! Versionsverlauf) und wird bei Bedarf aus dem Ordner nachgezogen.
//!
//! Notiz-IDs sind Pfade relativ zum Vault mit `/` (`Projekte/Idee.md`); der Papierkorb liegt
//! in `.trash/` im Vault.

mod legacy;
mod links;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use devmark_storage::{Store, new_id, now_ms};
use rusqlite::{OptionalExtension, Row, params};

pub use devmark_storage::{BackupInfo, create_backup, list_backups};
pub use legacy::import_legacy;
pub use links::{NoteLink, link_markup, note_links, relative, resolve, stem, strip_note_links};

const TRASH_DIR: &str = ".trash";
/// Name neuer, noch leerer Notizen; solche Dateien folgen der ersten Zeile.
const UNTITLED: &str = "Neue Notiz";

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(err) => err.fmt(f),
            Error::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Db(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Umbenennungen von Notizen (alte ID, neue ID), damit offene Tabs nachziehen können.
pub type Moves = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub id: String,
    /// Ordner relativ zum Vault; `None` = oberste Ebene (oder Papierkorb).
    pub folder_id: Option<String>,
    /// Dateiname ohne `.md`.
    pub title: String,
    pub content: String,
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

impl Document {
    pub fn display_title(&self) -> String {
        self.title.clone()
    }
}

/// Erste Textzeile (ohne Frontmatter und Überschriftszeichen), wie in Apple Notes.
pub fn display_title(title: &str, content: &str) -> String {
    if !title.trim().is_empty() {
        return title.trim().to_string();
    }
    let body = strip_frontmatter(content);
    let line = body
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '=']).trim())
        .find(|l| !l.is_empty() && *l != "---")
        .unwrap_or(UNTITLED);
    line.chars().take(80).collect()
}

fn strip_frontmatter(content: &str) -> &str {
    content
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---").map(|end| &rest[end + 4..]))
        .unwrap_or(content)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    /// Pfad relativ zum Vault.
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Revision {
    pub id: String,
    pub document_id: String,
    pub title: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    /// Ausschnitt mit Treffern zwischen `«` und `»`.
    pub snippet: String,
    /// Ordner der Notiz relativ zum Vault; `None` = oberste Ebene.
    pub folder_id: Option<String>,
    pub updated_at: i64,
}

/// Reihenfolge der Suchtreffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchOrder {
    /// Beste Übereinstimmung zuerst.
    #[default]
    Relevance,
    /// Zuletzt geänderte Notiz zuerst.
    Recent,
}

/// Auswahl für Listen.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// `None` = alle Ordner, `Some(None)` = oberste Ebene, `Some(Some(id))` = dieser Ordner.
    pub folder: Option<Option<String>>,
    pub tag: Option<String>,
    pub favorites_only: bool,
    /// Papierkorb statt aktiver Notizen.
    pub trash: bool,
}

pub struct Notes {
    root: PathBuf,
    store: Store,
    /// Mindestabstand zwischen zwei automatischen Revisionen.
    pub revision_interval_ms: i64,
}

const DOCUMENT_COLUMNS: &str =
    "id, folder_id, title, content, favorite, created_at, updated_at, deleted_at";

fn document_from_row(row: &Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        favorite: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
    })
}

/// Ordner der ID (`a/b/x.md` → `a/b`).
fn parent_of(id: &str) -> Option<String> {
    id.rsplit_once('/').map(|(dir, _)| dir.to_string())
}

fn join(dir: Option<&str>, name: &str) -> String {
    match dir {
        Some(dir) if !dir.is_empty() => format!("{dir}/{name}"),
        _ => name.to_string(),
    }
}

/// Heißt die Datei `stem` oder `stem 2`, `stem 3` … (freie Namen aus [`Notes::free_id`])?
fn follows(title: &str, stem: &str) -> bool {
    title == stem
        || title
            .strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix(' '))
            .is_some_and(|n| n.parse::<u32>().is_ok())
}

/// Dateiname aus einem Titel: ohne Pfad-/Sonderzeichen, nicht leer, höchstens 80 Zeichen.
pub fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        UNTITLED.into()
    } else {
        cleaned
            .chars()
            .take(80)
            .collect::<String>()
            .trim()
            .to_string()
    }
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(now_ms)
}

fn created_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.created())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|| mtime_ms(path))
}

/// Markdown-Dateien und Ordner unter `dir` (ohne versteckte), als IDs relativ zu `root`.
fn walk(root: &Path, dir: &Path, files: &mut Vec<String>, dirs: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let id = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if path.is_dir() {
            dirs.push(id);
            walk(root, &path, files, dirs);
        } else if name.to_lowercase().ends_with(".md") {
            files.push(id);
        }
    }
}

impl Notes {
    /// Öffnet den Vault (wird angelegt) mit Index-Datenbank und gleicht beide ab.
    pub fn open(root: &Path, index: &Path) -> Result<Self> {
        Self::with_store(root, Store::open(index)?)
    }

    pub fn with_store(root: &Path, store: Store) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let mut notes = Self {
            root: root.to_path_buf(),
            store,
            revision_interval_ms: 10 * 60 * 1000,
        };
        notes.rescan()?;
        Ok(notes)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Datei einer Notiz.
    pub fn path_of(&self, id: &str) -> PathBuf {
        id.split('/')
            .fold(self.root.clone(), |path, part| path.join(part))
    }

    /// Freie ID für `stem` in `dir` („Name.md“, „Name 2.md“ …); `keep` gilt als frei.
    fn free_id(&self, dir: Option<&str>, stem: &str, keep: Option<&str>) -> String {
        (1..)
            .map(|n| {
                join(
                    dir,
                    &if n == 1 {
                        format!("{stem}.md")
                    } else {
                        format!("{stem} {n}.md")
                    },
                )
            })
            .find(|id| Some(id.as_str()) == keep || !self.path_of(id).exists())
            .expect("unendlich viele Namen")
    }

    // ---- Abgleich mit dem Ordner ----

    /// Nimmt Änderungen von außen auf (andere Programme, Sync-Dienste). `true`, wenn sich
    /// etwas geändert hat.
    pub fn rescan(&mut self) -> Result<bool> {
        let (mut files, mut dirs) = (Vec::new(), Vec::new());
        walk(&self.root, &self.root, &mut files, &mut dirs);
        let mut trashed = Vec::new();
        walk(
            &self.root,
            &self.root.join(TRASH_DIR),
            &mut trashed,
            &mut dirs,
        );
        let known: HashMap<String, i64> = {
            let mut stmt = self
                .store
                .conn()
                .prepare("SELECT id, updated_at FROM documents")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut changed = false;
        let mut seen = HashSet::new();
        for (id, trash) in files
            .into_iter()
            .map(|id| (id, false))
            .chain(trashed.into_iter().map(|id| (id, true)))
        {
            let path = self.path_of(&id);
            let modified = mtime_ms(&path);
            if known.get(&id) != Some(&modified) {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                self.index(&id, &content, trash.then_some(modified))?;
                changed = true;
            }
            seen.insert(id);
        }
        for id in known.keys().filter(|id| !seen.contains(*id)) {
            self.unindex(id)?;
            changed = true;
        }
        Ok(changed)
    }

    /// Indexeintrag aus dem Dateistand (Favorit und Papierkorb-Zeit bleiben erhalten).
    fn index(&mut self, id: &str, content: &str, deleted_at: Option<i64>) -> Result<()> {
        let path = self.path_of(id);
        let folder = if deleted_at.is_some() {
            None
        } else {
            parent_of(id)
        };
        ensure_folder(self.store.conn(), folder.as_deref(), self.store.device_id())?;
        self.store.conn().execute(
            "INSERT INTO documents(id, folder_id, title, content, created_at, updated_at, deleted_at, device_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET folder_id = excluded.folder_id, title = excluded.title,
               content = excluded.content, updated_at = excluded.updated_at,
               deleted_at = CASE WHEN excluded.deleted_at IS NULL THEN NULL
                                 ELSE coalesce(documents.deleted_at, excluded.deleted_at) END",
            params![
                id,
                folder,
                stem(id),
                content,
                created_ms(&path),
                mtime_ms(&path),
                deleted_at,
                self.store.device_id()
            ],
        )?;
        let mut tags = extract_hashtags(content);
        for tag in frontmatter_tags(content) {
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                tags.push(tag);
            }
        }
        self.set_tags(id, &tags)
    }

    fn unindex(&mut self, id: &str) -> Result<()> {
        let tx = self.store.transaction()?;
        tx.execute("DELETE FROM revisions WHERE document_id = ?1", params![id])?;
        tx.execute(
            "DELETE FROM document_tags WHERE document_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM attachments WHERE document_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    fn write(&mut self, id: &str, content: &str) -> Result<()> {
        let path = self.path_of(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        self.index(id, content, None)
    }

    // ---- Notizen ----

    /// Neue Notiz in `folder`; Dateiname aus `title`, sonst aus der ersten Zeile.
    pub fn create(&mut self, title: &str, content: &str, folder: Option<&str>) -> Result<Document> {
        let id = self.free_id(folder, &file_stem(&display_title(title, content)), None);
        self.write(&id, content)?;
        Ok(self.get(&id)?.expect("just written"))
    }

    /// Schnelle Notiz auf oberster Ebene – „sofort schreiben“.
    pub fn create_note(&mut self, content: &str) -> Result<Document> {
        self.create("", content, None)
    }

    pub fn get(&self, id: &str) -> Result<Option<Document>> {
        Ok(self
            .store
            .conn()
            .query_row(
                &format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id = ?1"),
                params![id],
                document_from_row,
            )
            .optional()?)
    }

    /// Notizen nach Filter; Favoriten zuerst, dann zuletzt geändert.
    pub fn list(&self, filter: &Filter) -> Result<Vec<Document>> {
        let mut sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents d WHERE ");
        sql.push_str(if filter.trash {
            "deleted_at IS NOT NULL"
        } else {
            "deleted_at IS NULL"
        });
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        match &filter.folder {
            Some(None) => sql.push_str(" AND folder_id IS NULL"),
            Some(Some(folder)) => {
                args.push(Box::new(folder.clone()));
                sql.push_str(&format!(" AND folder_id = ?{}", args.len()));
            }
            None => {}
        }
        if let Some(tag) = &filter.tag {
            args.push(Box::new(tag.clone()));
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM document_tags dt JOIN tags t ON t.id = dt.tag_id
                   WHERE dt.document_id = d.id AND t.name = ?{} COLLATE NOCASE)",
                args.len()
            ));
        }
        if filter.favorites_only {
            sql.push_str(" AND favorite = 1");
        }
        sql.push_str(if filter.trash {
            " ORDER BY deleted_at DESC"
        } else {
            " ORDER BY favorite DESC, updated_at DESC"
        });
        let mut stmt = self.store.conn().prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())),
            document_from_row,
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Autosave. Revisionen werden zusammengefasst – höchstens eine je `revision_interval_ms`,
    /// mit dem Stand *vor* der Änderung. Folgt der Dateiname der ersten Zeile (neue Notiz oder
    /// nie anders benannt), wird die Datei mit umbenannt; zurück kommen die Umbenennungen.
    pub fn save_content(&mut self, id: &str, content: &str) -> Result<Moves> {
        let Some(current) = self.get(id)? else {
            return Ok(Vec::new());
        };
        if current.content == content {
            return Ok(Vec::new());
        }
        let last_revision: Option<i64> = self.store.conn().query_row(
            "SELECT max(created_at) FROM revisions WHERE document_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if last_revision.is_none_or(|t| now_ms() - t >= self.revision_interval_ms)
            && !current.content.trim().is_empty()
        {
            self.snapshot(&current)?;
        }
        self.write(id, content)?;
        if follows(
            &current.title,
            &file_stem(&display_title("", &current.content)),
        ) || follows(&current.title, UNTITLED)
        {
            return self.rename(id, &display_title("", content));
        }
        Ok(Vec::new())
    }

    /// Datei umbenennen (Titel → Dateiname); Links in anderen Notizen ziehen nach.
    pub fn rename(&mut self, id: &str, title: &str) -> Result<Moves> {
        let dir = parent_of(id);
        let new = self.free_id(dir.as_deref(), &file_stem(title), Some(id));
        self.move_note(id, &new)
    }

    pub fn move_to_folder(&mut self, id: &str, folder: Option<&str>) -> Result<Moves> {
        let new = self.free_id(folder, stem(id), Some(id));
        self.move_note(id, &new)
    }

    fn move_note(&mut self, old: &str, new: &str) -> Result<Moves> {
        if old == new {
            return Ok(Vec::new());
        }
        let target = self.path_of(new);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(self.path_of(old), &target)?;
        let moves = vec![(old.to_string(), new.to_string())];
        self.apply_moves(&moves)?;
        Ok(moves)
    }

    /// Nach Verschieben auf der Platte: Index-IDs nachziehen und Links in allen aktiven
    /// Notizen anpassen (auch die relativen Links der verschobenen Notizen selbst).
    fn apply_moves(&mut self, moves: &[(String, String)]) -> Result<()> {
        let map: HashMap<&str, &str> = moves
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        {
            let device = self.store.device_id().to_string();
            let tx = self.store.transaction()?;
            // IDs ändern sich, Revisionen/Tags zeigen kurzzeitig ins Leere.
            tx.pragma_update(None, "defer_foreign_keys", "ON")?;
            for (old, new) in moves {
                let folder = if new.starts_with(&format!("{TRASH_DIR}/")) {
                    None
                } else {
                    parent_of(new)
                };
                ensure_folder(&tx, folder.as_deref(), &device)?;
                tx.execute(
                    "UPDATE documents SET id = ?1, title = ?2, folder_id = ?3 WHERE id = ?4",
                    params![new, stem(new), folder, old],
                )?;
                tx.execute(
                    "UPDATE revisions SET document_id = ?1 WHERE document_id = ?2",
                    params![new, old],
                )?;
                tx.execute(
                    "UPDATE document_tags SET document_id = ?1 WHERE document_id = ?2",
                    params![new, old],
                )?;
                tx.execute(
                    "UPDATE attachments SET document_id = ?1 WHERE document_id = ?2",
                    params![new, old],
                )?;
            }
            tx.commit()?;
        }
        let reverse: HashMap<&str, &str> = moves
            .iter()
            .map(|(a, b)| (b.as_str(), a.as_str()))
            .collect();
        for document in self.list(&Filter::default())? {
            let old_source = reverse
                .get(document.id.as_str())
                .copied()
                .unwrap_or(&document.id);
            let rewritten =
                links::rewrite_links(&document.content, old_source, &document.id, |id| {
                    map.get(id).map(|s| s.to_string())
                });
            if let Some(content) = rewritten {
                self.write(&document.id, &content)?;
            }
        }
        // Mtime nach dem Verschieben übernehmen, sonst liest der nächste Abgleich neu ein.
        for (_, new) in moves {
            let modified = mtime_ms(&self.path_of(new));
            self.store.conn().execute(
                "UPDATE documents SET updated_at = ?1 WHERE id = ?2",
                params![modified, new],
            )?;
        }
        Ok(())
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<()> {
        self.store.conn().execute(
            "UPDATE documents SET favorite = ?1 WHERE id = ?2",
            params![favorite as i64, id],
        )?;
        Ok(())
    }

    /// In den Papierkorb (`.trash/` im Vault); Links auf die Notiz bleiben stehen und
    /// funktionieren nach dem Wiederherstellen wieder.
    pub fn trash(&mut self, id: &str) -> Result<Moves> {
        if let Some(document) = self.get(id)? {
            self.snapshot(&document)?;
        }
        let dir = match parent_of(id) {
            Some(parent) => format!("{TRASH_DIR}/{parent}"),
            None => TRASH_DIR.to_string(),
        };
        let new = self.free_id(Some(&dir), stem(id), None);
        let target = self.path_of(&new);
        std::fs::create_dir_all(target.parent().expect("im Vault"))?;
        std::fs::rename(self.path_of(id), &target)?;
        let moves = vec![(id.to_string(), new.clone())];
        {
            let tx = self.store.transaction()?;
            tx.pragma_update(None, "defer_foreign_keys", "ON")?;
            tx.execute(
                "UPDATE documents SET id = ?1, folder_id = NULL, deleted_at = ?2, updated_at = ?3 WHERE id = ?4",
                params![new, now_ms(), mtime_ms(&target), id],
            )?;
            tx.execute(
                "UPDATE revisions SET document_id = ?1 WHERE document_id = ?2",
                params![new, id],
            )?;
            tx.execute(
                "DELETE FROM document_tags WHERE document_id = ?1",
                params![id],
            )?;
            tx.commit()?;
        }
        Ok(moves)
    }

    /// Aus dem Papierkorb an den alten Ort (bei Namensgleichheit mit Nummer).
    pub fn restore(&mut self, id: &str) -> Result<Moves> {
        let Some(original) = id.strip_prefix(&format!("{TRASH_DIR}/")) else {
            return Ok(Vec::new());
        };
        let new = self.free_id(parent_of(original).as_deref(), stem(original), None);
        let target = self.path_of(&new);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(self.path_of(id), &target)?;
        self.store.conn().execute(
            "UPDATE documents SET deleted_at = NULL WHERE id = ?1",
            params![id],
        )?;
        let moves = vec![(id.to_string(), new.clone())];
        self.apply_moves(&moves)?;
        let content = std::fs::read_to_string(&target)?;
        self.index(&new, &content, None)?;
        Ok(moves)
    }

    /// Endgültig löschen, was länger als `older_than_ms` im Papierkorb liegt.
    pub fn purge_trash(&mut self, older_than_ms: i64) -> Result<usize> {
        let cutoff = now_ms() - older_than_ms;
        let ids: Vec<String> = {
            let mut stmt = self.store.conn().prepare(
                "SELECT id FROM documents WHERE deleted_at IS NOT NULL AND deleted_at <= ?1",
            )?;
            stmt.query_map(params![cutoff], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in &ids {
            self.purge(id)?;
        }
        Ok(ids.len())
    }

    /// Endgültig löschen samt Verlauf (nur aus dem Papierkorb).
    pub fn purge(&mut self, id: &str) -> Result<bool> {
        if !id.starts_with(&format!("{TRASH_DIR}/")) {
            return Ok(false);
        }
        match std::fs::remove_file(self.path_of(id)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
            _ => {}
        }
        self.unindex(id)?;
        Ok(true)
    }

    // ---- Revisionen ----

    fn snapshot(&mut self, document: &Document) -> Result<()> {
        self.store.conn().execute(
            "INSERT INTO revisions(id, document_id, title, content, created_at, device_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![new_id(), document.id, document.title, document.content, now_ms(), self.store.device_id()],
        )?;
        Ok(())
    }

    /// Versionsverlauf, neueste zuerst.
    pub fn revisions(&self, id: &str) -> Result<Vec<Revision>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT id, document_id, title, content, created_at FROM revisions WHERE document_id = ?1 ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map(params![id], |r| {
            Ok(Revision {
                id: r.get(0)?,
                document_id: r.get(1)?,
                title: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Stellt eine Revision wieder her; der aktuelle Stand wird vorher selbst gesichert.
    /// Der Dateiname bleibt.
    pub fn restore_revision(&mut self, revision_id: &str) -> Result<bool> {
        let revision: Option<(String, String)> = self
            .store
            .conn()
            .query_row(
                "SELECT document_id, content FROM revisions WHERE id = ?1",
                params![revision_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((document_id, content)) = revision else {
            return Ok(false);
        };
        let Some(current) = self.get(&document_id)? else {
            return Ok(false);
        };
        self.snapshot(&current)?;
        self.write(&document_id, &content)?;
        Ok(true)
    }

    // ---- Ordner ----

    /// Alle Ordner im Vault (ohne versteckte), alphabetisch nach Pfad.
    pub fn folders(&self) -> Result<Vec<Folder>> {
        let (mut files, mut dirs) = (Vec::new(), Vec::new());
        walk(&self.root, &self.root, &mut files, &mut dirs);
        dirs.sort_by_key(|d| d.to_lowercase());
        Ok(dirs
            .into_iter()
            .map(|id| Folder {
                parent_id: parent_of(&id),
                name: id.rsplit('/').next().unwrap_or(&id).to_string(),
                id,
            })
            .collect())
    }

    pub fn create_folder(&mut self, name: &str, parent: Option<&str>) -> Result<Folder> {
        let name = file_stem(name);
        let id = (1..)
            .map(|n| {
                join(
                    parent,
                    &if n == 1 {
                        name.clone()
                    } else {
                        format!("{name} {n}")
                    },
                )
            })
            .find(|id| !self.path_of(id).exists())
            .expect("unendlich viele Namen");
        std::fs::create_dir_all(self.path_of(&id))?;
        Ok(Folder {
            parent_id: parent.map(str::to_string),
            name: id.rsplit('/').next().unwrap_or(&id).to_string(),
            id,
        })
    }

    /// Ordner umbenennen; gibt die neue Ordner-ID und die verschobenen Notizen zurück.
    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<(String, Moves)> {
        let parent = parent_of(id);
        let name = file_stem(name);
        let new = (1..)
            .map(|n| {
                join(
                    parent.as_deref(),
                    &if n == 1 {
                        name.clone()
                    } else {
                        format!("{name} {n}")
                    },
                )
            })
            .find(|candidate| candidate == id || !self.path_of(candidate).exists())
            .expect("unendlich viele Namen");
        let moves = self.move_dir(id, &new)?;
        Ok((new, moves))
    }

    fn move_dir(&mut self, old: &str, new: &str) -> Result<Moves> {
        if old == new {
            return Ok(Vec::new());
        }
        std::fs::rename(self.path_of(old), self.path_of(new))?;
        let prefix = format!("{old}/");
        let moves: Moves = self
            .list(&Filter::default())?
            .into_iter()
            .filter_map(|d| {
                d.id.strip_prefix(&prefix)
                    .map(|rest| (d.id.clone(), format!("{new}/{rest}")))
            })
            .collect();
        self.apply_moves(&moves)?;
        Ok(moves)
    }

    /// Ordner auflösen: Inhalt (Notizen, Unterordner, andere Dateien) rückt eine Ebene nach
    /// oben – nichts geht verloren.
    pub fn delete_folder(&mut self, id: &str) -> Result<Moves> {
        let dir = self.path_of(id);
        let parent = parent_of(id);
        let mut moves = Vec::new();
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let old = format!("{id}/{name}");
            if name == ".DS_Store" {
                std::fs::remove_file(entry.path())?;
            } else if entry.path().is_dir() {
                let new = (1..)
                    .map(|n| {
                        join(
                            parent.as_deref(),
                            &if n == 1 {
                                name.clone()
                            } else {
                                format!("{name} {n}")
                            },
                        )
                    })
                    .find(|c| !self.path_of(c).exists())
                    .expect("unendlich viele Namen");
                moves.extend(self.move_dir(&old, &new)?);
            } else if name.to_lowercase().ends_with(".md") {
                moves.extend(self.move_to_folder(&old, parent.as_deref())?);
            } else {
                let new = (1..)
                    .map(|n| {
                        join(
                            parent.as_deref(),
                            &if n == 1 {
                                name.clone()
                            } else {
                                format!("{n}-{name}")
                            },
                        )
                    })
                    .find(|c| !self.path_of(c).exists())
                    .expect("unendlich viele Namen");
                std::fs::rename(entry.path(), self.path_of(&new))?;
            }
        }
        std::fs::remove_dir(&dir)?;
        Ok(moves)
    }

    // ---- Tags ----

    fn set_tags(&mut self, document_id: &str, names: &[String]) -> Result<()> {
        let now = now_ms();
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        tx.execute(
            "DELETE FROM document_tags WHERE document_id = ?1",
            params![document_id],
        )?;
        for name in names {
            let id = tag_id(name);
            tx.execute(
                "INSERT INTO tags(id, name, created_at, updated_at, device_id) VALUES (?1, ?2, ?3, ?3, ?4)
                 ON CONFLICT(id) DO NOTHING",
                params![id, name, now, device],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO document_tags(document_id, tag_id, created_at, device_id) VALUES (?1, ?2, ?3, ?4)",
                params![document_id, id, now, device],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn tags_of(&self, document_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id
             WHERE dt.document_id = ?1 ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![document_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Alle benutzten Tags mit Anzahl aktiver Notizen.
    pub fn tags(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT t.name, count(d.id) FROM tags t
             JOIN document_tags dt ON dt.tag_id = t.id
             JOIN documents d ON d.id = dt.document_id AND d.deleted_at IS NULL
             GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- Verknüpfungen ----

    /// Aktive Notizen, die per Link auf `id` verweisen.
    pub fn backlinks(&self, id: &str) -> Result<Vec<Document>> {
        Ok(self
            .list(&Filter::default())?
            .into_iter()
            .filter(|d| d.id != id)
            .filter(|d| {
                note_links(&d.content)
                    .iter()
                    .any(|l| resolve(&d.id, &l.target).as_deref() == Some(id))
            })
            .collect())
    }

    // ---- Suche ----

    /// Volltextsuche über Titel und Inhalt (Wortanfänge, Umlaut-/Akzent-tolerant).
    pub fn search(&self, query: &str, limit: usize, order: SearchOrder) -> Result<Vec<SearchHit>> {
        let Some(fts_query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let order_by = match order {
            SearchOrder::Relevance => "bm25(documents_fts, 5.0, 1.0)",
            SearchOrder::Recent => "d.updated_at DESC, bm25(documents_fts, 5.0, 1.0)",
        };
        let mut stmt = self.store.conn().prepare(&format!(
            "SELECT d.id, d.title, snippet(documents_fts, 1, '«', '»', '…', 12),
                    d.folder_id, d.updated_at
             FROM documents_fts JOIN documents d ON d.rid = documents_fts.rowid
             WHERE documents_fts MATCH ?1 AND d.deleted_at IS NULL
             ORDER BY {order_by} LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |r| {
            Ok(SearchHit {
                id: r.get(0)?,
                title: r.get(1)?,
                snippet: r.get(2)?,
                folder_id: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// Ordnerpfad samt Elternordnern in der Tabelle `folders` (Fremdschlüssel von
/// `documents.folder_id`); maßgeblich sind die Ordner auf der Platte.
fn ensure_folder(conn: &rusqlite::Connection, folder: Option<&str>, device: &str) -> Result<()> {
    let Some(folder) = folder else {
        return Ok(());
    };
    let now = now_ms();
    let mut parent: Option<String> = None;
    for part in folder.split('/') {
        let id = join(parent.as_deref(), part);
        conn.execute(
            "INSERT OR IGNORE INTO folders(id, parent_id, name, created_at, updated_at, device_id) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![id, parent, part, now, device],
        )?;
        parent = Some(id);
    }
    Ok(())
}

/// Stabile Tag-ID aus dem Namen (Groß-/Kleinschreibung egal).
pub fn tag_id(name: &str) -> String {
    format!("tag:{}", name.trim().trim_start_matches('#').to_lowercase())
}

/// `tags:` im Frontmatter (`[a, b]` oder Liste mit `- a`).
fn frontmatter_tags(content: &str) -> Vec<String> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Vec::new();
    };
    let Some(end) = rest.find("\n---") else {
        return Vec::new();
    };
    let mut tags = Vec::new();
    let mut in_list = false;
    for line in rest[..end].lines() {
        if let Some(value) = line.strip_prefix("tags:") {
            let value = value.trim().trim_start_matches('[').trim_end_matches(']');
            tags.extend(
                value
                    .split(',')
                    .map(|t| t.trim().trim_matches(['"', '\'', '#']).to_string()),
            );
            in_list = value.is_empty();
        } else if in_list && let Some(item) = line.trim_start().strip_prefix("- ") {
            tags.push(item.trim().trim_matches(['"', '\'', '#']).to_string());
        } else {
            in_list = false;
        }
    }
    tags.retain(|t| !t.is_empty());
    tags
}

/// `#tags` im Markdown-Text (wie in Apple Notes/Obsidian). Überschriften (`# Titel`),
/// Code, Links-Anker und Farben wie `#fff` zählen nicht.
pub fn extract_hashtags(markdown: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut in_code = false;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
            } else if c == '#' && !in_code {
                let boundary =
                    i == 0 || chars[i - 1].is_whitespace() || matches!(chars[i - 1], '(' | ',');
                let start = i + 1;
                let mut end = start;
                while end < chars.len()
                    && (chars[end].is_alphanumeric() || matches!(chars[end], '-' | '_' | '/'))
                {
                    end += 1;
                }
                let tag: String = chars[start..end].iter().collect();
                let tag = tag.trim_end_matches(['-', '_', '/']).to_string();
                let is_hex_color = (tag.len() == 3 || tag.len() == 6)
                    && tag.chars().all(|c| c.is_ascii_hexdigit());
                if boundary
                    && !tag.is_empty()
                    && tag.chars().next().is_some_and(char::is_alphabetic)
                    && !is_hex_color
                    && !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag))
                {
                    tags.push(tag);
                }
                i = end;
                continue;
            }
            i += 1;
        }
    }
    tags
}

/// Nutzereingabe → FTS5-Abfrage: jedes Wort als Präfix, alle Wörter müssen vorkommen.
/// Sonderzeichen werden gequotet, damit keine FTS-Syntaxfehler entstehen.
fn fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Vault in einem frischen Temp-Ordner, Index im Speicher.
    pub(crate) fn vault() -> (Notes, PathBuf) {
        let dir = std::env::temp_dir().join(format!("devmark-vault-{}", new_id()));
        (
            Notes::with_store(&dir, Store::open_in_memory().unwrap()).unwrap(),
            dir,
        )
    }

    fn read(dir: &Path, id: &str) -> String {
        std::fs::read_to_string(dir.join(id)).unwrap()
    }

    #[test]
    fn notes_are_files_named_after_first_line() {
        let (mut n, dir) = vault();
        let note = n.create_note("").unwrap();
        assert_eq!(note.id, "Neue Notiz.md");
        let moves = n.save_content(&note.id, "# Einkauf\nMilch").unwrap();
        assert_eq!(
            moves,
            [("Neue Notiz.md".to_string(), "Einkauf.md".to_string())]
        );
        assert_eq!(read(&dir, "Einkauf.md"), "# Einkauf\nMilch");
        assert!(!dir.join("Neue Notiz.md").exists());
        // Gleicher Titel → Nummer; auch so nummerierte Dateien folgen der ersten Zeile.
        let second = n.create_note("# Einkauf").unwrap();
        assert_eq!(second.id, "Einkauf 2.md");
        assert_eq!(
            n.save_content(&second.id, "# Brot").unwrap()[0].1,
            "Brot.md"
        );
        // Von Hand anders benannte Datei behält ihren Namen.
        std::fs::write(dir.join("Eigener Name.md"), "# Erste Zeile").unwrap();
        n.rescan().unwrap();
        assert!(
            n.save_content("Eigener Name.md", "# Andere Zeile")
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rescan_picks_up_outside_changes() {
        let (mut n, dir) = vault();
        std::fs::create_dir_all(dir.join("Projekte")).unwrap();
        std::fs::write(dir.join("Projekte/Plan.md"), "Text #idee").unwrap();
        std::fs::create_dir_all(dir.join(".obsidian")).unwrap();
        std::fs::write(dir.join(".obsidian/x.md"), "versteckt").unwrap();
        assert!(n.rescan().unwrap());
        assert!(!n.rescan().unwrap(), "zweiter Abgleich ohne Änderung");
        let all = n.list(&Filter::default()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].folder_id.as_deref(), Some("Projekte"));
        assert_eq!(n.tags().unwrap(), [("idee".to_string(), 1)]);
        assert_eq!(n.folders().unwrap()[0].id, "Projekte");
        std::fs::remove_file(dir.join("Projekte/Plan.md")).unwrap();
        n.rescan().unwrap();
        assert!(n.list(&Filter::default()).unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn moving_rewrites_links_everywhere() {
        let (mut n, dir) = vault();
        let folder = n.create_folder("Arbeit", None).unwrap();
        let target = n.create("Ziel", "Inhalt", Some(&folder.id)).unwrap();
        let source = n
            .create(
                "Quelle",
                &format!("Siehe {}", link_markup("Ziel", "Quelle.md", &target.id)),
                None,
            )
            .unwrap();
        assert_eq!(read(&dir, &source.id), "Siehe [Ziel](Arbeit/Ziel.md)");
        assert_eq!(n.backlinks(&target.id).unwrap()[0].id, source.id);

        n.rename(&target.id, "Neues Ziel").unwrap();
        assert_eq!(
            read(&dir, &source.id),
            "Siehe [Neues Ziel](Arbeit/Neues%20Ziel.md)"
        );
        n.move_to_folder(&source.id, Some("Arbeit")).unwrap();
        assert_eq!(
            read(&dir, "Arbeit/Quelle.md"),
            "Siehe [Neues Ziel](Neues%20Ziel.md)"
        );

        let (new_folder, moves) = n.rename_folder("Arbeit", "Job").unwrap();
        assert_eq!(new_folder, "Job");
        assert_eq!(moves.len(), 2);
        assert_eq!(
            read(&dir, "Job/Quelle.md"),
            "Siehe [Neues Ziel](Neues%20Ziel.md)"
        );
        assert_eq!(n.backlinks("Job/Neues Ziel.md").unwrap().len(), 1);

        n.delete_folder("Job").unwrap();
        assert_eq!(
            read(&dir, "Quelle.md"),
            "Siehe [Neues Ziel](Neues%20Ziel.md)"
        );
        assert!(n.folders().unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trash_restore_purge() {
        let (mut n, dir) = vault();
        let doc = n.create("Weg", "weg damit #alt", None).unwrap();
        n.set_favorite(&doc.id, true).unwrap();
        let moves = n.trash(&doc.id).unwrap();
        let trashed = &moves[0].1;
        assert_eq!(trashed, ".trash/Weg.md");
        assert!(dir.join(".trash/Weg.md").exists());
        assert!(n.list(&Filter::default()).unwrap().is_empty());
        assert_eq!(
            n.list(&Filter {
                trash: true,
                ..Default::default()
            })
            .unwrap()
            .len(),
            1
        );
        assert!(
            n.search("weg", 10, SearchOrder::Relevance)
                .unwrap()
                .is_empty(),
            "Papierkorb nicht in der Suche"
        );
        // Abgleich behält Papierkorb-Zustand.
        assert!(!n.rescan().unwrap());
        let restored = n.restore(trashed).unwrap()[0].1.clone();
        assert_eq!(restored, "Weg.md");
        assert!(
            n.get(&restored).unwrap().unwrap().favorite,
            "Favorit bleibt"
        );
        assert_eq!(n.tags_of(&restored).unwrap(), ["alt"]);
        let trashed = n.trash(&restored).unwrap()[0].1.clone();
        assert_eq!(n.purge_trash(0).unwrap(), 1);
        assert!(n.get(&trashed).unwrap().is_none());
        assert!(!dir.join(".trash/Weg.md").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn revisions_are_coalesced_and_follow_renames() {
        let (mut n, dir) = vault();
        let doc = n.create("Text", "v1", None).unwrap();
        n.rename(&doc.id, "Fest").unwrap();
        let id = "Fest.md";
        n.save_content(id, "v2").unwrap();
        n.save_content(id, "v3").unwrap();
        let revisions = n.revisions(id).unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].content, "v1");
        n.revision_interval_ms = 0;
        n.save_content(id, "v4").unwrap();
        let first = n.revisions(id).unwrap().last().unwrap().id.clone();
        n.restore_revision(&first).unwrap();
        assert_eq!(read(&dir, id), "v1");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn full_text_search_and_frontmatter_tags() {
        let (mut n, dir) = vault();
        n.create("Rezept", "Apfelkuchen mit Zimt", None).unwrap();
        n.create(
            "Größenvergleich",
            "---\ntags: [maße, \"#vergleich\"]\n---\nFluss und Straße",
            None,
        )
        .unwrap();
        let hits = n.search("apfel", 10, SearchOrder::Relevance).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rezept");
        assert!(
            hits[0].snippet.contains("«Apfelkuchen»"),
            "{}",
            hits[0].snippet
        );
        assert_eq!(
            n.search("Gro", 10, SearchOrder::Relevance).unwrap().len(),
            1,
            "remove_diacritics"
        );
        assert!(n.search("\"\"* OR ( NEAR", 10, SearchOrder::Recent).is_ok());
        assert_eq!(
            n.tags_of("Größenvergleich.md").unwrap(),
            ["maße", "vergleich"]
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn search_order_relevance_or_recent() {
        let (mut n, dir) = vault();
        // Älter, aber „birne“ steht im Titel und mehrfach im Text: bessere Relevanz.
        let old = n.create("Birne", "Birne Birne Birne", None).unwrap();
        let new = n
            .create("Einkauf", "Milch, eine Birne", Some("Haus"))
            .unwrap();
        n.store
            .conn()
            .execute(
                "UPDATE documents SET updated_at = 1000 WHERE id = ?1",
                params![old.id],
            )
            .unwrap();
        let ids = |order| -> Vec<String> {
            n.search("birne", 10, order)
                .unwrap()
                .into_iter()
                .map(|h| h.id)
                .collect()
        };
        assert_eq!(
            ids(SearchOrder::Relevance),
            [old.id.clone(), new.id.clone()]
        );
        assert_eq!(ids(SearchOrder::Recent), [new.id.clone(), old.id]);
        let hits = n.search("einkauf", 10, SearchOrder::Recent).unwrap();
        assert_eq!(hits[0].folder_id.as_deref(), Some("Haus"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn hashtags() {
        let text = "# Überschrift\nText #projekt und #Größe, nicht a#b oder `#code`\n#start (#klammer) #fff #2026 #projekt\n```\n#nein\n```";
        assert_eq!(
            extract_hashtags(text),
            ["projekt", "Größe", "start", "klammer"]
        );
    }

    #[test]
    fn titles_and_stems() {
        assert_eq!(
            display_title("", "---\ntags: [a]\n---\n\n# Titel\nText"),
            "Titel"
        );
        assert_eq!(display_title("", ""), "Neue Notiz");
        assert_eq!(file_stem("a/b: c?"), "a-b- c-");
        assert_eq!(file_stem("..."), "Neue Notiz");
    }
}
