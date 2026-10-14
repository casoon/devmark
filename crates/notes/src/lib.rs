//! Notizen und interne Dokumente: Anlegen, Autosave, Ordner, Tags, Favoriten,
//! Volltextsuche, Versionsverlauf, Papierkorb, Export als Dateien.
//!
//! Gemeinsames Dokumentenmodell: Notiz, Artikel, Buch und Kapitel liegen in einer Tabelle
//! (`kind`), Bücher enthalten Kapitel über `parent_id`.

mod export;

use std::path::Path;

use devmark_storage::{ChangeOp, Store, new_id, now_ms, record_change};
use rusqlite::{OptionalExtension, Row, params};

pub use devmark_storage::{BackupInfo, Result, create_backup, list_backups};
pub use export::export_markdown;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Note,
    Article,
    Book,
    Chapter,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Note => "note",
            Kind::Article => "article",
            Kind::Book => "book",
            Kind::Chapter => "chapter",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "article" => Kind::Article,
            "book" => Kind::Book,
            "chapter" => Kind::Chapter,
            _ => Kind::Note,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Typst,
}

impl Format {
    fn as_str(self) -> &'static str {
        match self {
            Format::Markdown => "markdown",
            Format::Typst => "typst",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "typst" { Format::Typst } else { Format::Markdown }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Markdown => "md",
            Format::Typst => "typ",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub id: String,
    pub kind: Kind,
    pub format: Format,
    pub parent_id: Option<String>,
    pub folder_id: Option<String>,
    pub title: String,
    pub content: String,
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub version: i64,
}

impl Document {
    /// Anzeigename: Titel oder – wie in Apple Notes – die erste Textzeile.
    pub fn display_title(&self) -> String {
        display_title(&self.title, &self.content)
    }
}

/// Titel oder erste nicht-leere Zeile (ohne Markdown-/Typst-Überschriftszeichen).
pub fn display_title(title: &str, content: &str) -> String {
    if !title.trim().is_empty() {
        return title.trim().to_string();
    }
    let line = content
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '=']).trim())
        .find(|l| !l.is_empty() && *l != "---")
        .unwrap_or("Neue Notiz");
    line.chars().take(80).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
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
}

/// Auswahl für Listen.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// `None` = alle Ordner, `Some(None)` = ohne Ordner, `Some(Some(id))` = dieser Ordner.
    pub folder: Option<Option<String>>,
    pub tag: Option<String>,
    pub favorites_only: bool,
    pub kind: Option<Kind>,
    /// Papierkorb statt aktiver Dokumente.
    pub trash: bool,
}

pub struct Notes {
    store: Store,
    /// Mindestabstand zwischen zwei automatischen Revisionen.
    pub revision_interval_ms: i64,
}

const DOCUMENT_COLUMNS: &str = "id, kind, format, parent_id, folder_id, title, content, favorite, created_at, updated_at, deleted_at, version";

fn document_from_row(row: &Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        kind: Kind::parse(&row.get::<_, String>(1)?),
        format: Format::parse(&row.get::<_, String>(2)?),
        parent_id: row.get(3)?,
        folder_id: row.get(4)?,
        title: row.get(5)?,
        content: row.get(6)?,
        favorite: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        deleted_at: row.get(10)?,
        version: row.get(11)?,
    })
}

impl Notes {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::with_store(Store::open(path)?))
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self::with_store(Store::open_in_memory()?))
    }

    pub fn with_store(store: Store) -> Self {
        Self { store, revision_interval_ms: 10 * 60 * 1000 }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    // ---- Dokumente ----

    pub fn create(&mut self, kind: Kind, format: Format, title: &str, content: &str, folder_id: Option<&str>) -> Result<Document> {
        let id = new_id();
        let now = now_ms();
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        tx.execute(
            "INSERT INTO documents(id, kind, format, folder_id, title, content, created_at, updated_at, device_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)",
            params![id, kind.as_str(), format.as_str(), folder_id, title, content, now, device],
        )?;
        record_change(&tx, &device, "document", &id, ChangeOp::Upsert, 1)?;
        tx.commit()?;
        Ok(self.get(&id)?.expect("just inserted"))
    }

    /// Schnelle Notiz (Markdown, ohne Ordner) – „sofort schreiben“.
    pub fn create_note(&mut self, content: &str) -> Result<Document> {
        self.create(Kind::Note, Format::Markdown, "", content, None)
    }

    pub fn get(&self, id: &str) -> Result<Option<Document>> {
        self.store
            .conn()
            .query_row(
                &format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id = ?1"),
                params![id],
                document_from_row,
            )
            .optional()
    }

    /// Dokumente nach Filter; Favoriten zuerst, dann zuletzt geändert.
    pub fn list(&self, filter: &Filter) -> Result<Vec<Document>> {
        let mut sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents d WHERE ");
        sql.push_str(if filter.trash { "deleted_at IS NOT NULL" } else { "deleted_at IS NULL" });
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
                   WHERE dt.document_id = d.id AND dt.deleted_at IS NULL AND t.name = ?{} COLLATE NOCASE)",
                args.len()
            ));
        }
        if filter.favorites_only {
            sql.push_str(" AND favorite = 1");
        }
        if let Some(kind) = filter.kind {
            args.push(Box::new(kind.as_str()));
            sql.push_str(&format!(" AND kind = ?{}", args.len()));
        }
        sql.push_str(if filter.trash {
            " ORDER BY deleted_at DESC"
        } else {
            " ORDER BY favorite DESC, updated_at DESC"
        });
        let mut stmt = self.store.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())), document_from_row)?;
        rows.collect()
    }

    /// Ändert Felder eines Dokuments: Version hoch, Zeitstempel, Änderungsprotokoll.
    fn update(&mut self, id: &str, set: &str, value: &dyn rusqlite::ToSql) -> Result<bool> {
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        let changed = tx.execute(
            &format!("UPDATE documents SET {set} = ?1, updated_at = ?2, version = version + 1, device_id = ?3 WHERE id = ?4"),
            params![value, now_ms(), device, id],
        )?;
        if changed > 0 {
            let version: i64 = tx.query_row("SELECT version FROM documents WHERE id = ?1", params![id], |r| r.get(0))?;
            record_change(&tx, &device, "document", id, ChangeOp::Upsert, version)?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Autosave: speichert den Inhalt sofort. Revisionen werden zusammengefasst – höchstens
    /// eine je `revision_interval_ms`, und zwar mit dem Stand *vor* der Änderung.
    pub fn save_content(&mut self, id: &str, content: &str) -> Result<bool> {
        let Some(current) = self.get(id)? else {
            return Ok(false);
        };
        if current.content == content {
            return Ok(false);
        }
        let last_revision: Option<i64> = self.store.conn().query_row(
            "SELECT max(created_at) FROM revisions WHERE document_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let due = last_revision.is_none_or(|t| now_ms() - t >= self.revision_interval_ms);
        if due && !current.content.trim().is_empty() {
            self.snapshot(&current)?;
        }
        self.update(id, "content", &content)
    }

    pub fn rename(&mut self, id: &str, title: &str) -> Result<bool> {
        self.update(id, "title", &title)
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<bool> {
        self.update(id, "favorite", &(favorite as i64))
    }

    pub fn move_to_folder(&mut self, id: &str, folder_id: Option<&str>) -> Result<bool> {
        self.update(id, "folder_id", &folder_id)
    }

    pub fn set_format(&mut self, id: &str, format: Format) -> Result<bool> {
        self.update(id, "format", &format.as_str())
    }

    /// In den Papierkorb (Soft-Delete; wird mitsynchronisiert).
    pub fn trash(&mut self, id: &str) -> Result<bool> {
        if let Some(document) = self.get(id)? {
            self.snapshot(&document)?;
        }
        self.update(id, "deleted_at", &now_ms())
    }

    pub fn restore(&mut self, id: &str) -> Result<bool> {
        self.update(id, "deleted_at", &None::<i64>)
    }

    /// Endgültig löschen, was länger als `older_than_ms` im Papierkorb liegt.
    pub fn purge_trash(&mut self, older_than_ms: i64) -> Result<usize> {
        let cutoff = now_ms() - older_than_ms;
        let ids: Vec<String> = {
            let mut stmt = self
                .store
                .conn()
                .prepare("SELECT id FROM documents WHERE deleted_at IS NOT NULL AND deleted_at <= ?1")?;
            stmt.query_map(params![cutoff], |r| r.get(0))?.collect::<Result<_>>()?
        };
        for id in &ids {
            self.purge(id)?;
        }
        Ok(ids.len())
    }

    /// Endgültig löschen samt Revisionen, Tags und Anhängen (nur aus dem Papierkorb).
    pub fn purge(&mut self, id: &str) -> Result<bool> {
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        let in_trash: Option<i64> = tx
            .query_row("SELECT version FROM documents WHERE id = ?1 AND deleted_at IS NOT NULL", params![id], |r| r.get(0))
            .optional()?;
        let Some(version) = in_trash else {
            return Ok(false);
        };
        // Kinder (Kapitel) hängen sonst am gelöschten Buch.
        tx.execute("UPDATE documents SET parent_id = NULL WHERE parent_id = ?1", params![id])?;
        tx.execute("DELETE FROM revisions WHERE document_id = ?1", params![id])?;
        tx.execute("DELETE FROM document_tags WHERE document_id = ?1", params![id])?;
        tx.execute("DELETE FROM attachments WHERE document_id = ?1", params![id])?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        record_change(&tx, &device, "document", id, ChangeOp::Purge, version + 1)?;
        tx.commit()?;
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
        rows.collect()
    }

    /// Stellt eine Revision wieder her; der aktuelle Stand wird vorher selbst gesichert.
    pub fn restore_revision(&mut self, revision_id: &str) -> Result<bool> {
        let revision: Option<(String, String, String)> = self
            .store
            .conn()
            .query_row(
                "SELECT document_id, title, content FROM revisions WHERE id = ?1",
                params![revision_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((document_id, title, content)) = revision else {
            return Ok(false);
        };
        if let Some(current) = self.get(&document_id)? {
            self.snapshot(&current)?;
        }
        self.update(&document_id, "title", &title)?;
        self.update(&document_id, "content", &content)
    }

    // ---- Ordner ----

    pub fn create_folder(&mut self, name: &str, parent_id: Option<&str>) -> Result<Folder> {
        let id = new_id();
        let now = now_ms();
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        tx.execute(
            "INSERT INTO folders(id, parent_id, name, created_at, updated_at, device_id) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![id, parent_id, name, now, device],
        )?;
        record_change(&tx, &device, "folder", &id, ChangeOp::Upsert, 1)?;
        tx.commit()?;
        Ok(Folder { id, parent_id: parent_id.map(str::to_string), name: name.to_string() })
    }

    pub fn folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT id, parent_id, name FROM folders WHERE deleted_at IS NULL ORDER BY position, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok(Folder { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)? }))?;
        rows.collect()
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<bool> {
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        let changed = tx.execute(
            "UPDATE folders SET name = ?1, updated_at = ?2, version = version + 1, device_id = ?3 WHERE id = ?4",
            params![name, now_ms(), device, id],
        )?;
        if changed > 0 {
            let version: i64 = tx.query_row("SELECT version FROM folders WHERE id = ?1", params![id], |r| r.get(0))?;
            record_change(&tx, &device, "folder", id, ChangeOp::Upsert, version)?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Ordner löschen: Unterordner und Dokumente rücken eine Ebene nach oben – nichts geht verloren.
    pub fn delete_folder(&mut self, id: &str) -> Result<bool> {
        let parent: Option<Option<String>> = self
            .store
            .conn()
            .query_row("SELECT parent_id FROM folders WHERE id = ?1 AND deleted_at IS NULL", params![id], |r| r.get(0))
            .optional()?;
        let Some(parent) = parent else {
            return Ok(false);
        };
        let documents: Vec<String> = {
            let mut stmt = self.store.conn().prepare("SELECT id FROM documents WHERE folder_id = ?1")?;
            stmt.query_map(params![id], |r| r.get(0))?.collect::<Result<_>>()?
        };
        for document in documents {
            self.move_to_folder(&document, parent.as_deref())?;
        }
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        tx.execute("UPDATE folders SET parent_id = ?1 WHERE parent_id = ?2", params![parent, id])?;
        tx.execute(
            "UPDATE folders SET deleted_at = ?1, updated_at = ?1, version = version + 1, device_id = ?2 WHERE id = ?3",
            params![now_ms(), device, id],
        )?;
        let version: i64 = tx.query_row("SELECT version FROM folders WHERE id = ?1", params![id], |r| r.get(0))?;
        record_change(&tx, &device, "folder", id, ChangeOp::Delete, version)?;
        tx.commit()?;
        Ok(true)
    }

    // ---- Tags ----

    pub fn add_tag(&mut self, document_id: &str, name: &str) -> Result<()> {
        let name = name.trim().trim_start_matches('#');
        if name.is_empty() {
            return Ok(());
        }
        let device = self.store.device_id().to_string();
        let now = now_ms();
        let tx = self.store.transaction()?;
        let tag_id: String = match tx
            .query_row("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE", params![name], |r| r.get(0))
            .optional()?
        {
            Some(id) => {
                tx.execute("UPDATE tags SET deleted_at = NULL WHERE id = ?1", params![id])?;
                id
            }
            None => {
                let id = new_id();
                tx.execute(
                    "INSERT INTO tags(id, name, created_at, updated_at, device_id) VALUES (?1, ?2, ?3, ?3, ?4)",
                    params![id, name, now, device],
                )?;
                record_change(&tx, &device, "tag", &id, ChangeOp::Upsert, 1)?;
                id
            }
        };
        tx.execute(
            "INSERT INTO document_tags(document_id, tag_id, created_at, device_id) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(document_id, tag_id) DO UPDATE SET deleted_at = NULL, version = version + 1, device_id = excluded.device_id",
            params![document_id, tag_id, now, device],
        )?;
        record_change(&tx, &device, "document_tag", &format!("{document_id}:{tag_id}"), ChangeOp::Upsert, 1)?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_tag(&mut self, document_id: &str, name: &str) -> Result<bool> {
        let device = self.store.device_id().to_string();
        let tx = self.store.transaction()?;
        let changed = tx.execute(
            "UPDATE document_tags SET deleted_at = ?1, version = version + 1, device_id = ?2
             WHERE document_id = ?3 AND deleted_at IS NULL
               AND tag_id = (SELECT id FROM tags WHERE name = ?4 COLLATE NOCASE)",
            params![now_ms(), device, document_id, name.trim().trim_start_matches('#')],
        )?;
        if changed > 0 {
            let tag_id: String = tx.query_row("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE", params![name.trim().trim_start_matches('#')], |r| r.get(0))?;
            record_change(&tx, &device, "document_tag", &format!("{document_id}:{tag_id}"), ChangeOp::Delete, 1)?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn tags_of(&self, document_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id
             WHERE dt.document_id = ?1 AND dt.deleted_at IS NULL ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![document_id], |r| r.get(0))?;
        rows.collect()
    }

    /// Alle benutzten Tags mit Anzahl aktiver Dokumente.
    pub fn tags(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self.store.conn().prepare(
            "SELECT t.name, count(d.id) FROM tags t
             JOIN document_tags dt ON dt.tag_id = t.id AND dt.deleted_at IS NULL
             JOIN documents d ON d.id = dt.document_id AND d.deleted_at IS NULL
             GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))?;
        rows.collect()
    }

    // ---- Suche ----

    /// Volltextsuche über Titel und Inhalt (Wortanfänge, Umlaut-/Akzent-tolerant).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let Some(fts_query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let mut stmt = self.store.conn().prepare(
            "SELECT d.id, d.title, d.content, snippet(documents_fts, 1, '«', '»', '…', 12)
             FROM documents_fts JOIN documents d ON d.rid = documents_fts.rowid
             WHERE documents_fts MATCH ?1 AND d.deleted_at IS NULL
             ORDER BY bm25(documents_fts, 5.0, 1.0) LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |r| {
            let title: String = r.get(1)?;
            let content: String = r.get(2)?;
            Ok(SearchHit { id: r.get(0)?, title: display_title(&title, &content), snippet: r.get(3)? })
        })?;
        rows.collect()
    }
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
mod tests {
    use super::*;

    fn notes() -> Notes {
        Notes::open_in_memory().unwrap()
    }

    #[test]
    fn create_get_and_list() {
        let mut n = notes();
        let a = n.create_note("# Einkauf\nMilch").unwrap();
        let b = n.create(Kind::Article, Format::Typst, "Artikel", "= Text", None).unwrap();
        assert_eq!(a.display_title(), "Einkauf");
        assert_eq!(n.get(&b.id).unwrap().unwrap().format, Format::Typst);
        n.set_favorite(&a.id, true).unwrap();
        let all = n.list(&Filter::default()).unwrap();
        assert_eq!(all[0].id, a.id, "Favoriten zuerst");
        let articles = n.list(&Filter { kind: Some(Kind::Article), ..Default::default() }).unwrap();
        assert_eq!(articles.len(), 1);
    }

    #[test]
    fn autosave_versions_and_change_log() {
        let mut n = notes();
        let doc = n.create_note("eins").unwrap();
        assert!(n.save_content(&doc.id, "zwei").unwrap());
        assert!(!n.save_content(&doc.id, "zwei").unwrap(), "unverändert speichert nichts");
        let doc = n.get(&doc.id).unwrap().unwrap();
        assert_eq!(doc.version, 2);
        let changes: i64 = n.store().conn().query_row("SELECT count(*) FROM changes", [], |r| r.get(0)).unwrap();
        assert_eq!(changes, 2);
    }

    #[test]
    fn revisions_are_coalesced_and_restorable() {
        let mut n = notes();
        let doc = n.create_note("v1").unwrap();
        n.save_content(&doc.id, "v2").unwrap();
        n.save_content(&doc.id, "v3").unwrap();
        // Innerhalb des Intervalls nur eine Revision (Stand vor der ersten Änderung).
        let revisions = n.revisions(&doc.id).unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].content, "v1");
        n.revision_interval_ms = 0;
        n.save_content(&doc.id, "v4").unwrap();
        assert_eq!(n.revisions(&doc.id).unwrap()[0].content, "v3");
        let first = n.revisions(&doc.id).unwrap().last().unwrap().id.clone();
        n.restore_revision(&first).unwrap();
        assert_eq!(n.get(&doc.id).unwrap().unwrap().content, "v1");
    }

    #[test]
    fn trash_restore_purge() {
        let mut n = notes();
        let doc = n.create_note("weg damit").unwrap();
        n.add_tag(&doc.id, "alt").unwrap();
        n.trash(&doc.id).unwrap();
        assert!(n.list(&Filter::default()).unwrap().is_empty());
        assert_eq!(n.list(&Filter { trash: true, ..Default::default() }).unwrap().len(), 1);
        assert!(n.search("weg", 10).unwrap().is_empty(), "Papierkorb nicht in der Suche");
        n.restore(&doc.id).unwrap();
        assert_eq!(n.list(&Filter::default()).unwrap().len(), 1);
        n.trash(&doc.id).unwrap();
        assert_eq!(n.purge_trash(0).unwrap(), 1);
        assert!(n.get(&doc.id).unwrap().is_none());
        let purge: i64 = n
            .store()
            .conn()
            .query_row("SELECT count(*) FROM changes WHERE op = 'purge'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(purge, 1);
    }

    #[test]
    fn folders_and_tags() {
        let mut n = notes();
        let work = n.create_folder("Arbeit", None).unwrap();
        let sub = n.create_folder("Projekte", Some(&work.id)).unwrap();
        let doc = n.create(Kind::Note, Format::Markdown, "Plan", "", Some(&sub.id)).unwrap();
        n.add_tag(&doc.id, "#Wichtig").unwrap();
        n.add_tag(&doc.id, "wichtig").unwrap();
        assert_eq!(n.tags_of(&doc.id).unwrap(), ["Wichtig"]);
        assert_eq!(n.tags().unwrap(), [("Wichtig".to_string(), 1)]);
        let tagged = n.list(&Filter { tag: Some("WICHTIG".into()), ..Default::default() }).unwrap();
        assert_eq!(tagged.len(), 1);
        n.delete_folder(&sub.id).unwrap();
        assert_eq!(n.get(&doc.id).unwrap().unwrap().folder_id.as_deref(), Some(work.id.as_str()));
        assert_eq!(n.folders().unwrap().len(), 1);
        n.remove_tag(&doc.id, "wichtig").unwrap();
        assert!(n.tags_of(&doc.id).unwrap().is_empty());
    }

    #[test]
    fn full_text_search() {
        let mut n = notes();
        n.create_note("# Rezept\nApfelkuchen mit Zimt").unwrap();
        n.create(Kind::Note, Format::Markdown, "Größenvergleich", "Fluss und Straße", None).unwrap();
        let hits = n.search("apfel", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rezept");
        assert!(hits[0].snippet.contains("«Apfelkuchen»"), "{}", hits[0].snippet);
        assert_eq!(n.search("Grö", 10).unwrap().len(), 1);
        assert_eq!(n.search("Gro", 10).unwrap().len(), 1, "remove_diacritics");
        // FTS-Sonderzeichen in der Eingabe dürfen keinen Syntaxfehler auslösen.
        assert!(n.search("\"\"* OR ( NEAR", 10).is_ok());
        assert!(n.search("", 10).unwrap().is_empty());
    }

    #[test]
    fn display_titles() {
        assert_eq!(display_title("", "\n\n= Typst-Titel\nText"), "Typst-Titel");
        assert_eq!(display_title("", "---\n"), "Neue Notiz");
        assert_eq!(display_title(" Eigener ", "x"), "Eigener");
    }
}
