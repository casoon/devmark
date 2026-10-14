//! Notes-Modus: Notizen aus der SQLite-Datenbank – sofort schreiben, Autosave, Suche,
//! Favoriten, Papierkorb, Export und tägliches Backup.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use devmark_notes::{Document, Filter, Notes, Revision, create_backup, export_markdown, extract_hashtags, list_backups};
use devmark_writing::authoring::Syntax;
use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::{
    ActiveTheme, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    v_flex,
};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;

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
    pub subtitle: String,
    pub favorite: bool,
}

pub struct NotesState {
    pub notes: Notes,
    pub data_dir: PathBuf,
    pub search: Entity<InputState>,
    pub rows: Vec<NoteRow>,
    pub show_trash: bool,
    pub save_tasks: HashMap<String, Task<()>>,
    /// Alle Tags mit Anzahl; `tag_filter` schränkt die Liste ein.
    pub tags: Vec<(String, usize)>,
    pub tag_filter: Option<String>,
    /// Versionsverlauf der Notiz, falls eingeblendet.
    pub history: Option<(String, Vec<Revision>)>,
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
            save_tasks: HashMap::new(),
            tags: Vec::new(),
            tag_filter: None,
            history: None,
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
    }

    pub(crate) fn refresh_notes(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.notes.as_mut() else {
            return;
        };
        let query = state.search.read(cx).value().trim().to_string();
        let rows = if !query.is_empty() && !state.show_trash {
            state.notes.search(&query, 200).map(|hits| {
                hits.into_iter()
                    .map(|hit| NoteRow {
                        favorite: false,
                        id: hit.id,
                        title: hit.title,
                        subtitle: hit.snippet.replace(['\n', '\r'], " "),
                    })
                    .collect()
            })
        } else {
            let filter = Filter {
                trash: state.show_trash,
                tag: state.tag_filter.clone().filter(|_| !state.show_trash),
                ..Default::default()
            };
            state.notes.list(&filter).map(|documents| {
                documents
                    .iter()
                    .filter(|d| query.is_empty() || d.display_title().to_lowercase().contains(&query.to_lowercase()))
                    .map(|d| NoteRow {
                        id: d.id.clone(),
                        title: d.display_title(),
                        subtitle: format!("{} · {}", relative_time(d.updated_at), preview_line(d)),
                        favorite: d.favorite,
                    })
                    .collect()
            })
        };
        state.rows = rows.unwrap_or_default();
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
        match state.notes.create_note(content) {
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
        self.open_buffer(path, document.content, Some((document.id, title)), window, cx);
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
        if let Some(state) = self.notes.as_mut() {
            state.show_trash = !state.show_trash;
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

    fn toggle_tag_filter(&mut self, tag: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.notes.as_mut() {
            state.tag_filter = match &state.tag_filter {
                Some(current) if current.eq_ignore_ascii_case(tag) => None,
                _ => Some(tag.to_string()),
            };
        }
        self.refresh_notes(cx);
    }

    // ---- Darstellung ----

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let switcher = h_flex()
            .gap_1()
            .p_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("mode-files")
                    .ghost()
                    .xsmall()
                    .label("Dateien")
                    .selected(self.sidebar == SidebarMode::Files)
                    .tooltip_with_action("Dateien des Ordners", &ShowFiles, None)
                    .on_click(cx.listener(|this, _, window, cx| this.show_files(&ShowFiles, window, cx))),
            )
            .child(
                Button::new("mode-notes")
                    .ghost()
                    .xsmall()
                    .label("Notizen")
                    .selected(self.sidebar == SidebarMode::Notes)
                    .tooltip_with_action("Notizen durchsuchen und schreiben", &ShowNotes, None)
                    .on_click(cx.listener(|this, _, window, cx| this.show_notes(&ShowNotes, window, cx))),
            );
        let body = match self.sidebar {
            SidebarMode::Files => self.render_file_tree(cx).into_any_element(),
            SidebarMode::Notes => self.render_notes_list(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(switcher)
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
        let mut list = v_flex().id("notes-list").flex_1().min_h_0().overflow_y_scroll().p_1().gap_0p5();
        if state.rows.is_empty() {
            let hint = if state.show_trash {
                "Der Papierkorb ist leer."
            } else if !state.search.read(cx).value().trim().is_empty() {
                "Nichts gefunden."
            } else {
                "Noch keine Notizen. „Neu“ oder Cmd-Shift-N legt eine an."
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
            list = list.child(
                v_flex()
                    .id(("note", ix))
                    .role(Role::ListItem)
                    .aria_label(row.title.clone())
                    .aria_selected(selected)
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .bg(if selected { cx.theme().accent } else { cx.theme().transparent })
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap_1()
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
                            .child(actions),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(row.subtitle.clone()),
                    )
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
                    .child(Input::new(&state.search).small().cleanable(true))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("new-note")
                                    .small()
                                    .icon(AssetIcon::Plus)
                                    .label("Neu")
                                    .tooltip_with_action("Neue Notiz", &NewNote, None)
                                    .on_click(cx.listener(|this, _, window, cx| this.new_note(&NewNote, window, cx))),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("show-trash")
                                    .ghost()
                                    .small()
                                    .icon(AssetIcon::Trash)
                                    .label("Papierkorb")
                                    .selected(state.show_trash)
                                    .tooltip_with_action("Papierkorb anzeigen", &ToggleTrash, None)
                                    .on_click(cx.listener(|this, _, window, cx| this.toggle_trash(&ToggleTrash, window, cx))),
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
                    }),
            )
            .child(list)
            .into_any_element()
    }
}

/// Erste Inhaltszeile nach dem Titel als Vorschau.
fn preview_line(document: &Document) -> String {
    let title = document.display_title();
    document
        .content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "---")
        .map(|l| l.trim_start_matches(['#', '=', '-', '*', '>']).trim())
        .find(|l| !l.is_empty() && *l != title)
        .unwrap_or("")
        .chars()
        .take(80)
        .collect()
}

/// „gerade eben“, „vor 5 Min.“, „vor 3 Std.“, „vor 2 Tagen“ bzw. Datum.
fn relative_time(ms: i64) -> String {
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
