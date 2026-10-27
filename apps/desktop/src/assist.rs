//! Schreibhilfen im Editor ohne Language Server: Einfüge-Liste per `/` am Zeilenanfang,
//! Erklärungen beim Überfahren von Markdown-/Typst-Syntax und Verknüpfungen zwischen Notizen
//! als Markdown-Links (Eingabe per `[[`, Cmd-Klick öffnet).

use std::cell::RefCell;
use std::rc::Rc;

use devmark_notes::{NoteLink, link_markup, note_links, resolve};
use devmark_writing::authoring::{self, Action, Syntax};
use gpui_kit::component::input::{CompletionProvider, DefinitionProvider, HoverProvider, Position, Rope, RopeExt};
use gpui_kit::{App, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Hover, HoverContents, LocationLink, MarkupContent, MarkupKind, Range, TextEdit, Uri,
};

/// Notizen für Verknüpfungen: (id, Titel), von der Notizliste aktuell gehalten.
pub type NoteIndex = Rc<RefCell<Vec<(String, String)>>>;

const NOTE_URI_PREFIX: &str = "devmark-note:";

/// Notiz-ID aus dem Sprungziel einer Verknüpfung.
pub fn note_id_from_uri(uri: &Uri) -> Option<&str> {
    uri.as_str().strip_prefix(NOTE_URI_PREFIX)
}

pub struct WritingAssist {
    pub syntax: Syntax,
    /// Gesetzt in Markdown-Notizen: dann gibt es Verknüpfungen.
    pub notes: Option<NoteIndex>,
    /// ID der Notiz selbst (Links sind relativ dazu); zieht beim Umbenennen nach.
    pub source: RefCell<String>,
    /// Zuletzt angebotene `/`-Einfügungen: Position des `/`, Einfügetexte mit Platzhaltern.
    pub offered: RefCell<Option<(usize, Vec<Snippet>)>>,
}

/// Einfügetext einer `/`-Aktion und seine Platzhalter relativ zum Anfang.
pub struct Snippet {
    text: String,
    placeholders: Vec<std::ops::Range<usize>>,
}

impl WritingAssist {
    /// Text vor dem Cursor in der aktuellen Zeile.
    fn line_prefix(text: &Rope, offset: usize) -> (usize, String) {
        let position = text.offset_to_position(offset);
        let line_start = text.position_to_offset(&Position::new(position.line, 0));
        (line_start, text.slice(line_start..offset).to_string())
    }

    /// Wurde gerade eine angebotene `/`-Einfügung übernommen (Text endet am Cursor)? Dann
    /// ihre Platzhalter im ganzen Text.
    pub fn accepted_snippet(&self, text: &Rope, cursor: usize) -> Option<Vec<std::ops::Range<usize>>> {
        let mut offered = self.offered.borrow_mut();
        let (slash, snippets) = offered.as_ref()?;
        let slash = (*slash <= cursor).then_some(*slash)?;
        let inserted = text.slice(slash..cursor).to_string();
        let snippet = snippets.iter().find(|s| s.text == inserted)?;
        let placeholders = snippet.placeholders.iter().map(|r| slash + r.start..slash + r.end).collect();
        *offered = None;
        Some(placeholders)
    }

    /// Verknüpfung an `offset` (Bereich im ganzen Text) und die aufgelöste Notiz-ID.
    fn link_at(&self, text: &Rope, offset: usize) -> Option<(NoteLink, Option<String>)> {
        let index = self.notes.as_ref()?;
        let position = text.offset_to_position(offset);
        let line_start = text.position_to_offset(&Position::new(position.line, 0));
        let line_end = text.position_to_offset(&Position::new(position.line + 1, 0)).max(line_start);
        let line = text.slice(line_start..line_end).to_string();
        let column = offset - line_start;
        let mut link = note_links(&line).into_iter().find(|l| l.range.contains(&column))?;
        link.range = line_start + link.range.start..line_start + link.range.end;
        let id = resolve(&self.source.borrow(), &link.target)
            .filter(|id| index.borrow().iter().any(|(known, _)| known == id));
        Some((link, id))
    }

    fn link_completions(&self, index: &NoteIndex, text: &Rope, offset: usize) -> Option<Vec<CompletionItem>> {
        let (line_start, prefix) = Self::line_prefix(text, offset);
        let (start, query) = link_query(&prefix)?;
        // Schon getippte schließende Klammern mit ersetzen.
        let position = text.offset_to_position(offset);
        let line_end = text.position_to_offset(&Position::new(position.line + 1, 0)).max(offset);
        let closing = text.slice(offset..line_end).to_string().chars().take(2).take_while(|&c| c == ']').count();
        let range = Range::new(
            text.offset_to_position(line_start + start),
            text.offset_to_position(offset + closing),
        );
        let query = query.trim().to_lowercase();
        let index = index.borrow();
        let source = self.source.borrow();
        let mut matches: Vec<&(String, String)> = index
            .iter()
            .filter(|(id, title)| *id != *source && title.to_lowercase().contains(&query))
            .collect();
        // Titel, die mit der Eingabe beginnen, zuerst; sonst Reihenfolge der Notizliste.
        matches.sort_by_key(|(_, title)| !title.to_lowercase().starts_with(&query));
        Some(
            matches
                .into_iter()
                .take(50)
                .enumerate()
                .map(|(ix, (id, title))| CompletionItem {
                    label: title.clone(),
                    kind: Some(CompletionItemKind::REFERENCE),
                    detail: Some("Notiz verknüpfen".into()),
                    sort_text: Some(format!("{ix:03}")),
                    // Siehe unten: Umlaute im Label.
                    filter_text: Some(String::new()),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: link_markup(title, &source, id) })),
                    ..Default::default()
                })
                .collect(),
        )
    }
}

/// Offenes `[[` vor dem Cursor: Byte-Position im Präfix und bisher getippter Suchtext.
fn link_query(prefix: &str) -> Option<(usize, &str)> {
    let start = prefix.rfind("[[")?;
    let query = &prefix[start + 2..];
    (!query.contains(['|', ']'])).then_some((start, query))
}

impl CompletionProvider for WritingAssist {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        if let Some(index) = &self.notes
            && let Some(items) = self.link_completions(index, text, offset)
        {
            *self.offered.borrow_mut() = None;
            return Task::ready(Ok(CompletionResponse::Array(items)));
        }
        let (line_start, prefix) = Self::line_prefix(text, offset);
        let indent = prefix.len() - prefix.trim_start().len();
        let Some(query) = prefix.trim_start().strip_prefix('/') else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let query = query.to_lowercase();
        let slash = line_start + indent;
        let range = Range::new(text.offset_to_position(slash), text.offset_to_position(offset));

        let mut snippets = Vec::new();
        let items = Action::ALL
            .iter()
            .filter(|action| action.available(self.syntax))
            .filter(|action| {
                query.is_empty()
                    || action.label().to_lowercase().contains(&query)
                    || action.keywords().split(' ').any(|k| k.starts_with(&query))
            })
            .enumerate()
            .map(|(ix, &action)| {
                // Einfügetext = Ergebnis der Aktion auf einer leeren Zeile, mit Platzhaltern.
                let edit = authoring::apply(action, self.syntax, "", 0..0);
                let snippet = edit.apply_to("");
                snippets.push(Snippet { text: snippet.clone(), placeholders: edit.placeholders });
                CompletionItem {
                    label: action.label().to_string(),
                    kind: Some(CompletionItemKind::SNIPPET),
                    detail: Some(action.description().to_string()),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: format!("```\n{}\n```", action.example(self.syntax)),
                    })),
                    sort_text: Some(format!("{ix:03}")),
                    // gpui-kit 0.7.3 hebt `filter_text.len()` Bytes des Labels hervor und schneidet
                    // dabei Umlaute („Fußnote“) mitten im Zeichen → Absturz. Leer = keine Hervorhebung.
                    filter_text: Some(String::new()),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: snippet })),
                    ..Default::default()
                }
            })
            .collect();
        *self.offered.borrow_mut() = Some((slash, snippets));
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    /// `/` bzw. `[[` öffnet die Liste; weitere Zeichen verfeinern sie (ohne `/` am Zeilenanfang
    /// oder offenes `[[` liefert `completions` nichts, dann bleibt die Liste zu).
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        new_text == "/" || new_text == "[" || (new_text.chars().count() == 1 && new_text.chars().all(char::is_alphanumeric))
    }
}

impl HoverProvider for WritingAssist {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Option<Hover>>> {
        if let Some((link, id)) = self.link_at(text, offset) {
            let title = id.and_then(|id| {
                let index = self.notes.as_ref()?.borrow();
                index.iter().find(|(i, _)| *i == id).map(|(_, t)| t.clone())
            });
            let value = match title {
                Some(title) => format!("Notiz **{title}** – Cmd-Klick öffnet sie."),
                None => format!("Keine Notiz „{}“ gefunden (gelöscht oder im Papierkorb?).", link.target),
            };
            return Task::ready(Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value }),
                range: None,
            })));
        }
        let position = text.offset_to_position(offset);
        let line_start = text.position_to_offset(&Position::new(position.line, 0));
        let line_end = text.position_to_offset(&Position::new(position.line + 1, 0));
        let line = text.slice(line_start..line_end.max(line_start)).to_string();
        let word = text.word_at(offset);
        let help = explain(self.syntax, line.trim_end_matches('\n'), offset - line_start, &word);
        Task::ready(Ok(help.map(|value| Hover {
            contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value }),
            range: None,
        })))
    }
}

/// Cmd-Klick auf eine Verknüpfung: Ziel `devmark-note:<id>`, geöffnet über `show_document`.
impl DefinitionProvider for WritingAssist {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        let Some((link, Some(id))) = self.link_at(text, offset) else {
            return Task::ready(Ok(Vec::new()));
        };
        let target = format!("{NOTE_URI_PREFIX}{id}").parse::<Uri>();
        Task::ready(target.map_err(anyhow::Error::from).map(|target_uri| {
            vec![LocationLink {
                origin_selection_range: Some(Range::new(
                    text.offset_to_position(link.range.start),
                    text.offset_to_position(link.range.end),
                )),
                target_uri,
                target_range: Range::default(),
                target_selection_range: Range::default(),
            }]
        }))
    }
}

/// Erklärung für die Syntax an der Cursorposition – kurz, mit Beispiel.
fn explain(syntax: Syntax, line: &str, column: usize, word: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let at_marker = column <= line.len() - trimmed.len() + 6;
    let action = match syntax {
        Syntax::Markdown if at_marker && trimmed.starts_with('#') => {
            let level = trimmed.chars().take_while(|&c| c == '#').count().min(3) as u8;
            Some(Action::Heading(level))
        }
        Syntax::Typst if at_marker && trimmed.starts_with('=') => {
            let level = trimmed.chars().take_while(|&c| c == '=').count().min(3) as u8;
            Some(Action::Heading(level))
        }
        _ if at_marker && (trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]")) => Some(Action::TaskList),
        _ if at_marker && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) => Some(Action::BulletList),
        Syntax::Typst if at_marker && trimmed.starts_with("+ ") => Some(Action::NumberedList),
        Syntax::Markdown if at_marker && trimmed.starts_with("> [!") => Some(Action::Callout),
        Syntax::Markdown if at_marker && trimmed.starts_with('>') => Some(Action::Quote),
        Syntax::Markdown if at_marker && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && trimmed.contains(". ") => {
            Some(Action::NumberedList)
        }
        _ if at_marker && trimmed.starts_with("```") => Some(Action::CodeBlock),
        _ if at_marker && trimmed.starts_with('|') => Some(Action::Table),
        Syntax::Markdown if trimmed.starts_with("$$") => Some(Action::MathBlock),
        Syntax::Markdown if at_marker && trimmed.starts_with("![") => Some(Action::Image),
        Syntax::Markdown if at_marker && trimmed.starts_with("[^") => Some(Action::Footnote),
        Syntax::Typst => typst_function(word),
        _ => None,
    }?;
    Some(format!(
        "**{}** – {}\n\n```\n{}\n```",
        action.label(),
        action.description(),
        action.example(syntax)
    ))
}

fn typst_function(word: &str) -> Option<Action> {
    Some(match word.trim_start_matches('#') {
        "strike" => Action::Strike,
        "link" => Action::Link,
        "quote" => Action::Quote,
        "figure" | "image" => Action::Image,
        "table" => Action::Table,
        "footnote" => Action::Footnote,
        "pagebreak" => Action::PageBreak,
        "outline" => Action::Outline,
        "line" => Action::Rule,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_markdown_headings_and_typst_functions() {
        assert!(explain(Syntax::Markdown, "## Titel", 1, "").unwrap().contains("Überschrift 2"));
        assert!(explain(Syntax::Typst, "Text #footnote[x]", 7, "footnote").unwrap().contains("Fußnote"));
        assert!(explain(Syntax::Markdown, "Normaler Text", 3, "Normaler").is_none());
    }

    #[test]
    fn link_query_needs_open_brackets() {
        assert_eq!(link_query("Siehe [[Ein"), Some((6, "Ein")));
        assert_eq!(link_query("[["), Some((0, "")));
        assert_eq!(link_query("[[A|id]] weiter"), None);
        assert_eq!(link_query("kein Link"), None);
    }
}
