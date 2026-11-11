//! Blockansicht für Markdown-Notizen: der Text als Folge von Blöcken (Absatz, Überschrift,
//! Liste, Code …). Inaktive Blöcke erscheinen formatiert, der angeklickte ist ein Eingabefeld
//! mit dem Markdown. Die Datei ändert sich nur dort, wo man schreibt – ohne Änderung bleibt sie
//! Byte für Byte gleich (`devmark_notes::blocks`).

use devmark_notes::blocks::{self, Block, Kind};
use gpui_kit::component::{
    ActiveTheme, h_flex,
    input::{InputEvent, Position, Textarea, TextareaState},
    text::TextView,
    v_flex,
};
use gpui_kit::*;

use crate::{ToggleBlocks, Workspace};
use devmark_writing::authoring::Syntax;

/// Der Block, der gerade bearbeitet wird.
pub struct Active {
    ix: usize,
    input: Entity<TextareaState>,
    _subscription: Subscription,
}

pub struct BlockView {
    blocks: Vec<Block>,
    active: Option<Active>,
}

fn start_of(blocks: &[Block], ix: usize) -> usize {
    blocks.iter().take(ix).map(|b| b.text.len()).sum()
}

/// Block, der das Byte `offset` enthält (der letzte, falls dahinter).
fn block_at(blocks: &[Block], offset: usize) -> usize {
    let mut start = 0;
    for (ix, block) in blocks.iter().enumerate() {
        start += block.text.len();
        if offset < start {
            return ix;
        }
    }
    blocks.len().saturating_sub(1)
}

impl Workspace {
    /// Blockansicht der aktiven Markdown-Notiz ein- bzw. ausschalten.
    pub(crate) fn toggle_blocks(
        &mut self,
        _: &ToggleBlocks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = &self.panes[self.active_pane];
        let Some(buffer) = pane.buffers.get(pane.active) else {
            return;
        };
        if buffer.note_id.is_none() || buffer.syntax != Some(Syntax::Markdown) {
            return;
        }
        if buffer.blocks.is_some() {
            self.commit_block(cx);
            let editor = self.active_buffer().map(|b| b.editor.clone());
            if let Some(buffer) = self.active_buffer_mut() {
                buffer.blocks = None;
            }
            if let Some(editor) = editor {
                editor.update(cx, |state, cx| state.focus(window, cx));
            }
        } else {
            let text = buffer.editor.read(cx).value().to_string();
            if let Some(buffer) = self.active_buffer_mut() {
                buffer.blocks = Some(BlockView {
                    blocks: blocks::split(&text),
                    active: None,
                });
            }
        }
        cx.notify();
    }

    /// Eingabe des aktiven Blocks in den Text übernehmen und die Blöcke neu zuschneiden (wer
    /// eine Leerzeile tippt, hat damit einen neuen Block).
    fn commit_block(&mut self, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer_mut() else {
            return;
        };
        let Some(view) = buffer.blocks.as_mut() else {
            return;
        };
        if let Some(active) = view.active.take() {
            let value = active.input.read(cx).value().to_string();
            if let Some(block) = view.blocks.get(active.ix) {
                view.blocks[active.ix] = block.with_body(&value);
            }
            view.blocks = blocks::split(&blocks::join(&view.blocks));
        }
    }

    /// Block `ix` zum Bearbeiten öffnen; `end` setzt den Cursor ans Ende statt an den Anfang.
    fn activate_block(
        &mut self,
        ix: usize,
        end: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Ziel als Byte-Position merken: das Zurechtschneiden kann die Nummern verschieben.
        let target = self
            .active_buffer()
            .and_then(|b| b.blocks.as_ref())
            .map(|v| {
                let at = start_of(&v.blocks, ix);
                if end {
                    at + v.blocks.get(ix).map_or(0, |b| b.body().len())
                } else {
                    at
                }
            });
        self.commit_block(cx);
        let Some(target) = target else {
            return;
        };
        let Some(buffer) = self.active_buffer() else {
            return;
        };
        let editor = buffer.editor.clone();
        let Some(view) = buffer.blocks.as_ref() else {
            return;
        };
        let ix = block_at(&view.blocks, target);
        let body = view.blocks[ix].body().to_string();
        let input = cx.new(|cx| {
            let mut state = TextareaState::new(window, cx)
                .auto_grow(1, 60)
                .soft_wrap(true)
                .default_value(body.clone());
            if end {
                let line = body.lines().count().max(1) as u32 - 1;
                let column = body.lines().last().map_or(0, |l| l.chars().count()) as u32;
                state.set_cursor_position(Position::new(line, column), window, cx);
            }
            state
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this: &mut Workspace, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.block_edited(&editor, input, window, cx);
                }
            },
        );
        input.update(cx, |state, cx| state.focus(window, cx));
        if let Some(view) = self.active_buffer_mut().and_then(|b| b.blocks.as_mut()) {
            view.active = Some(Active {
                ix,
                input,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    /// Eingabe im aktiven Block: den ganzen Text im Editor nachführen, damit Speichern,
    /// Wortzahl und Druckvorschau wie sonst arbeiten. Zugeschnitten wird erst beim Verlassen.
    fn block_edited(
        &mut self,
        editor: &Entity<crate::EditorState>,
        input: &Entity<TextareaState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = input.read(cx).value().to_string();
        let Some(view) = self.active_buffer_mut().and_then(|b| b.blocks.as_mut()) else {
            return;
        };
        let Some(ix) = view.active.as_ref().map(|a| a.ix) else {
            return;
        };
        let mut all = view.blocks.clone();
        let Some(block) = all.get(ix) else {
            return;
        };
        all[ix] = block.with_body(&value);
        let text = blocks::join(&all);
        editor.update(cx, |state, cx| state.set_value(text, window, cx));
        // `set_value` meldet keine Änderung; Stand, Autosave und Vorschau melden wir selbst.
        self.editor_changed(editor, cx);
    }

    /// Cursor des aktiven Blocks in erster bzw. letzter Zeile? Dann führen Pfeil hoch/runter in
    /// den Nachbarblock.
    fn block_edge(&self, up: bool, cx: &App) -> Option<usize> {
        let view = self.active_buffer()?.blocks.as_ref()?;
        let active = view.active.as_ref()?;
        let input = active.input.read(cx);
        let line = input.cursor_position().line as usize;
        let last = input.value().lines().count().saturating_sub(1);
        let at_edge = if up { line == 0 } else { line >= last };
        let next = if up {
            active.ix.checked_sub(1)?
        } else {
            (active.ix + 1 < view.blocks.len()).then_some(active.ix + 1)?
        };
        at_edge.then_some(next)
    }

    /// Die Blockansicht anstelle des Editors.
    pub(crate) fn render_blocks(&self, view: &BlockView, cx: &mut Context<Self>) -> AnyElement {
        let text_font = SharedString::from(self.fonts.text.clone());
        let code_font = SharedString::from(self.fonts.code.clone());
        let mut list = v_flex().w_full().gap_1().pb_6();
        for (ix, block) in view.blocks.iter().enumerate() {
            let body = block.body();
            if let Some(active) = view.active.as_ref().filter(|a| a.ix == ix) {
                list = list.child(
                    div()
                        .id(("block", ix))
                        .w_full()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().accent.opacity(0.35))
                        .px_2()
                        .py_1()
                        .child(
                            Textarea::new(&active.input)
                                .appearance(false)
                                .bordered(false)
                                .font_family(if block.kind == Kind::Code {
                                    code_font.clone()
                                } else {
                                    text_font.clone()
                                }),
                        ),
                );
                continue;
            }
            let shown = match block.kind {
                Kind::Frontmatter => div()
                    .text_xs()
                    .font_family(code_font.clone())
                    .text_color(cx.theme().muted_foreground)
                    .whitespace_normal()
                    .child(body.to_string())
                    .into_any_element(),
                _ if body.trim().is_empty() => div()
                    .h(px(20.))
                    .text_color(cx.theme().muted_foreground)
                    .child("")
                    .into_any_element(),
                _ => TextView::markdown(("block-md", ix), body.to_string()).into_any_element(),
            };
            list = list.child(
                div()
                    .id(("block", ix))
                    .w_full()
                    .rounded(cx.theme().radius)
                    .px_2()
                    .py_0p5()
                    .cursor_text()
                    .hover(|this| this.bg(cx.theme().accent.opacity(0.2)))
                    .child(shown)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.activate_block(ix, false, window, cx)
                    })),
            );
        }
        // Unter dem letzten Block weiterschreiben: ein neuer, leerer Absatz.
        list = list.child(
            div()
                .id("block-tail")
                .w_full()
                .min_h(px(80.))
                .cursor_text()
                .on_click(cx.listener(|this, _, window, cx| this.append_block(window, cx))),
        );
        div()
            .id("blocks")
            .size_full()
            .overflow_y_scroll()
            .font_family(text_font)
            .text_size(px(17.))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let plain = !event.keystroke.modifiers.modified();
                let handled = match key {
                    "escape" => {
                        this.commit_block(cx);
                        cx.notify();
                        true
                    }
                    "up" if plain => match this.block_edge(true, cx) {
                        Some(ix) => {
                            this.activate_block(ix, true, window, cx);
                            true
                        }
                        None => false,
                    },
                    "down" if plain => match this.block_edge(false, cx) {
                        Some(ix) => {
                            this.activate_block(ix, false, window, cx);
                            true
                        }
                        None => false,
                    },
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            }))
            .child(h_flex().w_full().child(list))
            .into_any_element()
    }

    /// Neuen leeren Absatz ans Ende hängen und dort weiterschreiben.
    fn append_block(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_block(cx);
        let Some(view) = self.active_buffer_mut().and_then(|b| b.blocks.as_mut()) else {
            return;
        };
        // Zwischen dem bisher letzten und dem neuen Block steht eine Leerzeile.
        if let Some(last) = view.blocks.last_mut() {
            let missing = match last.separator().matches('\n').count() {
                0 => "\n\n",
                1 => "\n",
                _ => "",
            };
            last.text.push_str(missing);
        }
        view.blocks.push(Block {
            text: String::new(),
            kind: Kind::Paragraph,
        });
        let ix = view.blocks.len() - 1;
        self.activate_block(ix, false, window, cx);
    }
}
