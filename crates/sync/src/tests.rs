//! Zwei Geräte (eigene Datenbanken) über einen gemeinsamen Sync-Ordner.

use super::*;
use devmark_notes::{Filter, Format, Kind, Notes};

struct Setup {
    dir: std::path::PathBuf,
    a: Notes,
    b: Notes,
}

impl Setup {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("devmark-sync-{}", new_id()));
        Self { dir, a: Notes::open_in_memory().unwrap(), b: Notes::open_in_memory().unwrap() }
    }

    fn sync_a(&mut self) -> SyncReport {
        sync(self.a.store_mut(), &FolderTransport::new(&self.dir)).unwrap()
    }

    fn sync_b(&mut self) -> SyncReport {
        sync(self.b.store_mut(), &FolderTransport::new(&self.dir)).unwrap()
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn contents(notes: &Notes) -> Vec<String> {
    let mut all: Vec<String> = notes.list(&Filter::default()).unwrap().into_iter().map(|d| d.content).collect();
    all.sort();
    all
}

#[test]
fn documents_folders_and_tags_reach_the_other_device() {
    let mut s = Setup::new();
    let folder = s.a.create_folder("Arbeit", None).unwrap();
    let doc = s.a.create(Kind::Note, Format::Markdown, "Plan", "Inhalt", Some(&folder.id)).unwrap();
    s.a.add_tag(&doc.id, "wichtig").unwrap();
    let report = s.sync_a();
    assert_eq!(report.pushed, 4, "{report:?}");

    let report = s.sync_b();
    assert_eq!(report.pulled, 4, "{report:?}");
    let synced = s.b.get(&doc.id).unwrap().unwrap();
    assert_eq!(synced.content, "Inhalt");
    assert_eq!(synced.folder_id.as_deref(), Some(folder.id.as_str()));
    assert_eq!(s.b.tags_of(&doc.id).unwrap(), ["wichtig"]);
    assert_eq!(s.b.search("Inhalt", 10).unwrap().len(), 1, "Volltextindex aktualisiert");

    // Nichts mehr zu tun.
    assert!(s.sync_a().is_empty());
    assert!(s.sync_b().is_empty());
}

#[test]
fn edits_propagate_both_ways() {
    let mut s = Setup::new();
    let doc = s.a.create_note("v1").unwrap();
    s.sync_a();
    s.sync_b();
    s.b.save_content(&doc.id, "v2 von B").unwrap();
    s.sync_b();
    let report = s.sync_a();
    assert_eq!(report.changed_documents, std::slice::from_ref(&doc.id));
    assert_eq!(s.a.get(&doc.id).unwrap().unwrap().content, "v2 von B");
}

#[test]
fn concurrent_edits_create_conflict_copy() {
    let mut s = Setup::new();
    let doc = s.a.create_note("Basis").unwrap();
    s.sync_a();
    s.sync_b();
    s.a.save_content(&doc.id, "Fassung A").unwrap();
    s.b.save_content(&doc.id, "Fassung B").unwrap();
    s.sync_a();
    let report = s.sync_b();
    assert_eq!(report.conflicts.len(), 1, "{report:?}");
    // B behält seine Fassung im Original, A's Fassung als Kopie.
    assert_eq!(s.b.get(&doc.id).unwrap().unwrap().content, "Fassung B");
    assert_eq!(contents(&s.b), ["Fassung A", "Fassung B"]);
    s.sync_a();
    assert_eq!(contents(&s.a), ["Fassung A", "Fassung B"], "nichts geht verloren");
    assert!(s.sync_a().is_empty());
    assert!(s.sync_b().is_empty());
}

#[test]
fn trash_and_purge_propagate() {
    let mut s = Setup::new();
    let doc = s.a.create_note("weg").unwrap();
    s.sync_a();
    s.sync_b();
    s.a.trash(&doc.id).unwrap();
    s.sync_a();
    s.sync_b();
    assert!(s.b.get(&doc.id).unwrap().unwrap().deleted_at.is_some(), "Papierkorb synchronisiert");

    s.a.purge(&doc.id).unwrap();
    let report = s.sync_a();
    assert_eq!(report.deleted_remote, 1, "{report:?}");
    let report = s.sync_b();
    assert_eq!(report.deleted_local, 1, "{report:?}");
    assert!(s.b.get(&doc.id).unwrap().is_none());
}

#[test]
fn purge_does_not_destroy_unsynced_edits_elsewhere() {
    let mut s = Setup::new();
    let doc = s.a.create_note("geteilt").unwrap();
    s.sync_a();
    s.sync_b();
    s.b.save_content(&doc.id, "B hat weitergeschrieben").unwrap();
    s.a.trash(&doc.id).unwrap();
    s.a.purge(&doc.id).unwrap();
    s.sync_a();
    s.sync_b();
    assert_eq!(s.b.get(&doc.id).unwrap().unwrap().content, "B hat weitergeschrieben");
    s.sync_a();
    assert_eq!(s.a.get(&doc.id).unwrap().unwrap().content, "B hat weitergeschrieben", "kommt zurück");
}

#[test]
fn same_tag_on_two_devices() {
    let mut s = Setup::new();
    let a_doc = s.a.create_note("a").unwrap();
    let b_doc = s.b.create_note("b").unwrap();
    s.a.add_tag(&a_doc.id, "Projekt").unwrap();
    s.b.add_tag(&b_doc.id, "projekt").unwrap();
    s.sync_a();
    s.sync_b();
    s.sync_a();
    assert_eq!(s.a.tags().unwrap().len(), 1);
    assert_eq!(s.a.tags().unwrap()[0].1, 2);
    assert_eq!(s.b.tags().unwrap()[0].1, 2);
}

#[test]
fn keys_roundtrip() {
    for key in ["5f3c-uuid", "tag:größe/x y", "doc\u{1f}tag:a"] {
        assert_eq!(decode_key(&encode_key(key)), key);
        assert!(encode_key(key).chars().all(|c| c.is_ascii_alphanumeric() || "-_~".contains(c)));
    }
}
