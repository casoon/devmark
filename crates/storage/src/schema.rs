//! Schema-Migrationen über `PRAGMA user_version`. Nur anhängen, nie bestehende ändern.

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // 1: Gemeinsames Dokumentenmodell (Notiz/Artikel/Buch/Kapitel), Ordner, Tags,
    //    Anhänge, Revisionen, Änderungsprotokoll, Volltextsuche.
    r#"
    CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE folders (
        id         TEXT PRIMARY KEY,
        parent_id  TEXT REFERENCES folders(id),
        name       TEXT NOT NULL,
        position   INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER,
        version    INTEGER NOT NULL DEFAULT 1,
        device_id  TEXT NOT NULL
    );

    CREATE TABLE documents (
        -- Ganzzahliger Schlüssel nur für FTS (stabil auch nach VACUUM); `id` ist die
        -- geräteübergreifende Identität.
        rid        INTEGER PRIMARY KEY,
        id         TEXT NOT NULL UNIQUE,
        kind       TEXT NOT NULL DEFAULT 'note' CHECK (kind IN ('note', 'article', 'book', 'chapter')),
        format     TEXT NOT NULL DEFAULT 'markdown' CHECK (format IN ('markdown', 'typst')),
        parent_id  TEXT REFERENCES documents(id),
        folder_id  TEXT REFERENCES folders(id),
        title      TEXT NOT NULL DEFAULT '',
        content    TEXT NOT NULL DEFAULT '',
        favorite   INTEGER NOT NULL DEFAULT 0,
        position   INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER,
        version    INTEGER NOT NULL DEFAULT 1,
        device_id  TEXT NOT NULL
    );
    CREATE INDEX documents_folder ON documents(folder_id, deleted_at);
    CREATE INDEX documents_updated ON documents(updated_at);

    CREATE TABLE tags (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER,
        version    INTEGER NOT NULL DEFAULT 1,
        device_id  TEXT NOT NULL
    );

    CREATE TABLE document_tags (
        document_id TEXT NOT NULL REFERENCES documents(id),
        tag_id      TEXT NOT NULL REFERENCES tags(id),
        created_at  INTEGER NOT NULL,
        deleted_at  INTEGER,
        version     INTEGER NOT NULL DEFAULT 1,
        device_id   TEXT NOT NULL,
        PRIMARY KEY (document_id, tag_id)
    );

    CREATE TABLE attachments (
        id          TEXT PRIMARY KEY,
        document_id TEXT NOT NULL REFERENCES documents(id),
        file_name   TEXT NOT NULL,
        mime_type   TEXT NOT NULL DEFAULT 'application/octet-stream',
        data        BLOB NOT NULL,
        created_at  INTEGER NOT NULL,
        deleted_at  INTEGER,
        version     INTEGER NOT NULL DEFAULT 1,
        device_id   TEXT NOT NULL
    );

    CREATE TABLE revisions (
        id          TEXT PRIMARY KEY,
        document_id TEXT NOT NULL REFERENCES documents(id),
        title       TEXT NOT NULL,
        content     TEXT NOT NULL,
        created_at  INTEGER NOT NULL,
        device_id   TEXT NOT NULL
    );
    CREATE INDEX revisions_document ON revisions(document_id, created_at);

    CREATE TABLE changes (
        seq        INTEGER PRIMARY KEY AUTOINCREMENT,
        entity     TEXT NOT NULL,
        entity_id  TEXT NOT NULL,
        op         TEXT NOT NULL CHECK (op IN ('upsert', 'delete', 'purge')),
        version    INTEGER NOT NULL,
        device_id  TEXT NOT NULL,
        changed_at INTEGER NOT NULL,
        synced     INTEGER NOT NULL DEFAULT 0
    );

    CREATE VIRTUAL TABLE documents_fts USING fts5(
        title, content,
        content = 'documents', content_rowid = 'rid',
        tokenize = 'unicode61 remove_diacritics 2'
    );
    CREATE TRIGGER documents_ai AFTER INSERT ON documents BEGIN
        INSERT INTO documents_fts(rowid, title, content) VALUES (new.rid, new.title, new.content);
    END;
    CREATE TRIGGER documents_ad AFTER DELETE ON documents BEGIN
        INSERT INTO documents_fts(documents_fts, rowid, title, content) VALUES ('delete', old.rid, old.title, old.content);
    END;
    CREATE TRIGGER documents_au AFTER UPDATE OF title, content ON documents BEGIN
        INSERT INTO documents_fts(documents_fts, rowid, title, content) VALUES ('delete', old.rid, old.title, old.content);
        INSERT INTO documents_fts(rowid, title, content) VALUES (new.rid, new.title, new.content);
    END;
    "#,
    // 2: Sync-Zustand je Datensatz (Stand beim letzten Abgleich).
    r#"
    CREATE TABLE sync_state (
        entity      TEXT NOT NULL,
        entity_id   TEXT NOT NULL,
        synced_hash TEXT NOT NULL,
        remote_etag TEXT NOT NULL,
        PRIMARY KEY (entity, entity_id)
    );
    "#,
];

#[cfg(test)]
pub const LATEST: i64 = MIGRATIONS.len() as i64;

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (ix, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", ix as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
