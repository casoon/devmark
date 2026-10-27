//! Notes-Modus: Notizen aus der SQLite-Datenbank – sofort schreiben, Autosave, Suche,
//! Favoriten, Ordner, Verknüpfungen, Papierkorb, Export und tägliches Backup.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use devmark_notes::{
    Document, Filter, Folder, Format, Kind, Notes, Revision, create_backup, export_markdown, extract_hashtags,
    list_backups,
};
use devmark_writing::authoring::Syntax;
use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    v_flex,
};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;

use crate::assist::NoteIndex;
use crate::{
    BackupNow, ExportNotes, NewNote, SelectionToNote, ShowFiles, ShowHistory, ShowNotes, ToggleTrash,
    Workspace,
};

/// Wie lange nach der letzten Eingabe gespeichert wird.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(600);
/// Papierkorb: endgültig löschen nach 30 Tagen.
const TRASH_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Backup-Generationen (täglich).
const BACKUP_KEEP: usize = 14;
const BACKUP_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Files,
    Notes,
}

pub struct NoteRow {
    pub id: String,
    pub title: String,
    /// Anfang des Textes ohne Titel, für zwei Zeilen Vorschau.
    pub preview: String,
    /// Letzte Änderung („vor 5 Min.“); leer bei Suchtreffern.
    pub time: String,
    pub favorite: bool,
    pub folder_id: Option<String>,
}

pub struct NotesState {
    pub notes: Notes,
    pub data_dir: PathBuf,
    pub search: Entity<InputState>,
    pub rows: Vec<NoteRow>,
    pub show_trash: bool,
    /// Navigation „Favoriten“: nur markierte Notizen.
    pub favorites_only: bool,
    pub save_tasks: HashMap<String, Task<()>>,
    /// Alle Tags mit Anzahl; `tag_filter` schränkt die Liste ein.
    pub tags: Vec<(String, usize)>,
    pub tag_filter: Option<String>,
    /// Versionsverlauf der Notiz, falls eingeblendet.
    pub history: Option<(String, Vec<Revision>)>,
    /// Alle aktiven Notizen (id, Titel) für `[[…]]`-Verknüpfungen im Editor.
    pub index: NoteIndex,
    pub folders: Vec<Folder>,
    /// Ausgewählter Ordner; `None` = alle Notizen. Neue Notizen landen darin.
    pub folder_filter: Option<String>,
    /// Ordner, der gerade umbenannt wird, mit Eingabefeld.
    renaming: Option<(String, Entity<InputState>, Subscription)>,
    _search_subscription: Subscription,
}

/// Datenordner je Plattform (ohne zusätzliche Abhängigkeit).
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DEVMARK_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/DevMark")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home).join("DevMark")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("devmark")
    }
}

/// Backup-Ziel: `DEVMARK_BACKUP_DIR` (z. B. externes Laufwerk, Cloud-Ordner) oder Datenordner.
fn backup_dir(data_dir: &std::path::Path) -> PathBuf {
    std::env::var_os("DEVMARK_BACKUP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("Backups"))
}

impl NotesState {
    /// Öffnet die Notiz-Datenbank; Fehler werden angezeigt, die App läuft ohne Notizen weiter.
    pub fn open(window: &mut Window, cx: &mut Context<Workspace>) -> Option<Self> {
        let data_dir = data_dir();
        let notes = match Notes::open(&data_dir.join("devmark.sqlite")) {
            Ok(notes) => notes,
            Err(err) => {
                let message = format!("Notizen nicht verfügbar – Datenbank konnte nicht geöffnet werden: {err}");
                window.defer(cx, move |window, cx| window.push_notification(Notification::error(message), cx));
                return None;
            }
        };
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Notizen durchsuchen …"));
        let subscription = cx.subscribe(&search, |this: &mut Workspace, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_notes(cx);
            }
        });
        Some(Self {
            notes,
            data_dir,
            search,
            rows: Vec::new(),
            show_trash: false,
            favorites_only: false,
            save_tasks: HashMap::new(),
            tags: Vec::new(),
            tag_filter: None,
            history: None,
            index: Rc::default(),
            folders: Vec::new(),
            folder_filter: None,
            renaming: None,
            _search_subscription: subscription,
        })
    }
}

impl Workspace {
    /// Nach dem Öffnen des Fensters: Liste laden, alten Papierkorb leeren, Backup falls fällig.
    pub(crate) fn after_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        if let Err(err) = state.notes.purge_trash(TRASH_RETENTION_MS) {
            window.push_notification(Notification::error(format!("Papierkorb konnte nicht geleert werden: {err}")), cx);
        }
        let dir = backup_dir(&state.data_dir);
        let due = list_backups(&dir)
            .ok()
            .and_then(|backups| backups.first().and_then(|b| std::fs::metadata(&b.path).ok()?.modified().ok()))
            .is_none_or(|modified| {
                modified.elapsed().map(|age| age.as_millis() as i64 >= BACKUP_INTERVAL_MS).unwrap_or(true)
            });
        if due
            && let Err(err) = create_backup(state.notes.store(), &dir, BACKUP_KEEP) {
                window.push_notification(
                    Notification::warning(format!("Automatisches Backup fehlgeschlagen ({}): {err}", dir.display())),
                    cx,
                );
            }
        self.refresh_notes(cx);
        self.start_sync_timer(window, cx);
    }

    pub(crate) fn refresh_notes(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        state.folders = state.notes.folders().unwrap_or_default();
        if let Some(folder) = &state.folder_filter
            && !state.folders.iter().any(|f| f.id == *folder)
        {
            state.folder_filter = None;
        }
        let query = state.search.read(cx).value().trim().to_string();
        let rows = if !query.is_empty() && !state.show_trash {
            state.notes.search(&query, 200).map(|hits| {
                hits.into_iter()
                    .map(|hit| NoteRow {
                        favorite: false,
                        folder_id: state.notes.get(&hit.id).ok().flatten().and_then(|d| d.folder_id),
                        id: hit.id,
                        title: hit.title,
                        preview: hit.snippet.replace(['\n', '\r'], " "),
                        time: String::new(),
                    })
                    .collect()
            })
        } else {
            let filter = Filter {
                trash: state.show_trash,
                favorites_only: state.favorites_only && !state.show_trash,
                tag: state.tag_filter.clone().filter(|_| !state.show_trash),
                folder: state.folder_filter.clone().filter(|_| !state.show_trash).map(Some),
                ..Default::default()
            };
            state.notes.list(&filter).map(|documents| {
                documents
                    .iter()
                    .filter(|d| query.is_empty() || d.display_title().to_lowercase().contains(&query.to_lowercase()))
                    .map(|d| NoteRow {
                        id: d.id.clone(),
                        title: d.display_title(),
                        preview: preview_text(d),
                        time: relative_time(d.updated_at),
                        favorite: d.favorite,
                        folder_id: d.folder_id.clone(),
                    })
                    .collect()
            })
        };
        state.rows = rows.unwrap_or_default();
        *state.index.borrow_mut() = state
            .notes
            .list(&Filter::default())
            .map(|documents| documents.iter().map(|d| (d.id.clone(), d.display_title())).collect())
            .unwrap_or_default();
        state.tags = state.notes.tags().unwrap_or_default();
        if let Some(tag) = &state.tag_filter
            && !state.tags.iter().any(|(t, _)| t.eq_ignore_ascii_case(tag)) {
                state.tag_filter = None;
            }
        if let Some((id, _)) = &state.history {
            let id = id.clone();
            state.history = Some((id.clone(), state.notes.revisions(&id).unwrap_or_default()));
        }
        cx.notify();
    }

    fn note_path(&self, document: &Document) -> PathBuf {
        let dir = self.notes.as_ref().map(|s| s.data_dir.join("Notizen")).unwrap_or_default();
        dir.join(format!("{}.{}", document.id, document.format.extension()))
    }

    pub(crate) fn new_note(&mut self, _: &NewNote, window: &mut Window, cx: &mut Context<Self>) {
        self.create_note_with("", window, cx);
    }

    fn create_note_with(&mut self, content: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let folder = state.folder_filter.clone().filter(|_| !state.show_trash);
        match state.notes.create(Kind::Note, Format::Markdown, "", content, folder.as_deref()) {
            Ok(document) => {
                state.show_trash = false;
                self.sidebar = SidebarMode::Notes;
                self.refresh_notes(cx);
                self.open_note(document.id, window, cx);
            }
            Err(err) => window.push_notification(Notification::error(format!("Notiz konnte nicht angelegt werden: {err}")), cx),
        }
    }

    /// Markierten Text aus dem aktiven Editor als neue Notiz übernehmen.
    pub(crate) fn selection_to_note(&mut self, _: &SelectionToNote, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self
            .active_buffer()
            .map(|b| b.editor.read(cx).selected_value().to_string())
            .unwrap_or_default();
        if selected.trim().is_empty() {
            window.push_notification(Notification::info("Erst Text markieren, dann „Auswahl als Notiz“."), cx);
            return;
        }
        let source = self.active_buffer().map(|b| b.label()).unwrap_or_default();
        let content = if source.is_empty() { selected } else { format!("{selected}\n\n_Aus {source}_\n") };
        self.create_note_with(&content, window, cx);
    }

    pub(crate) fn open_note(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.new_doc = None;
        let pane_ix = self.active_pane;
        if let Some(ix) = self.panes[pane_ix].buffers.iter().position(|b| b.note_id.as_deref() == Some(id.as_str())) {
            self.panes[pane_ix].active = ix;
            cx.notify();
            return;
        }
        let Some(document) = self.notes.as_ref().and_then(|s| s.notes.get(&id).ok().flatten()) else {
            return;
        };
        if document.deleted_at.is_some() {
            window.push_notification(Notification::info("Notiz liegt im Papierkorb – erst wiederherstellen."), cx);
            return;
        }
        let path = self.note_path(&document);
        let title = document.display_title();
        let updated_at = document.updated_at;
        self.open_buffer(path, document.content, Some((document.id, title)), window, cx);
        if let Some(buffer) = self.active_buffer_mut() {
            buffer.saved_at = Some(updated_at);
        }
    }

    pub(crate) fn schedule_note_save(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let task_id = id.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            this.update(cx, |this, cx| this.save_note_now(&task_id, cx)).ok();
        });
        state.save_tasks.insert(id, task);
    }

    /// Alle offenen, geänderten Notizen sofort sichern (Beenden).
    pub(crate) fn save_all_notes(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .panes
            .iter()
            .flat_map(|pane| pane.buffers.iter())
            .filter(|b| b.dirty)
            .filter_map(|b| b.note_id.clone())
            .collect();
        for id in ids {
            if let Err(err) = self.save_note_now(&id, cx) {
                eprintln!("Notiz {id} konnte nicht gespeichert werden: {err}");
            }
        }
    }

    pub(crate) fn save_note(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = self.save_note_now(id, cx) {
            window.push_notification(Notification::error(format!("Notiz konnte nicht gespeichert werden: {err}")), cx);
        }
    }

    fn save_note_now(&mut self, id: &str, cx: &mut Context<Self>) -> devmark_notes::Result<()> {
        let Some(buffer) = self
            .panes
            .iter()
            .flat_map(|pane| pane.buffers.iter())
            .find(|b| b.note_id.as_deref() == Some(id))
        else {
            return Ok(());
        };
        let content = buffer.editor.read(cx).value().to_string();
        let markdown = buffer.syntax == Some(Syntax::Markdown);
        let Some(state) = self.notes.as_mut() else {
            return Ok(());
        };
        state.save_tasks.remove(id);
        state.notes.save_content(id, &content)?;
        if markdown {
            // `#tag` im Text wird zum Tag (wie in Apple Notes).
            state.notes.set_tags(id, &extract_hashtags(&content))?;
        }
        let title = devmark_notes::display_title("", &content);
        for buffer in self.panes.iter_mut().flat_map(|pane| pane.buffers.iter_mut()) {
            if buffer.note_id.as_deref() == Some(id) {
                buffer.dirty = false;
                buffer.saved_at = Some(devmark_storage_now());
                buffer.title = Some(title.clone());
            }
        }
        self.refresh_notes(cx);
        Ok(())
    }

    fn toggle_favorite(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let favorite = state.notes.get(id).ok().flatten().is_some_and(|d| d.favorite);
        state.notes.set_favorite(id, !favorite).ok();
        self.refresh_notes(cx);
    }

    fn trash_note(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        // Offenen Tab erst sichern und schließen.
        self.save_note(id, window, cx);
        for pane in &mut self.panes {
            if let Some(ix) = pane.buffers.iter().position(|b| b.note_id.as_deref() == Some(id)) {
                pane.buffers.remove(ix);
                pane.active = pane.active.min(pane.buffers.len().saturating_sub(1));
            }
        }
        if let Some(state) = self.notes.as_mut() {
            state.notes.trash(id).ok();
        }
        window.push_notification(Notification::info("In den Papierkorb gelegt (30 Tage wiederherstellbar)."), cx);
        self.refresh_notes(cx);
    }

    fn restore_note(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.notes.restore(id).ok();
        }
        self.refresh_notes(cx);
    }

    fn purge_note(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.notes.purge(id).ok();
        }
        self.refresh_notes(cx);
    }

    pub(crate) fn toggle_trash(&mut self, _: &ToggleTrash, _window: &mut Window, cx: &mut Context<Self>) {
        let trash = !self.notes.as_ref().is_some_and(|s| s.show_trash);
        self.show_note_view(trash, false, cx);
    }

    /// Navigation: Notizen, Favoriten oder Papierkorb.
    fn show_note_view(&mut self, trash: bool, favorites: bool, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.show_trash = trash;
            state.favorites_only = favorites;
            self.sidebar = SidebarMode::Notes;
        }
        self.refresh_notes(cx);
    }

    pub(crate) fn show_files(&mut self, _: &ShowFiles, _window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Files;
        cx.notify();
    }

    pub(crate) fn show_notes(&mut self, _: &ShowNotes, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Notes;
        if let Some(state) = self.notes.as_ref() {
            state.search.update(cx, |input, cx| input.focus(window, cx));
        }
        self.refresh_notes(cx);
    }

    pub(crate) fn export_notes(&mut self, _: &ExportNotes, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Hierhin exportieren".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(dir) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let Some(state) = this.notes.as_ref() else {
                    return;
                };
                let target = dir.join("DevMark-Notizen");
                match export_markdown(&state.notes, &target) {
                    Ok(count) => window.push_notification(
                        Notification::success(format!("{count} Notizen nach „{}“ exportiert.", target.display())),
                        cx,
                    ),
                    Err(err) => window.push_notification(Notification::error(format!("Export fehlgeschlagen: {err}")), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn backup_now(&mut self, _: &BackupNow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_ref() else {
            return;
        };
        let dir = backup_dir(&state.data_dir);
        match create_backup(state.notes.store(), &dir, BACKUP_KEEP) {
            Ok(info) => {
                window.push_notification(Notification::success(format!("Backup erstellt: {}", info.path.display())), cx);
                cx.reveal_path(&info.path);
            }
            Err(err) => window.push_notification(Notification::error(format!("Backup fehlgeschlagen: {err}")), cx),
        }
    }

    pub(crate) fn show_history(&mut self, _: &ShowHistory, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.active_buffer().and_then(|b| b.note_id.clone()) else {
            window.push_notification(Notification::info("Versionsverlauf gibt es für Notizen."), cx);
            return;
        };
        self.save_note(&id, window, cx);
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        if state.history.as_ref().is_some_and(|(open, _)| *open == id) {
            state.history = None;
        } else {
            let revisions = state.notes.revisions(&id).unwrap_or_default();
            state.history = Some((id, revisions));
        }
        cx.notify();
    }

    fn restore_revision(&mut self, revision_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let Some((note_id, _)) = state.history.clone() else {
            return;
        };
        if let Err(err) = state.notes.restore_revision(revision_id) {
            window.push_notification(Notification::error(format!("Wiederherstellen fehlgeschlagen: {err}")), cx);
            return;
        }
        let Some(document) = state.notes.get(&note_id).ok().flatten() else {
            return;
        };
        for buffer in self.panes.iter().flat_map(|pane| pane.buffers.iter()) {
            if buffer.note_id.as_deref() == Some(note_id.as_str()) {
                buffer.editor.update(cx, |editor, cx| editor.set_value(document.content.clone(), window, cx));
            }
        }
        // set_value meldet eine Änderung → Autosave speichert denselben Stand (no-op).
        window.push_notification(Notification::success("Früherer Stand wiederhergestellt; der vorige liegt im Verlauf."), cx);
        self.refresh_notes(cx);
    }

    /// Verlaufs-Panel für die aktive Notiz (falls eingeblendet).
    pub(crate) fn render_history(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.notes.as_ref()?;
        let (note_id, revisions) = state.history.as_ref()?;
        if self.active_buffer().and_then(|b| b.note_id.as_deref()) != Some(note_id.as_str()) {
            return None;
        }
        let mut list = v_flex()
            .id("history-panel")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .gap_2()
            .bg(cx.theme().sidebar)
            .text_sm()
            .child(div().font_weight(FontWeight::BOLD).child("Versionsverlauf"))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Beim Schreiben wird höchstens alle 10 Minuten ein Stand gesichert, außerdem vor dem Löschen und Wiederherstellen."),
            );
        if revisions.is_empty() {
            list = list.child(div().text_color(cx.theme().muted_foreground).child("Noch keine früheren Stände."));
        }
        for (ix, revision) in revisions.iter().enumerate() {
            let id = revision.id.clone();
            let preview: String = revision.content.chars().take(160).collect();
            list = list.child(
                v_flex()
                    .gap_1()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(relative_time(revision.created_at)))
                            .child(
                                Button::new(("restore-revision", ix))
                                    .xsmall()
                                    .outline()
                                    .label("Wiederherstellen")
                                    .on_click(cx.listener(move |this, _, window, cx| this.restore_revision(&id, window, cx))),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(preview.replace('\n', " ⏎ ")),
                    ),
            );
        }
        Some(list.into_any_element())
    }

    pub(crate) fn toggle_tag_filter(&mut self, tag: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.tag_filter = match &state.tag_filter {
                Some(current) if current.eq_ignore_ascii_case(tag) => None,
                _ => Some(tag.to_string()),
            };
        }
        self.refresh_notes(cx);
    }

    // ---- Ordner ----

    fn select_folder(&mut self, folder: Option<String>, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.folder_filter = folder;
            state.show_trash = false;
        }
        self.refresh_notes(cx);
    }

    /// Neuer Ordner im ausgewählten Ordner (oder oberster Ebene), gleich zum Umbenennen offen.
    fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        match state.notes.create_folder("Neuer Ordner", state.folder_filter.as_deref()) {
            Ok(folder) => {
                state.folder_filter = Some(folder.id.clone());
                self.refresh_notes(cx);
                self.start_rename_folder(folder.id, window, cx);
            }
            Err(err) => window.push_notification(Notification::error(format!("Ordner konnte nicht angelegt werden: {err}")), cx),
        }
    }

    fn start_rename_folder(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self
            .notes
            .as_ref()
            .and_then(|s| s.folders.iter().find(|f| f.id == id))
            .map(|f| f.name.clone())
        else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        let subscription = cx.subscribe_in(&input, window, |this: &mut Workspace, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.finish_rename_folder(cx);
            }
        });
        if let Some(state) = self.notes.as_mut() {
            state.renaming = Some((id, input, subscription));
        }
        cx.notify();
    }

    fn finish_rename_folder(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let Some((id, input, _)) = state.renaming.take() else {
            return;
        };
        let name = input.read(cx).value().trim().to_string();
        if !name.is_empty() {
            state.notes.rename_folder(&id, &name).ok();
        }
        self.refresh_notes(cx);
    }

    /// Ordner löschen; Notizen und Unterordner rücken eine Ebene nach oben.
    fn delete_folder(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let parent = state.folders.iter().find(|f| f.id == id).and_then(|f| f.parent_id.clone());
        match state.notes.delete_folder(id) {
            Ok(_) => {
                state.folder_filter = parent;
                window.push_notification(
                    Notification::info("Ordner gelöscht – Notizen und Unterordner liegen jetzt eine Ebene höher."),
                    cx,
                );
            }
            Err(err) => window.push_notification(Notification::error(format!("Ordner konnte nicht gelöscht werden: {err}")), cx),
        }
        self.refresh_notes(cx);
    }

    fn move_note(&mut self, id: &str, folder: Option<&str>, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.notes.move_to_folder(id, folder).ok();
        }
        self.refresh_notes(cx);
    }

    // ---- Darstellung ----

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (trash, favorites) = self.notes.as_ref().map(|s| (s.show_trash, s.favorites_only)).unwrap_or_default();
        let current = match self.sidebar {
            SidebarMode::Files => 0,
            SidebarMode::Notes if trash => 3,
            SidebarMode::Notes if favorites => 2,
            SidebarMode::Notes => 1,
        };
        let entries = [
            (AssetIcon::FolderTree, "Dateien"),
            (AssetIcon::NotebookPen, "Notizen"),
            (AssetIcon::Star, "Favoriten"),
            (AssetIcon::Trash, "Papierkorb"),
        ];
        let mut nav = v_flex().p_2().gap_0p5().border_b_1().border_color(cx.theme().border);
        for (ix, (icon, label)) in entries.into_iter().enumerate() {
            // Ohne Datenbank gibt es nur Dateien.
            if ix > 0 && self.notes.is_none() {
                break;
            }
            nav = nav.child(
                h_flex()
                    .id(("nav", ix))
                    .role(Role::Button)
                    .aria_label(label)
                    .aria_selected(current == ix)
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .text_sm()
                    .when(current == ix, |this| this.bg(cx.theme().accent).font_weight(FontWeight::MEDIUM))
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(Icon::new(icon).small().text_color(cx.theme().muted_foreground))
                    .child(label)
                    .on_click(cx.listener(move |this, _, window, cx| match ix {
                        0 => this.show_files(&ShowFiles, window, cx),
                        1 => this.show_note_view(false, false, cx),
                        2 => this.show_note_view(false, true, cx),
                        _ => this.show_note_view(true, false, cx),
                    })),
            );
        }
        let body = match self.sidebar {
            SidebarMode::Files => self.render_file_tree(cx).into_any_element(),
            SidebarMode::Notes => self.render_notes_list(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(nav)
            .child(div().flex_1().min_h_0().child(body))
    }

    fn render_notes_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(state) = self.notes.as_ref() else {
            return v_flex()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Notizen sind nicht verfügbar (Datenbank konnte nicht geöffnet werden).")
                .into_any_element();
        };
        let active_note = self.active_buffer().and_then(|b| b.note_id.clone());
        let tree = folder_tree(&state.folders);
        // Ziele für „In Ordner verschieben“ (eingerückt wie der Baum).
        let move_targets: Rc<Vec<(Option<String>, String)>> = Rc::new(if tree.is_empty() {
            Vec::new()
        } else {
            std::iter::once((None, "Kein Ordner".to_string()))
                .chain(tree.iter().map(|(depth, f)| (Some(f.id.clone()), format!("{}{}", "   ".repeat(*depth), f.name))))
                .collect()
        });
        let mut list = v_flex().id("notes-list").flex_1().min_h_0().overflow_y_scroll().p_1().gap_0p5();
        if state.rows.is_empty() {
            let hint = if state.show_trash {
                "Der Papierkorb ist leer."
            } else if !state.search.read(cx).value().trim().is_empty() {
                "Nichts gefunden."
            } else if state.favorites_only {
                "Noch keine Favoriten. Der Stern an einer Notiz markiert sie."
            } else if state.folder_filter.is_some() {
                "Dieser Ordner ist leer. Der Stift neben der Suche legt hier eine Notiz an."
            } else {
                "Noch keine Notizen. Der Stift neben der Suche oder Cmd-Shift-N legt eine an."
            };
            list = list.child(div().p_2().text_sm().text_color(cx.theme().muted_foreground).child(hint));
        }
        for (ix, row) in state.rows.iter().enumerate() {
            let id = row.id.clone();
            let selected = active_note.as_deref() == Some(row.id.as_str());
            let actions = if state.show_trash {
                h_flex()
                    .gap_0p5()
                    .child(
                        Button::new(("restore", ix))
                            .ghost()
                            .xsmall()
                            .label("Wiederherstellen")
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| this.restore_note(&id, cx)
                            })),
                    )
                    .child(
                        Button::new(("purge", ix))
                            .ghost()
                            .xsmall()
                            .label("Löschen")
                            .tooltip("Endgültig löschen")
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| this.purge_note(&id, cx)
                            })),
                    )
            } else {
                h_flex()
                    .gap_0p5()
                    .when(!move_targets.is_empty(), |this| {
                        let workspace = cx.entity().downgrade();
                        let targets = move_targets.clone();
                        let (id, current) = (id.clone(), row.folder_id.clone());
                        this.child(
                            Button::new(("move", ix))
                                .ghost()
                                .xsmall()
                                .icon(AssetIcon::FolderInput)
                                .tooltip("In Ordner verschieben")
                                .dropdown_menu(move |mut menu, _, _| {
                                    for (folder, label) in targets.iter() {
                                        let (workspace, id, folder) = (workspace.clone(), id.clone(), folder.clone());
                                        menu = menu.item(
                                            PopupMenuItem::new(label.clone()).checked(folder == current).on_click(
                                                move |_, _, cx| {
                                                    workspace
                                                        .update(cx, |this, cx| this.move_note(&id, folder.as_deref(), cx))
                                                        .ok();
                                                },
                                            ),
                                        );
                                    }
                                    menu
                                }),
                        )
                    })
                    .child(
                        Button::new(("favorite", ix))
                            .ghost()
                            .xsmall()
                            .icon(if row.favorite { AssetIcon::StarFill } else { AssetIcon::Star })
                            .tooltip(if row.favorite { "Favorit entfernen" } else { "Als Favorit markieren" })
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| this.toggle_favorite(&id, cx)
                            })),
                    )
                    .child(
                        Button::new(("trash", ix))
                            .ghost()
                            .xsmall()
                            .icon(AssetIcon::Trash)
                            .tooltip("In den Papierkorb")
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, window, cx| this.trash_note(&id, window, cx)
                            })),
                    )
            };
            // Aktionen nur bei Mauszeiger oder Auswahl, im Papierkorb immer.
            let group = SharedString::from(format!("note-row-{ix}"));
            let actions = div()
                .absolute()
                .top_1()
                .right_1()
                .rounded(cx.theme().radius)
                .bg(cx.theme().accent)
                .when(!selected && !state.show_trash, |this| {
                    this.invisible().group_hover(group.clone(), |this| this.visible())
                })
                .child(actions);
            list = list.child(
                v_flex()
                    .id(("note", ix))
                    .group(group)
                    .relative()
                    .role(Role::ListItem)
                    .aria_label(row.title.clone())
                    .aria_selected(selected)
                    .w_full()
                    .gap_0p5()
                    .px_3()
                    .py_2()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .bg(if selected { cx.theme().accent } else { cx.theme().transparent })
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(row.title.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(row.time.clone()),
                            ),
                    )
                    .when(!row.preview.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .line_clamp(2)
                                .child(row.preview.clone()),
                        )
                    })
                    .child(actions)
                    .when(!state.show_trash, |this| {
                        this.on_click(cx.listener(move |this, _, window, cx| this.open_note(id.clone(), window, cx)))
                    }),
            );
        }
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .p_2()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().flex_1().min_w_0().child(Input::new(&state.search).small().cleanable(true)))
                            .child(
                                Button::new("new-note")
                                    .small()
                                    .ghost()
                                    .icon(AssetIcon::SquarePen)
                                    .tooltip_with_action("Neue Notiz", &NewNote, None)
                                    .on_click(cx.listener(|this, _, window, cx| this.new_note(&NewNote, window, cx))),
                            ),
                    )
                    .when(!state.tags.is_empty() && !state.show_trash, |this| {
                        let mut chips = h_flex().flex_wrap().gap_1();
                        for (ix, (tag, count)) in state.tags.iter().enumerate() {
                            let selected = state.tag_filter.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(tag));
                            let tag = tag.clone();
                            chips = chips.child(
                                Button::new(("tag", ix))
                                    .xsmall()
                                    .ghost()
                                    .selected(selected)
                                    .label(format!("#{tag} {count}"))
                                    .tooltip("Nach Tag filtern (nochmals klicken: alle)")
                                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_tag_filter(&tag, cx))),
                            );
                        }
                        this.child(chips)
                    })
                    .when(!state.show_trash, |this| this.child(self.render_folders(state, &tree, cx))),
            )
            .child(list)
            .when_some(active_note.filter(|_| !state.show_trash), |this, note| {
                this.children(self.render_backlinks(state, &note, cx))
            })
            .into_any_element()
    }

    fn render_folders(&self, state: &NotesState, tree: &[(usize, &Folder)], cx: &mut Context<Self>) -> impl IntoElement {
        let row = |id: ElementId, depth: usize, selected: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .role(Role::ListItem)
                .aria_selected(selected)
                .w_full()
                .gap_1p5()
                .pl(px(8. + 14. * depth as f32))
                .pr_1()
                .py_0p5()
                .min_h(px(26.))
                .rounded(cx.theme().radius)
                .cursor_pointer()
                .text_sm()
                .bg(if selected { cx.theme().accent } else { cx.theme().transparent })
                .hover(|this| this.bg(cx.theme().accent))
        };
        let mut folders = v_flex()
            .gap_0p5()
            .child(
                h_flex()
                    .justify_between()
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Ordner"))
                    .child(
                        Button::new("new-folder")
                            .ghost()
                            .xsmall()
                            .icon(AssetIcon::FolderPlus)
                            .tooltip(if state.folder_filter.is_some() {
                                "Neuer Unterordner im ausgewählten Ordner"
                            } else {
                                "Neuer Ordner"
                            })
                            .on_click(cx.listener(|this, _, window, cx| this.new_folder(window, cx))),
                    ),
            )
            .child(
                row("folder-all".into(), 0, state.folder_filter.is_none(), cx)
                    .child(IconName::Inbox)
                    .child("Alle Notizen")
                    .on_click(cx.listener(|this, _, _, cx| this.select_folder(None, cx))),
            );
        for (ix, (depth, folder)) in tree.iter().enumerate() {
            let selected = state.folder_filter.as_deref() == Some(folder.id.as_str());
            let renaming = state.renaming.as_ref().filter(|(id, _, _)| *id == folder.id);
            let id = folder.id.clone();
            let mut item = row(("folder", ix).into(), *depth, selected, cx)
                .child(if selected { IconName::FolderOpen } else { IconName::Folder });
            item = match renaming {
                Some((_, input, _)) => item.child(div().flex_1().child(Input::new(input).xsmall())),
                None => item
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(folder.name.clone()),
                    )
                    .when(selected, |this| {
                        this.child(
                            Button::new(("rename-folder", ix))
                                .ghost()
                                .xsmall()
                                .icon(AssetIcon::Pencil)
                                .tooltip("Umbenennen")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, window, cx| this.start_rename_folder(id.clone(), window, cx)
                                })),
                        )
                        .child(
                            Button::new(("delete-folder", ix))
                                .ghost()
                                .xsmall()
                                .icon(AssetIcon::Trash)
                                .tooltip("Ordner löschen (Notizen bleiben erhalten)")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, window, cx| {
                                        // Sonst wählt der Klick danach die Zeile des gelöschten Ordners.
                                        cx.stop_propagation();
                                        this.delete_folder(&id, window, cx)
                                    }
                                })),
                        )
                    }),
            };
            folders = folders.child(item.on_click(cx.listener(move |this, _, _, cx| this.select_folder(Some(id.clone()), cx))));
        }
        folders
    }

    /// Notizen, die auf die aktive Notiz verweisen.
    fn render_backlinks(&self, state: &NotesState, note: &str, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let backlinks = state.notes.backlinks(note).ok().filter(|b| !b.is_empty())?;
        let mut section = v_flex()
            .id("backlinks")
            .max_h(px(180.))
            .overflow_y_scroll()
            .p_2()
            .gap_0p5()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Verweise auf diese Notiz"));
        for (ix, document) in backlinks.iter().enumerate() {
            let id = document.id.clone();
            section = section.child(
                h_flex()
                    .id(("backlink", ix))
                    .gap_1p5()
                    .px_2()
                    .py_0p5()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .text_sm()
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(IconName::ArrowLeft)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(document.display_title()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.open_note(id.clone(), window, cx))),
            );
        }
        Some(section)
    }
}

/// Ordner als Baum in Anzeigereihenfolge: (Tiefe, Ordner). Verwaiste Ordner (Eltern gelöscht,
/// z. B. nach Sync) erscheinen auf oberster Ebene.
fn folder_tree(folders: &[Folder]) -> Vec<(usize, &Folder)> {
    fn add<'a>(folders: &'a [Folder], parent: Option<&str>, depth: usize, out: &mut Vec<(usize, &'a Folder)>) {
        for folder in folders.iter().filter(|f| f.parent_id.as_deref() == parent) {
            if out.iter().any(|(_, f)| f.id == folder.id) {
                continue;
            }
            out.push((depth, folder));
            add(folders, Some(&folder.id), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    add(folders, None, 0, &mut out);
    for folder in folders {
        if !out.iter().any(|(_, f)| f.id == folder.id) {
            out.push((0, folder));
            add(folders, Some(&folder.id), 1, &mut out);
        }
    }
    out
}

/// Erste Inhaltszeile nach dem Titel als Vorschau.
fn preview_text(document: &Document) -> String {
    let title = document.display_title();
    devmark_notes::strip_note_links(&document.content)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "---")
        .map(|l| l.trim_start_matches(['#', '=', '-', '*', '>']).trim())
        .filter(|l| !l.is_empty() && *l != title)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}

/// „gerade eben“, „vor 5 Min.“, „vor 3 Std.“, „vor 2 Tagen“ bzw. Datum.
pub(crate) fn relative_time(ms: i64) -> String {
    let diff = (devmark_storage_now() - ms).max(0) / 1000;
    match diff {
        0..=59 => "gerade eben".into(),
        60..=3599 => format!("vor {} Min.", diff / 60),
        3600..=86_399 => format!("vor {} Std.", diff / 3600),
        86_400..=172_799 => "gestern".into(),
        172_800..=604_799 => format!("vor {} Tagen", diff / 86_400),
        _ => crate::date_from_ms(ms),
    }
}

fn devmark_storage_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Folder, folder_tree};

    fn folder(id: &str, parent: Option<&str>) -> Folder {
        Folder { id: id.into(), parent_id: parent.map(Into::into), name: id.into() }
    }

    #[test]
    fn folder_tree_nests_and_keeps_orphans() {
        let folders = [folder("a", None), folder("b", None), folder("a1", Some("a")), folder("x", Some("weg"))];
        let tree: Vec<(usize, &str)> = folder_tree(&folders).into_iter().map(|(d, f)| (d, f.id.as_str())).collect();
        assert_eq!(tree, [(0, "a"), (1, "a1"), (0, "b"), (0, "x")]);
    }
}
