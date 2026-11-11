//! Schnellöffner (Cmd/Ctrl-P): Notizen und Dateien des Workspace in einer Liste, getippt wird
//! gefiltert; Enter öffnet, bei Eingaben ohne Treffer legt „Neu: …“ eine Notiz an.

use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme, WindowExt as _, h_flex,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::{IGNORED_DIRS, OpenQuick, Workspace};

/// So viele Treffer sieht man auf einmal.
const VISIBLE: usize = 9;
/// Obergrenze für gesammelte Dateien, damit ein riesiger Workspace das Öffnen nicht ausbremst.
const MAX_FILES: usize = 20_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Note(String),
    File(PathBuf),
    /// Neue Notiz mit diesem Titel.
    NewNote(String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    /// Ordner bzw. Pfad, in Grau hinter dem Namen.
    pub detail: String,
    pub target: Target,
}

pub struct QuickOpen {
    input: Entity<InputState>,
    all: Vec<Item>,
    shown: Vec<Item>,
    selected: usize,
    can_create: bool,
    _subscription: Subscription,
}

/// Treffer für `query`: jedes Wort muss im Namen oder Pfad vorkommen; Namen, die damit beginnen
/// oder die ganz passen, stehen vorn. Ohne Eingabe: die ersten Einträge in Ausgangsreihenfolge.
pub fn rank(query: &str, items: &[Item], limit: usize) -> Vec<Item> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return items.iter().take(limit).cloned().collect();
    }
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (ix, item) in items.iter().enumerate() {
        let (label, detail) = (item.label.to_lowercase(), item.detail.to_lowercase());
        let mut score = 0;
        let mut all = true;
        for word in &words {
            if label == *word {
                score += 6;
            } else if label.starts_with(word.as_str()) {
                score += 4;
            } else if label.contains(word.as_str()) {
                score += 2;
            } else if detail.contains(word.as_str()) {
                score += 1;
            } else {
                all = false;
                break;
            }
        }
        if all {
            scored.push((score, ix));
        }
    }
    // Stabil: gleiche Punkte behalten die Ausgangsreihenfolge (Notizen vor Dateien).
    scored.sort_by_key(|&(score, ix)| (std::cmp::Reverse(score), ix));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, ix)| items[ix].clone())
        .collect()
}

/// Dateien unter `root` (relativ, ohne versteckte und ignorierte Ordner).
pub fn collect_files(root: &Path) -> Vec<Item> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !name.starts_with('.') && !IGNORED_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if kind.is_file() && !name.starts_with('.') {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let detail = relative
                    .parent()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                files.push(Item {
                    label: name,
                    detail,
                    target: Target::File(path),
                });
                if files.len() >= MAX_FILES {
                    return files;
                }
            }
        }
    }
    files.sort_by(|a, b| (&a.detail, &a.label).cmp(&(&b.detail, &b.label)));
    files
}

impl Workspace {
    pub(crate) fn open_quick(
        &mut self,
        _: &OpenQuick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.quick_open.is_some() {
            self.close_quick(window, cx);
            return;
        }
        let mut all: Vec<Item> = Vec::new();
        if let Some(state) = &self.notes {
            let mut notes: Vec<_> = state.index.borrow().clone();
            notes.sort_by_key(|(_, title)| title.to_lowercase());
            all.extend(notes.into_iter().map(|(id, title)| {
                let detail = devmark_notes::parent_of(&id).unwrap_or_default();
                Item {
                    label: title,
                    detail: if detail.is_empty() {
                        "Notiz".into()
                    } else {
                        format!("Notiz · {}", detail.replace('/', " / "))
                    },
                    target: Target::Note(id),
                }
            }));
        }
        if let Some(root) = &self.root {
            all.extend(collect_files(root));
        }
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Notiz oder Datei öffnen …"));
        let subscription =
            cx.subscribe(&input, |this: &mut Workspace, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh_quick(cx);
                }
            });
        input.update(cx, |input, cx| input.focus(window, cx));
        let shown = rank("", &all, VISIBLE * 3);
        self.quick_open = Some(QuickOpen {
            input,
            all,
            shown,
            selected: 0,
            can_create: self.notes.is_some(),
            _subscription: subscription,
        });
        cx.notify();
    }

    fn refresh_quick(&mut self, cx: &mut Context<Self>) {
        let Some(quick) = self.quick_open.as_mut() else {
            return;
        };
        let query = quick.input.read(cx).value().trim().to_string();
        let mut shown = rank(&query, &quick.all, 60);
        let exact = shown
            .iter()
            .any(|i| i.label.eq_ignore_ascii_case(&query) && matches!(i.target, Target::Note(_)));
        if quick.can_create && !query.is_empty() && !exact {
            shown.push(Item {
                label: format!("Neu: {query}"),
                detail: "Notiz anlegen".into(),
                target: Target::NewNote(query),
            });
        }
        quick.shown = shown;
        quick.selected = 0;
        cx.notify();
    }

    fn close_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_open = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn move_quick(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(quick) = self.quick_open.as_mut()
            && !quick.shown.is_empty()
        {
            let last = quick.shown.len() as isize - 1;
            quick.selected = (quick.selected as isize + delta).clamp(0, last) as usize;
            cx.notify();
        }
    }

    fn confirm_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self
            .quick_open
            .as_ref()
            .and_then(|q| q.shown.get(q.selected).cloned())
        else {
            return;
        };
        self.close_quick(window, cx);
        match item.target {
            Target::Note(id) => self.open_note(id, window, cx),
            Target::File(path) => self.open_file(path, window, cx),
            Target::NewNote(title) => {
                let Some(state) = self.notes.as_mut() else {
                    return;
                };
                match state.notes.create(&title, &format!("# {title}\n\n"), None) {
                    Ok(document) => {
                        state.show_trash = false;
                        self.refresh_notes(cx);
                        self.open_note(document.id, window, cx);
                    }
                    Err(err) => window.push_notification(
                        Notification::error(format!("Notiz konnte nicht angelegt werden: {err}")),
                        cx,
                    ),
                }
            }
        }
    }

    /// Überlagerung mit Eingabe und Trefferliste; `None`, wenn der Schnellöffner zu ist.
    pub(crate) fn render_quick_open(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let quick = self.quick_open.as_ref()?;
        let start = quick.selected.saturating_sub(VISIBLE - 1);
        let mut list = v_flex().p_1().gap_0p5();
        if quick.shown.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Nichts gefunden."),
            );
        }
        for (ix, item) in quick.shown.iter().enumerate().skip(start).take(VISIBLE) {
            let selected = ix == quick.selected;
            list = list.child(
                h_flex()
                    .id(("quick-item", ix))
                    .role(Role::ListItem)
                    .aria_label(item.label.clone())
                    .aria_selected(selected)
                    .w_full()
                    .gap_3()
                    .px_3()
                    .py_1p5()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .when(selected, |this| this.bg(cx.theme().accent))
                    .hover(|this| this.bg(cx.theme().accent))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(300.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(item.label.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(item.detail.clone()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(quick) = this.quick_open.as_mut() {
                            quick.selected = ix;
                        }
                        this.confirm_quick(window, cx);
                    })),
            );
        }
        Some(
            div()
                .id("quick-open")
                .absolute()
                .inset_0()
                // Pfeile, Enter und Escape fangen wir vor der Eingabe ab: sie würde Pfeil
                // und Enter selbst verbrauchen.
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    let handled = match event.keystroke.key.as_str() {
                        "down" => {
                            this.move_quick(1, cx);
                            true
                        }
                        "up" => {
                            this.move_quick(-1, cx);
                            true
                        }
                        "enter" => {
                            this.confirm_quick(window, cx);
                            true
                        }
                        "escape" => {
                            this.close_quick(window, cx);
                            true
                        }
                        _ => false,
                    };
                    if handled {
                        cx.stop_propagation();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_quick(window, cx)),
                )
                .child(
                    v_flex().w_full().items_center().pt(px(90.)).child(
                        v_flex()
                            .w(px(620.))
                            .max_w_full()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius_lg)
                            .shadow_lg()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .p_2()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(Input::new(&quick.input)),
                            )
                            .child(list),
                    ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, Target, collect_files, rank};

    fn item(label: &str, detail: &str) -> Item {
        Item {
            label: label.into(),
            detail: detail.into(),
            target: Target::Note(label.into()),
        }
    }

    #[test]
    fn ranks_names_before_paths_and_needs_every_word() {
        let items = [
            item("Wochenplan", "Notiz"),
            item("Plan B", "Notiz · Projekte"),
            item("Einkauf", "Notiz · Plan"),
            item("plan", "Notiz"),
        ];
        let labels =
            |q: &str| -> Vec<String> { rank(q, &items, 10).into_iter().map(|i| i.label).collect() };
        // Ganzer Name, Anfang, Teil des Namens, nur im Pfad.
        assert_eq!(labels("plan"), ["plan", "Plan B", "Wochenplan", "Einkauf"]);
        assert_eq!(labels("plan proj"), ["Plan B"]);
        assert!(labels("xyz").is_empty());
        assert_eq!(labels("").len(), 4);
        assert_eq!(rank("", &items, 2).len(), 2);
    }

    #[test]
    fn collects_files_without_hidden_and_ignored_folders() {
        let root = std::env::temp_dir().join(format!("devmark-quick-{}", std::process::id()));
        for dir in ["docs", "target", ".git", "src/deep"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            "README.md",
            ".hidden",
            "docs/guide.md",
            "target/out.bin",
            ".git/HEAD",
            "src/deep/main.rs",
        ] {
            std::fs::write(root.join(file), "x").unwrap();
        }
        let found: Vec<_> = collect_files(&root)
            .into_iter()
            .map(|i| (i.detail, i.label))
            .collect();
        assert_eq!(
            found,
            [
                ("".to_string(), "README.md".to_string()),
                ("docs".to_string(), "guide.md".to_string()),
                ("src/deep".to_string(), "main.rs".to_string()),
            ]
        );
        std::fs::remove_dir_all(root).ok();
    }
}
