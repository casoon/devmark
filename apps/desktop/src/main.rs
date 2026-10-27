//! DevMark: Dateibaum, Editor mit Tree-sitter-Highlighting, Tabs, Split, Speichern;
//! Schreibhilfen für Markdown/Typst und Export nach Typst/PDF.

mod assist;
mod notes_ui;
mod sync_ui;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use devmark_writing::authoring::{self, Action as Format, Syntax, Tabstops};
use devmark_writing::templates::{DocInfo, DocType};
use devmark_writing::{Options, compile_pdf, markdown_to_typst, render_pages};
use gpui_kit::assets::{Assets, IconName as AssetIcon, icon_assets};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable, Theme, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState, TabSize},
    list::ListItem,
    menu::{DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    tree::{TreeEvent, TreeItem, TreeState, tree},
    v_flex,
};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;

use crate::assist::WritingAssist;
use crate::notes_ui::{NotesState, SidebarMode};
use crate::sync_ui::SyncSettings;

actions!(
    devmark,
    [
        Open,
        OpenWorkspace,
        Save,
        ToggleSplit,
        CloseTab,
        Quit,
        NewDocument,
        ExportTypst,
        ExportPdf,
        ToggleHelp,
        TogglePreview,
        NewNote,
        SelectionToNote,
        ExportNotes,
        BackupNow,
        ShowFiles,
        ShowNotes,
        ToggleTrash,
        ShowHistory,
        SyncNow,
        ChooseSyncFolder,
        DisableSync,
        FormatBold,
        FormatItalic,
        FormatStrike,
        FormatCode,
        InsertLink,
        Heading1,
        Heading2,
        Heading3,
        BulletList,
        NumberedList,
        TaskList,
        QuoteBlock,
        InsertImage,
        InsertTable,
        InsertCodeBlock,
        InsertFootnote,
        InsertCallout,
        InsertMath,
        InsertMathBlock,
        InsertRule,
        InsertPageBreak,
        InsertOutline,
        NextPlaceholder,
        PrevPlaceholder,
        EndPlaceholders,
    ]
);

// Zusätzliche Icons für die Formatleiste (Standard-Assets enthalten nur Basis-Icons).
icon_assets!(
    WritingIcons,
    [
        Bold,
        Italic,
        Strikethrough,
        Code,
        Link,
        Heading1,
        Heading2,
        Heading3,
        List,
        ListOrdered,
        ListTodo,
        TextQuote,
        Image,
        Table,
        SquareCode,
        Footprints,
        MessageSquareWarning,
        Radical,
        Sigma,
        SeparatorHorizontal,
        FileDown,
        TableOfContents,
        CircleQuestionMark,
        FilePlus,
        Printer,
        FileType,
        Trash,
        RotateCcwClock,
        FolderPlus,
        FolderInput,
        Pencil,
        FolderTree,
        NotebookPen,
        FolderOpen,
        ChevronsUpDown,
        FileSymlink,
    ]
);

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(bytes) = WritingIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(WritingIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

/// Formatleiste: Aktion, Icon, UI-Action (für Tastenkürzel im Tooltip).
fn format_entries() -> Vec<(Format, AssetIcon, Box<dyn gpui_kit::Action>)> {
    vec![
        (Format::Heading(1), AssetIcon::Heading1, Box::new(Heading1)),
        (Format::Heading(2), AssetIcon::Heading2, Box::new(Heading2)),
        (Format::Heading(3), AssetIcon::Heading3, Box::new(Heading3)),
        (Format::Bold, AssetIcon::Bold, Box::new(FormatBold)),
        (Format::Italic, AssetIcon::Italic, Box::new(FormatItalic)),
        (Format::Strike, AssetIcon::Strikethrough, Box::new(FormatStrike)),
        (Format::Code, AssetIcon::Code, Box::new(FormatCode)),
        (Format::Link, AssetIcon::Link, Box::new(InsertLink)),
        (Format::BulletList, AssetIcon::List, Box::new(BulletList)),
        (Format::NumberedList, AssetIcon::ListOrdered, Box::new(NumberedList)),
        (Format::TaskList, AssetIcon::ListTodo, Box::new(TaskList)),
        (Format::Quote, AssetIcon::TextQuote, Box::new(QuoteBlock)),
        (Format::Image, AssetIcon::Image, Box::new(InsertImage)),
        (Format::Table, AssetIcon::Table, Box::new(InsertTable)),
        (Format::CodeBlock, AssetIcon::SquareCode, Box::new(InsertCodeBlock)),
        (Format::Footnote, AssetIcon::Footprints, Box::new(InsertFootnote)),
        (Format::Callout, AssetIcon::MessageSquareWarning, Box::new(InsertCallout)),
        (Format::MathInline, AssetIcon::Radical, Box::new(InsertMath)),
        (Format::MathBlock, AssetIcon::Sigma, Box::new(InsertMathBlock)),
        (Format::Rule, AssetIcon::SeparatorHorizontal, Box::new(InsertRule)),
        (Format::PageBreak, AssetIcon::FileDown, Box::new(InsertPageBreak)),
        (Format::Outline, AssetIcon::TableOfContents, Box::new(InsertOutline)),
    ]
}

const IGNORED_DIRS: &[&str] = &[".git", "target", "node_modules", "build", ".dart_tool"];

struct Buffer {
    /// Dateipfad; bei Notizen ein virtueller Pfad im Datenordner (für relative Bilder, Vorschau).
    path: PathBuf,
    editor: Entity<EditorState>,
    syntax: Option<Syntax>,
    dirty: bool,
    /// Notiz aus der Datenbank statt Datei (Autosave, kein Dateiname).
    note_id: Option<String>,
    /// Anzeigename für Notizen (erste Zeile).
    title: Option<String>,
    /// Schreibhilfe (nur Markdown/Typst), u. a. für übernommene `/`-Einfügungen.
    assist: Option<Rc<WritingAssist>>,
    /// Platzhalter der zuletzt eingefügten Vorlage, solange Tab sie anspringt.
    tabstops: Option<Tabstops>,
    /// Letztes Speichern (ms seit 1970): Datei-Änderungszeit bzw. Notiz-Stand.
    saved_at: Option<i64>,
    /// Wortzahl, Gliederung, Tags – nur bei Markdown/Typst.
    stats: DocStats,
}

/// Überschrift für die Gliederung am rechten Rand.
struct Heading {
    line: u32,
    level: usize,
    text: String,
}

/// Wortzahl, Überschriften und `#tags` eines Textes; neu berechnet bei jeder Änderung.
#[derive(Default)]
struct DocStats {
    words: usize,
    headings: Vec<Heading>,
    tags: Vec<String>,
}

impl DocStats {
    fn of(syntax: Syntax, text: &str, with_tags: bool) -> Self {
        let marker = match syntax {
            Syntax::Markdown => '#',
            Syntax::Typst => '=',
        };
        let mut headings = Vec::new();
        let mut in_code = false;
        for (ix, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
                continue;
            }
            let level = line.chars().take_while(|&c| c == marker).count();
            if in_code || !(1..=6).contains(&level) || !line[level..].starts_with(' ') {
                continue;
            }
            let text = line[level..].trim().trim_end_matches(marker).trim();
            if !text.is_empty() {
                headings.push(Heading { line: ix as u32, level, text: text.to_string() });
            }
        }
        Self {
            words: text.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count(),
            headings,
            tags: if with_tags { devmark_notes::extract_hashtags(text) } else { Vec::new() },
        }
    }
}

/// Textspalte im Schreib-Layout: rund 70 Zeichen.
const WRITING_COLUMN: f32 = 720.;
const WRITING_FONT_SIZE: f32 = 17.;

/// Lesefreundliche Schrift für Fließtext; Systemschrift, wo keine Serifenschrift sicher da ist.
fn writing_font(cx: &App) -> SharedString {
    if cfg!(target_os = "macos") {
        "Charter".into()
    } else if cfg!(target_os = "windows") {
        "Georgia".into()
    } else {
        cx.theme().font_family.clone()
    }
}

impl Buffer {
    fn label(&self) -> String {
        self.title.clone().unwrap_or_else(|| file_name(&self.path))
    }
}

#[derive(Default)]
struct Pane {
    buffers: Vec<Buffer>,
    active: usize,
}

/// Formular „Neues Dokument“: Dokumenttyp, Format, Titel, Autor.
struct NewDocForm {
    doc_type: DocType,
    syntax: Syntax,
    title: Entity<InputState>,
    author: Entity<InputState>,
}

/// Live-Vorschau: gerenderte Seiten des aktiven Markdown-/Typst-Dokuments.
#[derive(Default)]
struct Preview {
    path: Option<PathBuf>,
    /// Seite + Seitenverhältnis (Breite/Höhe).
    pages: Vec<(Arc<Image>, f32)>,
    total_pages: usize,
    error: Option<String>,
}

/// Vorschau-Auflösung: 1,5 Pixel pro Punkt ist auf Retina scharf genug und schnell.
const PREVIEW_PIXEL_PER_PT: f64 = 1.5;
const PREVIEW_MAX_PAGES: usize = 60;

/// Dateibaum, der Ordner erst beim Aufklappen liest – große oder geschützte Ordner
/// (macOS: Dokumente, Schreibtisch) blockieren so den Start nicht.
#[derive(Default)]
struct FileTree {
    listings: HashMap<PathBuf, Vec<(PathBuf, String, bool)>>,
    expanded: HashSet<PathBuf>,
}

/// Kennzeichen für Platzhalter-Einträge in noch nicht gelesenen Ordnern.
const PLACEHOLDER_SUFFIX: &str = "\u{0}placeholder";

impl FileTree {
    fn listing(&mut self, dir: &Path) -> &[(PathBuf, String, bool)] {
        self.listings.entry(dir.to_path_buf()).or_insert_with(|| list_dir(dir))
    }

    fn items(&mut self, dir: &Path) -> Vec<TreeItem> {
        let entries = self.listing(dir).to_vec();
        entries
            .into_iter()
            .map(|(path, name, is_dir)| {
                let id = path.to_string_lossy().to_string();
                if !is_dir {
                    return TreeItem::new(id, name);
                }
                let expanded = self.expanded.contains(&path);
                let mut children = if expanded { self.items(&path) } else { Vec::new() };
                if children.is_empty() {
                    // Ohne Kind wäre der Eintrag kein Ordner mehr.
                    let label = if expanded { "(leer)" } else { "…" };
                    children.push(TreeItem::new(format!("{id}{PLACEHOLDER_SUFFIX}"), label).disabled(true));
                }
                TreeItem::new(id, name).children(children).expanded(expanded)
            })
            .collect()
    }
}

struct Workspace {
    /// Hauptordner; der Dateibaum zeigt nur ihn. `None`: noch keiner gewählt.
    root: Option<PathBuf>,
    tree_state: Entity<TreeState>,
    file_tree: FileTree,
    panes: Vec<Pane>,
    active_pane: usize,
    split: bool,
    show_help: bool,
    show_preview: bool,
    preview: Preview,
    preview_task: Task<()>,
    new_doc: Option<NewDocForm>,
    sidebar: SidebarMode,
    notes: Option<NotesState>,
    sync: SyncSettings,
    sync_timer: Task<()>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    fn new(
        root: Option<PathBuf>,
        file: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));
        let mut file_tree = FileTree::default();
        if let Some(root) = &root {
            remember_root(root);
            let items = file_tree.items(root);
            tree_state.update(cx, |state, cx| state.set_items(items, cx));
        }
        let tree_subscription = cx.subscribe(&tree_state, |this: &mut Self, _, event: &TreeEvent, cx| {
            match event {
                TreeEvent::Expanded(id) => {
                    this.file_tree.expanded.insert(PathBuf::from(id.as_str()));
                }
                TreeEvent::Collapsed(id) => {
                    this.file_tree.expanded.remove(&PathBuf::from(id.as_str()));
                    return;
                }
            }
            this.rebuild_tree(cx);
        });

        // Fokus auf dem Workspace, damit Menü-Aktionen auch ohne offene Datei ankommen.
        let focus_handle = cx.focus_handle();
        window.defer(cx, {
            let focus_handle = focus_handle.clone();
            move |window, cx| focus_handle.focus(window, cx)
        });
        if let Some(file) = file {
            cx.defer_in(window, move |this, window, cx| this.open_file(file, window, cx));
        }
        let notes = NotesState::open(window, cx);
        // Ausstehende Notiz-Speicherungen nicht verlieren, wenn kurz nach dem Tippen beendet wird.
        let quit_subscription = cx.on_app_quit(|this: &mut Self, cx| {
            this.save_all_notes(cx);
            async {}
        });
        // Fenster schließen = Beenden (mit Rückfrage bei ungesicherten Dateien).
        window.on_window_should_close(cx, |window, cx| {
            window.dispatch_action(Box::new(Quit), cx);
            false
        });
        cx.defer_in(window, |this, window, cx| this.after_start(window, cx));

        Self {
            focus_handle,
            root,
            tree_state,
            file_tree,
            panes: vec![Pane::default(), Pane::default()],
            active_pane: 0,
            split: false,
            show_help: false,
            show_preview: false,
            preview: Preview::default(),
            preview_task: Task::ready(()),
            new_doc: None,
            sidebar: SidebarMode::Files,
            notes,
            sync: SyncSettings::load(),
            sync_timer: Task::ready(()),
            _subscriptions: vec![tree_subscription, quit_subscription],
        }
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some("Öffnen".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                if path.is_dir() {
                    this.set_root(path, window, cx);
                } else {
                    this.open_file(path, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Ordner als Workspace wählen.
    fn open_workspace(&mut self, _: &OpenWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Als Workspace öffnen".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| this.set_root(path, window, cx)).ok();
        })
        .detach();
    }

    /// Ordner wird Workspace, Datei wird geöffnet – die Wurzel bleibt dabei, wo sie ist.
    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if path.is_dir() {
            self.set_root(path, window, cx);
        } else {
            self.open_file(path, window, cx);
        }
    }

    fn set_root(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        window.set_window_title(&window_title(Some(&root)));
        remember_root(&root);
        self.file_tree = FileTree::default();
        self.root = Some(root);
        self.sidebar = SidebarMode::Files;
        self.refresh_tree(cx);
    }

    /// Datei liegt nicht im Workspace (Notizen zählen nicht).
    fn is_outside(&self, buffer: &Buffer) -> bool {
        buffer.note_id.is_none() && !self.root.as_ref().is_some_and(|root| buffer.path.starts_with(root))
    }

    /// Ordnerinhalte neu einlesen (aufgeklappte Ordner bleiben offen).
    fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        self.file_tree.listings.clear();
        self.rebuild_tree(cx);
    }

    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let items = match &self.root {
            Some(root) => self.file_tree.items(root),
            None => Vec::new(),
        };
        self.tree_state
            .update(cx, |state, cx| state.set_items(items, cx));
        cx.notify();
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.new_doc = None;
        let pane_ix = self.active_pane;
        if let Some(ix) = self.panes[pane_ix].buffers.iter().position(|b| b.path == path) {
            self.panes[pane_ix].active = ix;
            self.schedule_preview(Duration::ZERO, cx);
            cx.notify();
            return;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) => {
                let name = file_name(&path);
                let message = if err.kind() == std::io::ErrorKind::InvalidData {
                    format!("„{name}“ ist keine Textdatei und kann nicht geöffnet werden.")
                } else {
                    format!("„{name}“ konnte nicht geöffnet werden: {err}")
                };
                window.push_notification(Notification::error(message), cx);
                return;
            }
        };
        self.open_buffer(path, content, None, window, cx);
    }

    /// Neuer Tab mit Editor für Datei- oder Notizinhalt.
    fn open_buffer(
        &mut self,
        path: PathBuf,
        content: String,
        note: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane_ix = self.active_pane;
        let language = language_for(&path);
        let syntax = Syntax::for_path(&path);

        // `[[…]]`-Verknüpfungen in Markdown, sofern Notizen verfügbar sind.
        let note_index = self.notes.as_ref().map(|s| s.index.clone()).filter(|_| syntax == Some(Syntax::Markdown));
        let workspace = cx.entity().downgrade();
        let mut buffer_assist = None;
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language(language)
                // Notizen wie in Apple Notes ohne Zeilennummern.
                .line_number(note.is_none())
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                // Fließtext umbrechen, Quelltext nicht.
                .soft_wrap(syntax.is_some());
            state.set_value(content, window, cx);
            if let Some(syntax) = syntax {
                let links = note_index.is_some();
                let assist = Rc::new(WritingAssist { syntax, notes: note_index, offered: Default::default() });
                buffer_assist = Some(assist.clone());
                state.lsp_mut().completion_provider = Some(assist.clone());
                state.lsp_mut().hover_provider = Some(assist.clone());
                if links {
                    state.lsp_mut().definition_provider = Some(assist);
                    state.lsp_mut().show_document = Some(Rc::new(move |params, window, cx| {
                        let Some(id) = assist::note_id_from_uri(&params.uri) else {
                            return false;
                        };
                        let (id, workspace) = (id.to_string(), workspace.clone());
                        window.defer(cx, move |window, cx| {
                            workspace.update(cx, |this, cx| this.open_note(id, window, cx)).ok();
                        });
                        true
                    }));
                }
            }
            state
        });

        let subscription = cx.subscribe(&editor, |this, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.track_placeholders(&editor, cx);
                let mut note_id = None;
                let text = editor.read(cx).value();
                if let Some(buffer) = this.buffer_for_editor(&editor) {
                    buffer.dirty = true;
                    note_id = buffer.note_id.clone();
                    if let Some(syntax) = buffer.syntax {
                        buffer.stats = DocStats::of(syntax, &text, note_id.is_some());
                    }
                }
                if let Some(note_id) = note_id {
                    this.schedule_note_save(note_id, cx);
                }
                if this.active_buffer().is_some_and(|b| b.editor == editor) {
                    this.schedule_preview(Duration::from_millis(400), cx);
                }
                cx.notify();
            }
        });
        self._subscriptions.push(subscription);

        editor.update(cx, |state, cx| state.focus(window, cx));

        let (note_id, title) = match note {
            Some((id, title)) => (Some(id), Some(title)),
            None => (None, None),
        };
        // Notizen: Stand setzt `open_note` aus der Datenbank.
        let saved_at = note_id.is_none().then(|| file_modified_ms(&path)).flatten();
        let stats = syntax
            .map(|syntax| DocStats::of(syntax, &editor.read(cx).value(), note_id.is_some()))
            .unwrap_or_default();
        let pane = &mut self.panes[pane_ix];
        pane.buffers.push(Buffer {
            path,
            editor,
            syntax,
            dirty: false,
            note_id,
            title,
            assist: buffer_assist,
            tabstops: None,
            saved_at,
            stats,
        });
        pane.active = pane.buffers.len() - 1;
        self.schedule_preview(Duration::ZERO, cx);
        cx.notify();
    }

    fn buffer_for_editor(&mut self, editor: &Entity<EditorState>) -> Option<&mut Buffer> {
        self.panes
            .iter_mut()
            .flat_map(|pane| pane.buffers.iter_mut())
            .find(|buffer| &buffer.editor == editor)
    }

    fn active_buffer(&self) -> Option<&Buffer> {
        let pane = &self.panes[self.active_pane];
        pane.buffers.get(pane.active)
    }

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(note_id) = self.active_buffer().and_then(|b| b.note_id.clone()) {
            self.save_note(&note_id, window, cx);
            return;
        }
        let pane = &mut self.panes[self.active_pane];
        let Some(buffer) = pane.buffers.get_mut(pane.active) else {
            return;
        };
        let content = buffer.editor.read(cx).value();
        match std::fs::write(&buffer.path, content.as_bytes()) {
            Ok(()) => {
                buffer.dirty = false;
                buffer.saved_at = Some(now_ms());
            }
            Err(err) => {
                let message = format!(
                    "„{}“ konnte nicht gespeichert werden: {err}",
                    file_name(&buffer.path)
                );
                window.push_notification(Notification::error(message), cx);
            }
        }
        cx.notify();
    }

    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.save_all_notes(cx);
        let dirty: Vec<String> = self
            .panes
            .iter()
            .flat_map(|pane| pane.buffers.iter())
            .filter(|b| b.dirty && b.note_id.is_none())
            .map(|b| file_name(&b.path))
            .collect();
        if dirty.is_empty() {
            cx.quit();
            return;
        }
        let message = if dirty.len() == 1 {
            format!("„{}“ hat ungesicherte Änderungen.", dirty[0])
        } else {
            format!("{} Dateien haben ungesicherte Änderungen.", dirty.len())
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some("Sollen die Änderungen vor dem Beenden gesichert werden?"),
            &["Sichern", "Nicht sichern", "Abbrechen"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(choice) = answer.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| match choice {
                0 => {
                    if this.save_all_files(window, cx) {
                        cx.quit();
                    }
                }
                1 => cx.quit(),
                _ => {}
            })
            .ok();
        })
        .detach();
    }

    /// Sichert alle geänderten Dateien; `false`, wenn eine nicht gespeichert werden konnte.
    fn save_all_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut ok = true;
        for buffer in self.panes.iter_mut().flat_map(|pane| pane.buffers.iter_mut()) {
            if !buffer.dirty || buffer.note_id.is_some() {
                continue;
            }
            let content = buffer.editor.read(cx).value();
            match std::fs::write(&buffer.path, content.as_bytes()) {
                Ok(()) => {
                    buffer.dirty = false;
                    buffer.saved_at = Some(now_ms());
                }
                Err(err) => {
                    ok = false;
                    window.push_notification(
                        Notification::error(format!("„{}“ konnte nicht gespeichert werden: {err}", file_name(&buffer.path))),
                        cx,
                    );
                }
            }
        }
        cx.notify();
        ok
    }

    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_doc.take().is_some() {
            cx.notify();
            return;
        }
        // Notizen vor dem Schließen sichern (Autosave könnte noch ausstehen).
        if let Some(note_id) = self.active_buffer().filter(|b| b.dirty).and_then(|b| b.note_id.clone()) {
            self.save_note(&note_id, window, cx);
        }
        let pane = &mut self.panes[self.active_pane];
        if pane.buffers.is_empty() {
            return;
        }
        pane.buffers.remove(pane.active);
        pane.active = pane.active.min(pane.buffers.len().saturating_sub(1));
        cx.notify();
    }

    fn toggle_split(&mut self, _: &ToggleSplit, _window: &mut Window, cx: &mut Context<Self>) {
        self.split = !self.split;
        if !self.split {
            self.active_pane = 0;
        }
        cx.notify();
    }

    fn toggle_help(&mut self, _: &ToggleHelp, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_help = !self.show_help;
        cx.notify();
    }

    fn toggle_preview(&mut self, _: &TogglePreview, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_preview = !self.show_preview;
        self.schedule_preview(Duration::ZERO, cx);
        cx.notify();
    }

    /// Setzt das aktive Dokument nach `delay` im Hintergrund neu (vorige Anfrage verfällt).
    fn schedule_preview(&mut self, delay: Duration, cx: &mut Context<Self>) {
        if !self.show_preview {
            return;
        }
        let Some(buffer) = self.active_buffer() else {
            return;
        };
        let Some(syntax) = buffer.syntax else {
            return;
        };
        let mut source = buffer.editor.read(cx).value().to_string();
        if buffer.note_id.is_some() {
            source = devmark_notes::strip_note_links(&source);
        }
        let path = buffer.path.clone();
        self.preview_task = cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let render_path = path.clone();
            let result = cx
                .background_spawn(async move {
                    let (typst, main_path) = match syntax {
                        Syntax::Markdown => (
                            markdown_to_typst(&source, &Options::for_file(&render_path)).typst,
                            render_path.with_extension("typ"),
                        ),
                        Syntax::Typst => (source, render_path),
                    };
                    render_pages(&typst, &main_path, PREVIEW_PIXEL_PER_PT, PREVIEW_MAX_PAGES)
                })
                .await;
            this.update(cx, |this, cx| {
                let same_document = this.preview.path.as_ref() == Some(&path);
                match result {
                    Ok(rendered) => {
                        let pages = rendered
                            .pages
                            .into_iter()
                            .map(|page| {
                                let ratio = page.width as f32 / page.height.max(1) as f32;
                                (Arc::new(Image::from_bytes(ImageFormat::Png, page.png)), ratio)
                            })
                            .collect();
                        let old = std::mem::replace(&mut this.preview.pages, pages);
                        for (image, _) in old {
                            ImageSource::Image(image).remove_asset(cx);
                        }
                        this.preview.total_pages = rendered.total_pages;
                        this.preview.error = None;
                    }
                    Err(err) => {
                        // Letzten guten Stand stehen lassen, Fehler darüber anzeigen.
                        if !same_document {
                            for (image, _) in std::mem::take(&mut this.preview.pages) {
                                ImageSource::Image(image).remove_asset(cx);
                            }
                        }
                        this.preview.error = Some(err.to_string());
                    }
                }
                this.preview.path = Some(path);
                cx.notify();
            })
            .ok();
        });
    }

    // ---- Formatieren ----

    /// Wendet eine Formatier-/Einfügeaktion auf die aktive Markdown-/Typst-Datei an.
    fn format(&mut self, action: Format, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer() else {
            return;
        };
        let Some(syntax) = buffer.syntax else {
            window.push_notification(
                Notification::info("Formatierhilfen gibt es für Markdown- und Typst-Dateien."),
                cx,
            );
            return;
        };
        if !action.available(syntax) {
            window.push_notification(
                Notification::info(format!("„{}“ gibt es in {} nicht.", action.label(), syntax.name())),
                cx,
            );
            return;
        }
        let tabstops = buffer.editor.clone().update(cx, |state, cx| {
            let text = state.value().to_string();
            let edit = authoring::apply(action, syntax, &text, state.selected_range());
            for change in edit.changes.iter().rev() {
                state.set_selected_range(change.range.clone(), cx);
                state.replace(change.text.clone(), window, cx);
            }
            state.set_selected_range(edit.selection(), cx);
            state.focus(window, cx);
            Tabstops::new(edit.placeholders, state.value().to_string())
        });
        if let Some(buffer) = self.active_buffer_mut() {
            buffer.tabstops = tabstops;
        }
    }

    fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        let pane = &mut self.panes[self.active_pane];
        pane.buffers.get_mut(pane.active)
    }

    // ---- Platzhalter (Tab springt weiter) ----

    /// Führt die Platzhalter nach einer Eingabe nach; eine übernommene `/`-Einfügung aktiviert
    /// sie und markiert den ersten.
    fn track_placeholders(&mut self, editor: &Entity<EditorState>, cx: &mut Context<Self>) {
        let Some(buffer) = self.buffer_for_editor(editor) else {
            return;
        };
        let state = editor.read(cx);
        let cursor = state.cursor();
        if let Some(stops) = buffer.tabstops.as_mut() {
            if !stops.sync(&state.value(), cursor) {
                buffer.tabstops = None;
            }
            return;
        }
        let Some(placeholders) = buffer.assist.as_ref().and_then(|a| a.accepted_snippet(state.text(), cursor)) else {
            return;
        };
        let first = placeholders[0].clone();
        buffer.tabstops = Tabstops::new(placeholders, state.value().to_string());
        editor.update(cx, |state, cx| state.set_selected_range(first, cx));
    }

    /// Tab/Shift-Tab: nächsten/vorigen Platzhalter markieren. Ohne aktive Platzhalter (oder
    /// mit Cursor außerhalb der Vorlage) geht die Taste an den Editor (Einrücken).
    fn jump_placeholder(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer_mut() else {
            cx.propagate();
            return;
        };
        let editor = buffer.editor.clone();
        let state = editor.read(cx);
        let focused = state.focus_handle(cx).is_focused(window);
        let Some(stops) = buffer.tabstops.as_mut().filter(|s| focused && s.contains(state.cursor())) else {
            buffer.tabstops = None;
            cx.propagate();
            return;
        };
        let target = if forward { stops.next_range() } else { Some(stops.prev_range()) };
        let target = match target {
            Some(target) => target,
            None => {
                // Nach dem letzten Platzhalter: ans Zeilenende, Modus beenden.
                let end = stops.current().end;
                let text = state.value();
                let end = text[end..].find('\n').map_or(text.len(), |i| end + i);
                buffer.tabstops = None;
                end..end
            }
        };
        editor.update(cx, |state, cx| state.set_selected_range(target, cx));
    }

    fn next_placeholder(&mut self, _: &NextPlaceholder, window: &mut Window, cx: &mut Context<Self>) {
        self.jump_placeholder(true, window, cx);
    }

    fn prev_placeholder(&mut self, _: &PrevPlaceholder, window: &mut Window, cx: &mut Context<Self>) {
        self.jump_placeholder(false, window, cx);
    }

    /// Esc beendet den Platzhalter-Modus; der Editor bekommt die Taste trotzdem (z. B. um die
    /// Vorschlagsliste zu schließen).
    fn end_placeholders(&mut self, _: &EndPlaceholders, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(buffer) = self.active_buffer_mut() {
            buffer.tabstops = None;
        }
        cx.propagate();
    }

    // ---- Neues Dokument ----

    fn new_document(&mut self, _: &NewDocument, window: &mut Window, cx: &mut Context<Self>) {
        if self.root.is_none() {
            window.push_notification(Notification::info("Neue Dokumente landen im Workspace – bitte erst einen Ordner wählen."), cx);
            self.open_workspace(&OpenWorkspace, window, cx);
            return;
        }
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Titel des Dokuments"));
        let author = cx.new(|cx| InputState::new(window, cx).placeholder("Autor (optional)"));
        title.update(cx, |state, cx| state.focus(window, cx));
        self.new_doc = Some(NewDocForm {
            doc_type: DocType::Article,
            syntax: Syntax::Markdown,
            title,
            author,
        });
        cx.notify();
    }

    fn create_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(form), Some(root)) = (self.new_doc.as_ref(), self.root.clone()) else {
            return;
        };
        let info = DocInfo {
            title: form.title.read(cx).value().to_string(),
            author: form.author.read(cx).value().to_string(),
            date: today(),
            lang: "de".into(),
        };
        let extension = match form.syntax {
            Syntax::Markdown => "md",
            Syntax::Typst => "typ",
        };
        let stem = form.doc_type.file_stem(&info);
        let mut path = root.join(format!("{stem}.{extension}"));
        let mut n = 2;
        while path.exists() {
            path = root.join(format!("{stem}-{n}.{extension}"));
            n += 1;
        }
        let content = form.doc_type.render(form.syntax, &info);
        if let Err(err) = std::fs::write(&path, content) {
            window.push_notification(
                Notification::error(format!("Dokument konnte nicht angelegt werden: {err}")),
                cx,
            );
            return;
        }
        self.new_doc = None;
        self.refresh_tree(cx);
        self.open_file(path, window, cx);
    }

    // ---- Export ----

    fn export_typst(&mut self, _: &ExportTypst, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer() else {
            return;
        };
        if buffer.syntax != Some(Syntax::Markdown) {
            window.push_notification(
                Notification::info("Nach Typst umwandeln geht für Markdown-Dateien (.md, .mdx)."),
                cx,
            );
            return;
        }
        let source = buffer.editor.read(cx).value().to_string();
        let path = buffer.path.clone();
        if buffer.note_id.is_some() {
            // Notizen haben keinen Speicherort – Ziel erfragen.
            let name = format!("{}.typ", buffer.label());
            self.ask_export_path(name, move |this, target, window, cx| {
                this.write_typst_export(&source, &path, target, window, cx)
            }, window, cx);
            return;
        }
        let target = path.with_extension("typ");
        if target.exists() && !generated_by_devmark(&target) {
            window.push_notification(
                Notification::error(format!(
                    "„{}“ existiert bereits und wurde nicht von DevMark erzeugt – nicht überschrieben.",
                    file_name(&target)
                )),
                cx,
            );
            return;
        }
        self.write_typst_export(&source, &path, target, window, cx);
    }

    fn write_typst_export(&mut self, source: &str, path: &Path, target: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let mut options = Options::for_file(path);
        options.output_dir = target.parent().map(Path::to_path_buf);
        let conversion = markdown_to_typst(source, &options);
        if let Err(err) = std::fs::write(&target, &conversion.typst) {
            window.push_notification(
                Notification::error(format!("„{}“ konnte nicht geschrieben werden: {err}", file_name(&target))),
                cx,
            );
            return;
        }
        let message = if conversion.warnings.is_empty() {
            format!("Nach „{}“ umgewandelt.", file_name(&target))
        } else {
            format!(
                "Nach „{}“ umgewandelt. Hinweise:\n• {}",
                file_name(&target),
                conversion.warnings.iter().take(5).cloned().collect::<Vec<_>>().join("\n• ")
            )
        };
        window.push_notification(Notification::success(message), cx);
        self.refresh_tree(cx);
        self.reload_or_open(target, conversion.typst, window, cx);
    }

    /// Speichern-Dialog (Startordner: Workspace, sonst Dokumente), danach `then` mit dem gewählten Pfad.
    fn ask_export_path(
        &mut self,
        suggested_name: String,
        then: impl FnOnce(&mut Self, PathBuf, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = self
            .root
            .clone()
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Documents")))
            .unwrap_or_default();
        let name = suggested_name.replace(['/', '\\', ':'], "-");
        let receiver = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(target))) = receiver.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| then(this, target, window, cx)).ok();
        })
        .detach();
    }

    /// Öffnet `path`; ist die Datei schon offen, wird ihr Inhalt ersetzt.
    fn reload_or_open(&mut self, path: PathBuf, content: String, window: &mut Window, cx: &mut Context<Self>) {
        for buffer in self.panes.iter().flat_map(|pane| pane.buffers.iter()) {
            if buffer.path == path {
                buffer.editor.update(cx, |state, cx| state.set_value(content.clone(), window, cx));
            }
        }
        // Die Änderungsmeldung kommt verzögert; der Inhalt entspricht aber der Datei.
        cx.defer_in(window, {
            let path = path.clone();
            move |this, _window, cx| {
                for buffer in this.panes.iter_mut().flat_map(|pane| pane.buffers.iter_mut()) {
                    if buffer.path == path {
                        buffer.dirty = false;
                    }
                }
                cx.notify();
            }
        });
        self.open_file(path, window, cx);
    }

    fn export_pdf(&mut self, _: &ExportPdf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer() else {
            return;
        };
        let Some(syntax) = buffer.syntax else {
            window.push_notification(
                Notification::info("PDF-Export gibt es für Markdown- und Typst-Dateien."),
                cx,
            );
            return;
        };
        let source = buffer.editor.read(cx).value().to_string();
        let path = buffer.path.clone();
        if buffer.note_id.is_some() {
            let name = format!("{}.pdf", buffer.label());
            self.ask_export_path(name, move |this, target, window, cx| {
                this.start_pdf_export(source, syntax, path, target, window, cx)
            }, window, cx);
            return;
        }
        let pdf_path = path.with_extension("pdf");
        self.start_pdf_export(source, syntax, path, pdf_path, window, cx);
    }

    fn start_pdf_export(
        &mut self,
        source: String,
        syntax: Syntax,
        path: PathBuf,
        pdf_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(Notification::info(format!("„{}“ wird erstellt …", file_name(&pdf_path))), cx);

        let task = cx.background_spawn({
            let pdf_path = pdf_path.clone();
            async move {
                let (typst, main_path) = match syntax {
                    Syntax::Markdown => (
                        markdown_to_typst(&source, &Options::for_file(&path)).typst,
                        path.with_extension("typ"),
                    ),
                    Syntax::Typst => (source, path),
                };
                let compiled = compile_pdf(&typst, &main_path).map_err(|err| err.to_string())?;
                std::fs::write(&pdf_path, &compiled.pdf).map_err(|err| err.to_string())?;
                Ok::<usize, String>(compiled.pages)
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(pages) => {
                        window.push_notification(
                            Notification::success(format!(
                                "„{}“ erstellt ({pages} {}).",
                                file_name(&pdf_path),
                                if pages == 1 { "Seite" } else { "Seiten" }
                            )),
                            cx,
                        );
                        cx.open_with_system(&pdf_path);
                        this.refresh_tree(cx);
                    }
                    Err(err) => {
                        let first: Vec<&str> = err.lines().take(6).collect();
                        window.push_notification(
                            Notification::error(format!("PDF-Export fehlgeschlagen:\n{}", first.join("\n"))),
                            cx,
                        );
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- Darstellung ----

    /// Workspace-Kopf (Name + Wechsel) und Dateibaum; ohne Workspace eine Auswahl.
    fn render_files(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let recent = recent_workspaces();
        let Some(root) = self.root.clone() else {
            let mut body = v_flex()
                .p_3()
                .gap_3()
                .text_sm()
                .child(div().text_color(cx.theme().muted_foreground).child(
                    "Ein Workspace ist ein Hauptordner. Der Dateibaum zeigt nur ihn und seine Unterordner.",
                ))
                .child(
                    Button::new("open-workspace")
                        .outline()
                        .small()
                        .icon(AssetIcon::FolderOpen)
                        .label("Ordner öffnen …")
                        .on_click(cx.listener(|this, _, window, cx| this.open_workspace(&OpenWorkspace, window, cx))),
                );
            if !recent.is_empty() {
                body = body.child(
                    div().pt_2().text_xs().text_color(cx.theme().muted_foreground).child("Zuletzt geöffnet"),
                );
                for (ix, path) in recent.into_iter().enumerate() {
                    body = body.child(
                        ListItem::new(("recent", ix))
                            .rounded(cx.theme().radius)
                            .px_2()
                            .py_1()
                            .child(
                                v_flex()
                                    .child(file_name(&path))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(tilde(&path))),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| this.set_root(path.clone(), window, cx))),
                    );
                }
            }
            return body.into_any_element();
        };

        let workspace = cx.entity().downgrade();
        let header = h_flex().px_2().pt_2().child(
            Button::new("workspace-switch")
                .ghost()
                .small()
                .icon(AssetIcon::ChevronsUpDown)
                .label(file_name(&root))
                .tooltip(tilde(&root))
                .dropdown_menu(move |mut menu, _, _| {
                    for path in recent.iter().filter(|path| **path != root) {
                        let (workspace, path) = (workspace.clone(), path.clone());
                        menu = menu.item(PopupMenuItem::new(tilde(&path)).on_click(move |_, window, cx| {
                            workspace.update(cx, |this, cx| this.set_root(path.clone(), window, cx)).ok();
                        }));
                    }
                    menu.separator().menu("Ordner öffnen …", Box::new(OpenWorkspace))
                }),
        );
        v_flex()
            .size_full()
            .child(header)
            .child(div().flex_1().min_h_0().child(self.render_file_tree(cx)))
            .into_any_element()
    }

    fn render_file_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        tree(&self.tree_state, move |ix, entry, _selected, _window, cx| {
            view.update(cx, |_, cx| {
                let item = entry.item();
                let icon = if !entry.is_folder() {
                    IconName::File
                } else if entry.is_expanded() {
                    IconName::FolderOpen
                } else {
                    IconName::Folder
                };

                ListItem::new(ix)
                    .w_full()
                    .rounded(cx.theme().radius)
                    .py_0p5()
                    .px_2()
                    .pl(px(16.) * entry.depth() + px(8.))
                    .child(h_flex().gap_2().child(icon).child(item.label.clone()))
                    .on_click(cx.listener({
                        let item = item.clone();
                        move |this, _, window, cx| {
                            if !item.is_folder() && !item.id.ends_with(PLACEHOLDER_SUFFIX) {
                                this.open_file(PathBuf::from(item.id.as_str()), window, cx);
                            }
                        }
                    }))
            })
        })
        .text_sm()
        .p_1()
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().sidebar_foreground)
        .h_full()
    }

    /// Formatleiste über Markdown-/Typst-Dateien.
    fn render_toolbar(&self, syntax: Syntax, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = h_flex()
            .id("format-toolbar")
            .w_full()
            .flex_wrap()
            .gap_0p5()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border);
        let mut last_group = None;
        for (ix, (action, icon, ui_action)) in format_entries().into_iter().enumerate() {
            if !action.available(syntax) {
                continue;
            }
            if last_group.is_some_and(|group| group != action.group()) {
                bar = bar.child(div().w(px(1.)).h(px(16.)).mx_1().bg(cx.theme().border));
            }
            last_group = Some(action.group());
            let tooltip = format!("{} – {}", action.label(), action.description());
            bar = bar.child(
                Button::new(("format", ix))
                    .ghost()
                    .xsmall()
                    .icon(icon)
                    .tooltip_with_action(tooltip, ui_action.as_ref(), None)
                    .on_click(cx.listener(move |this, _, window, cx| this.format(action, window, cx))),
            );
        }
        bar = bar.child(div().flex_1());
        if syntax == Syntax::Markdown {
            bar = bar.child(
                Button::new("export-typst")
                    .ghost()
                    .xsmall()
                    .icon(AssetIcon::FileType)
                    .label("Typst")
                    .tooltip_with_action("Nach Typst umwandeln (.typ neben der Datei)", &ExportTypst, None)
                    .on_click(cx.listener(|this, _, window, cx| this.export_typst(&ExportTypst, window, cx))),
            );
        }
        if self.active_buffer().is_some_and(|b| b.note_id.is_some()) {
            let open = self.notes.as_ref().is_some_and(|n| n.history.is_some());
            bar = bar.child(
                Button::new("history")
                    .ghost()
                    .xsmall()
                    .icon(AssetIcon::RotateCcwClock)
                    .label("Verlauf")
                    .selected(open)
                    .tooltip_with_action("Frühere Stände dieser Notiz", &ShowHistory, None)
                    .on_click(cx.listener(|this, _, window, cx| this.show_history(&ShowHistory, window, cx))),
            );
        }
        bar.child(
            Button::new("preview")
                .ghost()
                .xsmall()
                .icon(AssetIcon::Eye)
                .label("Vorschau")
                .selected(self.show_preview)
                .tooltip_with_action("Druckvorschau neben dem Text ein-/ausblenden", &TogglePreview, None)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_preview(&TogglePreview, window, cx))),
        )
        .child(
            Button::new("export-pdf")
                .ghost()
                .xsmall()
                .icon(AssetIcon::Printer)
                .label("PDF")
                .tooltip_with_action("Als PDF exportieren und öffnen", &ExportPdf, None)
                .on_click(cx.listener(|this, _, window, cx| this.export_pdf(&ExportPdf, window, cx))),
        )
        .child(
            Button::new("help")
                .ghost()
                .xsmall()
                .icon(AssetIcon::CircleQuestionMark)
                .selected(self.show_help)
                .tooltip_with_action("Spickzettel ein-/ausblenden", &ToggleHelp, None)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_help(&ToggleHelp, window, cx))),
        )
    }

    /// Kopfzeile über dem Text: Tags der Notiz, Speicherstand, Wortzahl.
    fn render_doc_header(&self, buffer: &Buffer, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = match (buffer.dirty, buffer.note_id.is_some(), buffer.saved_at) {
            (true, true, _) => "Speichert …".to_string(),
            (true, false, _) => "Nicht gespeichert".to_string(),
            (false, _, Some(at)) => format!("Gespeichert {}", notes_ui::relative_time(at)),
            (false, _, None) => String::new(),
        };
        let words = match buffer.stats.words {
            1 => "1 Wort".to_string(),
            n => format!("{n} Wörter"),
        };
        let filter = self.notes.as_ref().and_then(|n| n.tag_filter.clone());
        let mut tags = h_flex().flex_1().min_w_0().flex_wrap().gap_1();
        for (ix, tag) in buffer.stats.tags.iter().enumerate() {
            let tag = tag.clone();
            tags = tags.child(
                Button::new(("doc-tag", ix))
                    .xsmall()
                    .ghost()
                    .selected(filter.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(&tag)))
                    .label(format!("#{tag}"))
                    .tooltip("Notizen mit diesem Tag zeigen")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sidebar = SidebarMode::Notes;
                        this.toggle_tag_filter(&tag, cx);
                    })),
            );
        }
        h_flex()
            .w_full()
            .gap_2()
            .pt_3()
            .pb_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(tags)
            .child(
                div()
                    .flex_none()
                    .child(if saved.is_empty() { words } else { format!("{saved} · {words}") }),
            )
    }

    /// Gliederung am rechten Rand: ein Strich je Überschrift, Klick springt hin.
    fn render_outline(&self, buffer: &Buffer, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let headings = &buffer.stats.headings;
        if headings.len() < 2 {
            return None;
        }
        let cursor_line = buffer.editor.read(cx).cursor_position().line;
        let current = headings.iter().rposition(|h| h.line <= cursor_line);
        let mut outline = v_flex()
            .id("outline")
            .flex_none()
            .w(px(32.))
            .h_full()
            .justify_center()
            .items_end()
            .pr_2()
            .overflow_y_scroll();
        for (ix, heading) in headings.iter().enumerate() {
            let editor = buffer.editor.clone();
            let line = heading.line;
            let text = SharedString::from(heading.text.clone());
            let width = match heading.level {
                1 => 16.,
                2 => 12.,
                _ => 8.,
            };
            let color = if current == Some(ix) { cx.theme().foreground } else { cx.theme().muted_foreground.opacity(0.5) };
            outline = outline.child(
                div()
                    .id(("outline-heading", ix))
                    .py(px(3.))
                    .cursor_pointer()
                    .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
                    .on_click(move |_, window, cx| {
                        editor.update(cx, |state, cx| {
                            state.set_cursor_position(lsp_types::Position::new(line, 0), window, cx)
                        });
                    })
                    .child(div().w(px(width)).h(px(2.)).rounded_full().bg(color)),
            );
        }
        Some(outline)
    }

    /// Spickzettel: alle Elemente mit Erklärung, Beispiel und „Einfügen“.
    fn render_help(&self, syntax: Syntax, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex()
            .id("help-panel")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .gap_3()
            .bg(cx.theme().sidebar)
            .text_sm()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().font_weight(FontWeight::BOLD).child(format!("Spickzettel {}", syntax.name())))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Tipp: „/“ am Zeilenanfang öffnet die Einfüge-Liste. Mauszeiger auf Syntax zeigt eine Erklärung."),
                    ),
            );
        for (ix, &action) in Format::ALL.iter().enumerate() {
            if !action.available(syntax) {
                continue;
            }
            list = list.child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(action.label()))
                            .child(
                                Button::new(("help-insert", ix))
                                    .xsmall()
                                    .outline()
                                    .label("Einfügen")
                                    .on_click(cx.listener(move |this, _, window, cx| this.format(action, window, cx))),
                            ),
                    )
                    .child(div().text_color(cx.theme().muted_foreground).child(action.description()))
                    .child(
                        div()
                            .p_2()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().background)
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_xs()
                            .whitespace_normal()
                            .child(action.example(syntax)),
                    ),
            );
        }
        if syntax == Syntax::Markdown {
            list = list.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Für den Druck: „PDF“ setzt das Dokument mit Typst. „Typst“ erzeugt eine .typ-Datei zum Weiterbearbeiten."),
            );
        }
        list
    }

    fn render_preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = v_flex()
            .id("preview-panel")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .bg(cx.theme().muted);
        if let Some(error) = &self.preview.error {
            let lines: Vec<&str> = error.lines().take(6).collect();
            column = column.child(
                div()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().danger)
                    .bg(cx.theme().background)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(lines.join("\n")),
            );
        }
        if self.preview.pages.is_empty() && self.preview.error.is_none() {
            column = column.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Vorschau wird erstellt …"),
            );
        }
        for (ix, (image, ratio)) in self.preview.pages.iter().enumerate() {
            column = column.child(
                div()
                    .id(("preview-page", ix))
                    .w_full()
                    .aspect_ratio(*ratio)
                    .flex_none()
                    .bg(gpui_kit::white())
                    .shadow_md()
                    .child(img(image.clone()).size_full()),
            );
        }
        if self.preview.total_pages > self.preview.pages.len() {
            column = column.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "Vorschau zeigt die ersten {} von {} Seiten – „PDF“ erzeugt das ganze Dokument.",
                        self.preview.pages.len(),
                        self.preview.total_pages
                    )),
            );
        }
        column
    }

    fn render_new_document(&self, form: &NewDocForm, cx: &mut Context<Self>) -> impl IntoElement {
        let mut types = v_flex().gap_1();
        for (ix, &doc_type) in DocType::ALL.iter().enumerate() {
            let selected = form.doc_type == doc_type;
            types = types.child(
                div()
                    .id(("doc-type", ix))
                    // Auswahlliste für Screenreader und Bedienungshilfen.
                    .role(Role::RadioButton)
                    .aria_label(doc_type.label())
                    .aria_description(doc_type.description())
                    .aria_toggled(if selected { Toggled::True } else { Toggled::False })
                    .aria_position_in_set(ix + 1)
                    .aria_size_of_set(DocType::ALL.len())
                    .rounded(cx.theme().radius)
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if selected { cx.theme().primary } else { cx.theme().transparent })
                    .bg(if selected { cx.theme().accent } else { cx.theme().transparent })
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(
                        v_flex()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(doc_type.label()))
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(doc_type.description())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = this.new_doc.as_mut() {
                            form.doc_type = doc_type;
                            cx.notify();
                        }
                    })),
            );
        }
        let syntax_button = |syntax: Syntax, hint: &'static str| {
            Button::new(syntax.name())
                .small()
                .label(format!("{} – {hint}", syntax.name()))
                .selected(form.syntax == syntax)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(form) = this.new_doc.as_mut() {
                        form.syntax = syntax;
                        cx.notify();
                    }
                }))
        };
        v_flex()
            .id("new-document")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_4()
            .max_w(px(640.))
            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Neues Dokument"))
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Dokumenttyp"))
                    .child(types),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Format"))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(syntax_button(Syntax::Markdown, "einfach, überall lesbar"))
                            .child(syntax_button(Syntax::Typst, "voller Satz für den Druck")),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Angaben"))
                    .child(Input::new(&form.title))
                    .child(Input::new(&form.author)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("create-document")
                            .primary()
                            .label("Anlegen")
                            .on_click(cx.listener(|this, _, window, cx| this.create_document(window, cx))),
                    )
                    .child(
                        Button::new("cancel-document")
                            .ghost()
                            .label("Abbrechen")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.new_doc = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "Die Datei wird in „{}“ angelegt.",
                        self.root.as_deref().map(Path::display).map(|d| d.to_string()).unwrap_or_default()
                    )),
            )
    }

    fn render_pane(&self, pane_ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = &self.panes[pane_ix];
        let tabs = pane.buffers.iter().map(|buffer| {
            let name = buffer.label();
            // Notizen speichern automatisch – kein Punkt für „ungespeichert“.
            let label = if buffer.dirty && buffer.note_id.is_none() { format!("● {name}") } else { name };
            Tab::new().label(label).when(self.is_outside(buffer), |tab| {
                tab.prefix(
                    Icon::new(AssetIcon::FileSymlink)
                        .xsmall()
                        .ml_2()
                        .text_color(cx.theme().muted_foreground),
                )
            })
        });

        let view = cx.entity();
        let tab_bar = TabBar::new(("tabs", pane_ix))
            .children(tabs)
            .selected_index(pane.active)
            .on_click(move |ix, _window, cx| {
                let ix = *ix;
                view.update(cx, |this, cx| {
                    this.active_pane = pane_ix;
                    this.panes[pane_ix].active = ix;
                    this.new_doc = None;
                    this.schedule_preview(Duration::ZERO, cx);
                    cx.notify();
                });
            });

        let new_doc_here = pane_ix == self.active_pane && self.new_doc.is_some();
        let body = match (self.new_doc.as_ref().filter(|_| new_doc_here), pane.buffers.get(pane.active)) {
            (Some(form), _) => self.render_new_document(form, cx).into_any_element(),
            (None, Some(buffer)) => {
                let editor = Editor::new(&buffer.editor).bordered(false).p_0().h_full();
                match buffer.syntax {
                    // Schreib-Layout: Proportionalschrift in einer zentrierten Textspalte.
                    Some(syntax) => v_flex()
                        .size_full()
                        .child(self.render_toolbar(syntax, cx))
                        .child(
                            h_flex()
                                .flex_1()
                                .min_h_0()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .h_full()
                                        .child(
                                            v_flex()
                                                .w_full()
                                                .max_w(px(WRITING_COLUMN))
                                                .mx_auto()
                                                .h_full()
                                                .px_6()
                                                .child(self.render_doc_header(buffer, cx))
                                                .child(
                                                    div().flex_1().min_h_0().child(
                                                        editor
                                                            .font_family(writing_font(cx))
                                                            .text_size(px(WRITING_FONT_SIZE)),
                                                    ),
                                                ),
                                        ),
                                )
                                .children(self.render_outline(buffer, cx)),
                        )
                        .into_any_element(),
                    None => editor
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(cx.theme().mono_font_size)
                        .into_any_element(),
                }
            }
            (None, None) => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(cx.theme().muted_foreground)
                .child(if self.root.is_some() { "Datei im Baum auswählen" } else { "Kein Workspace geöffnet" })
                .child(
                    Button::new(("new-doc-empty", pane_ix))
                        .outline()
                        .icon(AssetIcon::FilePlus)
                        .label("Neues Dokument …")
                        .on_click(cx.listener(|this, _, window, cx| this.new_document(&NewDocument, window, cx))),
                )
                .into_any_element(),
        };

        v_flex()
            .id(("pane", pane_ix))
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.active_pane != pane_ix {
                        this.active_pane = pane_ix;
                        cx.notify();
                    }
                }),
            )
            .child(tab_bar)
            .child(div().flex_1().min_h_0().child(body))
    }
}

/// Registriert die Formatier-Actions auf dem Workspace-Element.
macro_rules! format_actions {
    ($element:expr, $cx:expr, { $($action:ty => $format:expr),* $(,)? }) => {
        $element$(.on_action($cx.listener(|this, _: &$action, window, cx| this.format($format, window, cx))))*
    };
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editors = if self.split {
            h_resizable("panes")
                .child(resizable_panel().child(self.render_pane(0, cx)))
                .child(resizable_panel().child(self.render_pane(1, cx)))
                .into_any_element()
        } else {
            self.render_pane(0, cx).into_any_element()
        };

        let active_syntax = self.active_buffer().and_then(|b| b.syntax).filter(|_| self.new_doc.is_none());
        let history = self.render_history(cx);
        let main = match active_syntax {
            Some(syntax) if self.show_help || self.show_preview || history.is_some() => {
                let mut group = h_resizable("writing-split").child(resizable_panel().child(editors));
                if let Some(history) = history {
                    group = group.child(resizable_panel().size(px(300.)).child(history));
                }
                if self.show_preview {
                    group = group.child(resizable_panel().size(px(460.)).child(self.render_preview(cx)));
                }
                if self.show_help {
                    group = group.child(resizable_panel().size(px(300.)).child(self.render_help(syntax, cx)));
                }
                group.into_any_element()
            }
            _ => editors,
        };

        let status = self
            .active_buffer()
            .map(|b| {
                let path = match &b.note_id {
                    Some(_) => format!("Notiz – {}", b.label()),
                    None if self.is_outside(b) => format!("Außerhalb des Workspace – {}", tilde(&b.path)),
                    None => self.root.as_ref().and_then(|root| b.path.strip_prefix(root).ok()).unwrap_or(&b.path).display().to_string(),
                };
                match b.syntax {
                    Some(syntax) => format!("{path} · {}", syntax.name()),
                    None => path,
                }
            })
            .unwrap_or_default();

        let root = v_flex()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::open_workspace))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_split))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::toggle_preview))
            .on_action(cx.listener(Self::new_note))
            .on_action(cx.listener(Self::selection_to_note))
            .on_action(cx.listener(Self::export_notes))
            .on_action(cx.listener(Self::backup_now))
            .on_action(cx.listener(Self::show_files))
            .on_action(cx.listener(Self::show_notes))
            .on_action(cx.listener(Self::toggle_trash))
            .on_action(cx.listener(Self::show_history))
            .on_action(cx.listener(Self::sync_now))
            .on_action(cx.listener(Self::choose_sync_folder))
            .on_action(cx.listener(Self::disable_sync))
            .on_action(cx.listener(Self::new_document))
            .on_action(cx.listener(Self::export_typst))
            .on_action(cx.listener(Self::export_pdf))
            .on_action(cx.listener(Self::next_placeholder))
            .on_action(cx.listener(Self::prev_placeholder))
            .on_action(cx.listener(Self::end_placeholders));
        let root = format_actions!(root, cx, {
            FormatBold => Format::Bold,
            FormatItalic => Format::Italic,
            FormatStrike => Format::Strike,
            FormatCode => Format::Code,
            InsertLink => Format::Link,
            Heading1 => Format::Heading(1),
            Heading2 => Format::Heading(2),
            Heading3 => Format::Heading(3),
            BulletList => Format::BulletList,
            NumberedList => Format::NumberedList,
            TaskList => Format::TaskList,
            QuoteBlock => Format::Quote,
            InsertImage => Format::Image,
            InsertTable => Format::Table,
            InsertCodeBlock => Format::CodeBlock,
            InsertFootnote => Format::Footnote,
            InsertCallout => Format::Callout,
            InsertMath => Format::MathInline,
            InsertMathBlock => Format::MathBlock,
            InsertRule => Format::Rule,
            InsertPageBreak => Format::PageBreak,
            InsertOutline => Format::Outline,
        });

        root.child(
            div().flex_1().min_h_0().child(
                h_resizable("workspace-split")
                    .child(
                        resizable_panel()
                            .size(px(260.))
                            .child(self.render_sidebar(cx)),
                    )
                    .child(resizable_panel().child(main)),
            ),
        )
        .child(
            StatusBar::new().left(status).right(
                h_flex()
                    .gap_1()
                    .when(self.sync.folder.is_some(), |this| {
                        let label = if self.sync.running {
                            "Sync …".to_string()
                        } else if self.sync.last_error.is_some() {
                            "Sync ⚠".to_string()
                        } else {
                            "Sync".to_string()
                        };
                        let tooltip = match (&self.sync.last_error, self.sync.last_sync) {
                            (Some(err), _) => format!("Letzter Sync fehlgeschlagen: {err}"),
                            (None, Some(at)) => format!("Zuletzt synchronisiert {}", date_time_from_ms(at)),
                            (None, None) => "Noch nicht synchronisiert".to_string(),
                        };
                        this.child(
                            Button::new("sync-status")
                                .ghost()
                                .xsmall()
                                .label(label)
                                .tooltip(tooltip)
                                .on_click(cx.listener(|this, _, window, cx| this.sync_now(&SyncNow, window, cx))),
                        )
                    })
                    .child(
                        Button::new("new-doc")
                            .ghost()
                            .xsmall()
                            .label("Neu …")
                            .tooltip_with_action("Neues Dokument aus Vorlage", &NewDocument, None)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_document(&NewDocument, window, cx)
                            })),
                    )
                    .child(
                        Button::new("split")
                            .ghost()
                            .xsmall()
                            .label(if self.split { "Split aus" } else { "Split" })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_split(&ToggleSplit, window, cx)
                            })),
                    ),
            ),
        )
    }
}

fn language_for(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "rs" => "rust",
        "typ" => "typst",
        "md" | "markdown" | "mdx" => "markdown",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" => "python",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "" => "text",
        ext => ext,
    }
    .to_string()
}

/// Typst-Grammatik registrieren (gpui-kit bringt keine mit).
fn register_typst_language() {
    use gpui_kit::component::highlighter::{LanguageConfig, LanguageRegistry};
    let config = LanguageConfig::new(
        "typst",
        codebook_tree_sitter_typst::LANGUAGE.into(),
        vec![],
        TYPST_HIGHLIGHTS,
        "",
        "",
    );
    LanguageRegistry::singleton().register("typst", &config);
}

const TYPST_HIGHLIGHTS: &str = include_str!("typst-highlights.scm");

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Von DevMark erzeugte .typ-Dateien dürfen beim erneuten Umwandeln überschrieben werden.
fn generated_by_devmark(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.lines().next().is_some_and(|line| line.contains("mit DevMark erzeugt")))
        .unwrap_or(false)
}

fn today() -> String {
    iso_date(now_ms())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn file_modified_ms(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// Zeitpunkt (UTC) als `14.10.2026, 20:15 UTC`.
fn date_time_from_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000).rem_euclid(86_400);
    format!("{}, {:02}:{:02} UTC", date_from_ms(ms), secs / 3600, secs % 3600 / 60)
}

/// Datum (UTC) als `14.10.2026`.
fn date_from_ms(ms: i64) -> String {
    let iso = iso_date(ms);
    let mut parts = iso.split('-');
    let (year, month, day) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    format!("{day}.{month}.{year}")
}

fn iso_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // Tage seit 1970 → Datum (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn window_title(root: Option<&Path>) -> String {
    match root {
        Some(root) => format!("DevMark – {}", root.file_name().unwrap_or_default().to_string_lossy()),
        None => "DevMark".into(),
    }
}

/// `/Users/x/a.md` → `~/a.md`.
fn tilde(path: &Path) -> String {
    std::env::var_os("HOME")
        .and_then(|home| path.strip_prefix(home).ok())
        .map(|rest| format!("~/{}", rest.display()))
        .unwrap_or_else(|| path.display().to_string())
}

/// Einträge eines Ordners: Ordner zuerst, alphabetisch; Build-/VCS-Ordner ausgeblendet.
fn list_dir(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items: Vec<(PathBuf, String, bool)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let is_dir = path.is_dir();
            // Versteckte Ordner (Home voller Tool-Ordner) aus, nützliche Projektordner an.
            let hidden = if is_dir {
                IGNORED_DIRS.contains(&name.as_str())
                    || (name.starts_with('.') && !matches!(name.as_str(), ".github" | ".vscode"))
            } else {
                name == ".DS_Store"
            };
            (!hidden).then_some((path, name, is_dir))
        })
        .collect();
    items.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    items
}

const MAX_RECENT_WORKSPACES: usize = 8;

/// Zuletzt geöffnete Workspaces, neuester zuerst (nur noch vorhandene Ordner).
fn recent_workspaces() -> Vec<PathBuf> {
    let dir = notes_ui::data_dir();
    // `last-root` stammt aus der Zeit vor der Liste.
    std::fs::read_to_string(dir.join("workspaces"))
        .or_else(|_| std::fs::read_to_string(dir.join("last-root")))
        .unwrap_or_default()
        .lines()
        .map(|line| PathBuf::from(line.trim()))
        .filter(|path| path.is_dir())
        .collect()
}

fn remember_root(root: &Path) {
    let mut recent = recent_workspaces();
    recent.retain(|path| path != root);
    recent.insert(0, root.to_path_buf());
    recent.truncate(MAX_RECENT_WORKSPACES);
    let file = notes_ui::data_dir().join("workspaces");
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let lines: Vec<_> = recent.iter().map(|path| path.to_string_lossy()).collect();
    std::fs::write(file, lines.join("\n")).ok();
}

/// Standard-Theme (Schwarz-Weiß) mit warmen statt kühlen Grautönen (Tailwind „stone“).
fn warm_theme(cx: &mut App) {
    Theme::update(cx, |theme| {
        if theme.is_dark() {
            return;
        }
        theme.sidebar = rgb(0xfafaf9).into();
        theme.tab_bar = rgb(0xfafaf9).into();
        theme.accent = rgb(0xf0efed).into();
        theme.sidebar_accent = rgb(0xf0efed).into();
        theme.border = rgb(0xe7e5e4).into();
        theme.sidebar_border = rgb(0xe7e5e4).into();
        theme.muted_foreground = rgb(0x78716c).into();
    });
}

fn main() {
    // Argument: Ordner (wird Workspace) oder Datei (wird geöffnet, Workspace = zuletzt genutzter).
    // Ohne Argument: zuletzt genutzter Workspace, sonst keiner – nie stillschweigend Home oder „/“.
    let arg = std::env::args().nth(1).map(PathBuf::from).map(|arg| arg.canonicalize().unwrap_or(arg));
    let last = || recent_workspaces().into_iter().next();
    let (root, file) = match arg {
        Some(arg) if arg.is_dir() => (Some(arg), None),
        Some(arg) => (last(), Some(arg)),
        None => (last(), None),
    };

    // Finder „Öffnen mit …“ / Doppelklick: Pfade kommen als file://-URLs, auch beim Start.
    let (open_tx, mut open_rx) = futures::channel::mpsc::unbounded::<Vec<String>>();
    let app = gpui_kit::application().with_assets(AppAssets);
    app.on_open_urls(move |urls| {
        open_tx.unbounded_send(urls).ok();
    });
    app.run(move |cx| {
        gpui_kit::init(cx);
        warm_theme(cx);
        register_typst_language();
        cx.bind_keys([
            KeyBinding::new("secondary-o", Open, None),
            KeyBinding::new("secondary-n", NewDocument, None),
            KeyBinding::new("secondary-s", Save, None),
            KeyBinding::new("secondary-w", CloseTab, None),
            KeyBinding::new("secondary-\\", ToggleSplit, None),
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-shift-/", ToggleHelp, None),
            KeyBinding::new("secondary-shift-v", TogglePreview, None),
            KeyBinding::new("secondary-shift-n", NewNote, None),
            KeyBinding::new("secondary-shift-f", ShowNotes, None),
            KeyBinding::new("secondary-shift-e", ExportPdf, None),
            KeyBinding::new("secondary-shift-t", ExportTypst, None),
            KeyBinding::new("secondary-b", FormatBold, None),
            KeyBinding::new("secondary-i", FormatItalic, None),
            KeyBinding::new("secondary-shift-x", FormatStrike, None),
            KeyBinding::new("secondary-e", FormatCode, None),
            KeyBinding::new("secondary-k", InsertLink, None),
            KeyBinding::new("secondary-alt-1", Heading1, None),
            KeyBinding::new("secondary-alt-2", Heading2, None),
            KeyBinding::new("secondary-alt-3", Heading3, None),
            KeyBinding::new("secondary-shift-8", BulletList, None),
            KeyBinding::new("secondary-shift-7", NumberedList, None),
            KeyBinding::new("secondary-shift-9", TaskList, None),
            KeyBinding::new("secondary-shift-.", QuoteBlock, None),
            // Im Editor-Kontext „Input“ von gpui-kit; später gebunden = Vorrang vor dessen
            // Einrücken/Escape. Ohne aktive Platzhalter reichen die Handler die Taste weiter.
            KeyBinding::new("tab", NextPlaceholder, Some("Input")),
            KeyBinding::new("shift-tab", PrevPlaceholder, Some("Input")),
            KeyBinding::new("escape", EndPlaceholders, Some("Input")),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus([
            Menu::new("DevMark").items([MenuItem::action("DevMark beenden", Quit)]),
            Menu::new("Datei").items([
                MenuItem::action("Neues Dokument …", NewDocument),
                MenuItem::action("Öffnen …", Open),
                MenuItem::action("Workspace öffnen …", OpenWorkspace),
                MenuItem::separator(),
                MenuItem::action("Speichern", Save),
                MenuItem::action("Tab schließen", CloseTab),
                MenuItem::separator(),
                MenuItem::action("Nach Typst umwandeln", ExportTypst),
                MenuItem::action("Als PDF exportieren", ExportPdf),
            ]),
            Menu::new("Format").items([
                MenuItem::action("Fett", FormatBold),
                MenuItem::action("Kursiv", FormatItalic),
                MenuItem::action("Durchgestrichen", FormatStrike),
                MenuItem::action("Code im Text", FormatCode),
                MenuItem::action("Link", InsertLink),
                MenuItem::separator(),
                MenuItem::action("Überschrift 1", Heading1),
                MenuItem::action("Überschrift 2", Heading2),
                MenuItem::action("Überschrift 3", Heading3),
                MenuItem::separator(),
                MenuItem::action("Aufzählung", BulletList),
                MenuItem::action("Nummerierte Liste", NumberedList),
                MenuItem::action("Aufgabenliste", TaskList),
                MenuItem::action("Zitat", QuoteBlock),
            ]),
            Menu::new("Einfügen").items([
                MenuItem::action("Bild", InsertImage),
                MenuItem::action("Tabelle", InsertTable),
                MenuItem::action("Codeblock", InsertCodeBlock),
                MenuItem::action("Fußnote", InsertFootnote),
                MenuItem::action("Hinweisbox", InsertCallout),
                MenuItem::separator(),
                MenuItem::action("Formel im Text", InsertMath),
                MenuItem::action("Formel (abgesetzt)", InsertMathBlock),
                MenuItem::separator(),
                MenuItem::action("Trennlinie", InsertRule),
                MenuItem::action("Seitenumbruch (Typst)", InsertPageBreak),
                MenuItem::action("Inhaltsverzeichnis (Typst)", InsertOutline),
            ]),
            Menu::new("Notizen").items([
                MenuItem::action("Neue Notiz", NewNote),
                MenuItem::action("Auswahl als Notiz", SelectionToNote),
                MenuItem::action("Notizen durchsuchen", ShowNotes),
                MenuItem::action("Papierkorb", ToggleTrash),
                MenuItem::action("Versionsverlauf", ShowHistory),
                MenuItem::separator(),
                MenuItem::action("Alle Notizen exportieren …", ExportNotes),
                MenuItem::action("Backup jetzt erstellen", BackupNow),
                MenuItem::separator(),
                MenuItem::action("Jetzt synchronisieren", SyncNow),
                MenuItem::action("Sync-Ordner wählen …", ChooseSyncFolder),
                MenuItem::action("Synchronisierung ausschalten", DisableSync),
            ]),
            Menu::new("Ansicht").items([
                MenuItem::action("Dateien", ShowFiles),
                MenuItem::action("Notizen", ShowNotes),
                MenuItem::separator(),
                MenuItem::action("Split umschalten", ToggleSplit),
                MenuItem::action("Vorschau", TogglePreview),
                MenuItem::action("Spickzettel", ToggleHelp),
            ]),
        ]);
        cx.activate(true);

        let title = window_title(root.as_deref());
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        let (window, workspace) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::new(root, file, window, cx)),
        )
        .expect("open window");

        cx.spawn(async move |cx| {
            use futures::StreamExt as _;
            while let Some(urls) = open_rx.next().await {
                for path in urls.iter().filter_map(|url| path_from_file_url(url)) {
                    window
                        .update(cx, |_, window, cx| {
                            window.activate_window();
                            workspace.update(cx, |workspace, cx| workspace.open_path(path, window, cx));
                        })
                        .ok();
                }
            }
        })
        .detach();
    });
}

/// `file:///Users/x/Notiz%20A.md` → Pfad.
fn path_from_file_url(url: &str) -> Option<PathBuf> {
    let encoded = url.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_urls() {
        assert_eq!(
            super::path_from_file_url("file:///Users/a/Notiz%20%C3%A4.md"),
            Some(std::path::PathBuf::from("/Users/a/Notiz ä.md"))
        );
        assert_eq!(super::path_from_file_url("https://x"), None);
    }

    #[test]
    fn doc_stats_outline_and_words() {
        use devmark_writing::authoring::Syntax;
        let markdown = "# Titel\n\nZwei Wörter - hier.\n\n```\n# kein Titel\n```\n## Unter #\n#tag ohne Leerzeichen";
        let stats = super::DocStats::of(Syntax::Markdown, markdown, true);
        let outline: Vec<_> = stats.headings.iter().map(|h| (h.line, h.level, h.text.as_str())).collect();
        assert_eq!(outline, [(0, 1, "Titel"), (7, 2, "Unter")]);
        assert_eq!(stats.words, 10);
        assert_eq!(stats.tags, ["tag"]);
        let typst = super::DocStats::of(Syntax::Typst, "= Kapitel\n== Abschnitt\n#set page()", false);
        assert_eq!(typst.headings.len(), 2);
        assert!(typst.tags.is_empty());
    }

    #[test]
    fn typst_highlight_query_matches_grammar() {
        let language: tree_sitter::Language = codebook_tree_sitter_typst::LANGUAGE.into();
        if let Err(err) = tree_sitter::Query::new(&language, super::TYPST_HIGHLIGHTS) {
            panic!("{err}");
        }
    }
}
