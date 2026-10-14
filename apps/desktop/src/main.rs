//! Phase-1-Spike: Dateibaum, Editor mit Tree-sitter-Highlighting, Tabs, Split, Speichern.

use std::path::{Path, PathBuf};

use gpui_kit::assets::Assets;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, InputEvent, TabSize},
    list::ListItem,
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    tree::{TreeItem, TreeState, tree},
    v_flex,
};
use gpui_kit::*;

actions!(devmark, [Save, ToggleSplit, CloseTab, Quit]);

const IGNORED_DIRS: &[&str] = &[".git", "target", "node_modules", "build", ".dart_tool"];

struct Buffer {
    path: PathBuf,
    editor: Entity<EditorState>,
    dirty: bool,
}

#[derive(Default)]
struct Pane {
    buffers: Vec<Buffer>,
    active: usize,
}

struct Workspace {
    root: PathBuf,
    tree_state: Entity<TreeState>,
    panes: Vec<Pane>,
    active_pane: usize,
    split: bool,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    fn new(root: PathBuf, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));
        let items = build_file_items(&root);
        tree_state.update(cx, |state, cx| state.set_items(items, cx));

        Self {
            root,
            tree_state,
            panes: vec![Pane::default(), Pane::default()],
            active_pane: 0,
            split: false,
            _subscriptions: Vec::new(),
        }
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let pane_ix = self.active_pane;
        if let Some(ix) = self.panes[pane_ix].buffers.iter().position(|b| b.path == path) {
            self.panes[pane_ix].active = ix;
            cx.notify();
            return;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                return;
            }
        };
        let language = language_for(&path);

        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language(language)
                .line_number(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .soft_wrap(false);
            state.set_value(content, window, cx);
            state
        });

        let subscription = cx.subscribe(&editor, |this, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(buffer) = this.buffer_for_editor(&editor) {
                    buffer.dirty = true;
                }
                cx.notify();
            }
        });
        self._subscriptions.push(subscription);

        editor.update(cx, |state, cx| state.focus(window, cx));

        let pane = &mut self.panes[pane_ix];
        pane.buffers.push(Buffer {
            path,
            editor,
            dirty: false,
        });
        pane.active = pane.buffers.len() - 1;
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

    fn save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
        let pane = &mut self.panes[self.active_pane];
        let Some(buffer) = pane.buffers.get_mut(pane.active) else {
            return;
        };
        let content = buffer.editor.read(cx).value();
        match std::fs::write(&buffer.path, content.as_bytes()) {
            Ok(()) => buffer.dirty = false,
            Err(err) => eprintln!("{}: {err}", buffer.path.display()),
        }
        cx.notify();
    }

    fn close_tab(&mut self, _: &CloseTab, _window: &mut Window, cx: &mut Context<Self>) {
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
                            if !item.is_folder() {
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

    fn render_pane(&self, pane_ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = &self.panes[pane_ix];
        let tabs = pane.buffers.iter().map(|buffer| {
            let name = buffer
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let label = if buffer.dirty { format!("● {name}") } else { name };
            Tab::new().label(label)
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
                    cx.notify();
                });
            });

        let body = match pane.buffers.get(pane.active) {
            Some(buffer) => Editor::new(&buffer.editor)
                .bordered(false)
                .p_0()
                .h_full()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(cx.theme().mono_font_size)
                .into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Datei im Baum auswählen")
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

        let status = self
            .active_buffer()
            .map(|b| {
                b.path
                    .strip_prefix(&self.root)
                    .unwrap_or(&b.path)
                    .display()
                    .to_string()
            })
            .unwrap_or_default();

        v_flex()
            .id("workspace")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::toggle_split))
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("workspace-split")
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .child(self.render_file_tree(cx)),
                        )
                        .child(resizable_panel().child(editors)),
                ),
            )
            .child(
                StatusBar::new().left(status).right(
                    Button::new("split")
                        .ghost()
                        .xsmall()
                        .label(if self.split { "Split aus" } else { "Split" })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_split(&ToggleSplit, window, cx)
                        })),
                ),
            )
    }
}

fn language_for(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "rs" => "rust",
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

fn build_file_items(dir: &Path) -> Vec<TreeItem> {
    let mut items = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return items;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let id = path.to_string_lossy().to_string();
        if path.is_dir() {
            if IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            items.push(TreeItem::new(id, name).children(build_file_items(&path)));
        } else if name != ".DS_Store" {
            items.push(TreeItem::new(id, name));
        }
    }
    items.sort_by(|a, b| {
        b.is_folder()
            .cmp(&a.is_folder())
            .then(a.label.cmp(&b.label))
    });
    items
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("current dir"));
    let root = root.canonicalize().unwrap_or(root);

    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.bind_keys([
            KeyBinding::new("secondary-s", Save, None),
            KeyBinding::new("secondary-w", CloseTab, None),
            KeyBinding::new("secondary-\\", ToggleSplit, None),
            KeyBinding::new("secondary-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.activate(true);

        let title = format!(
            "DevMark – {}",
            root.file_name().unwrap_or_default().to_string_lossy()
        );
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Workspace::new(root, window, cx)),
        )
        .expect("open window");
    });
}
