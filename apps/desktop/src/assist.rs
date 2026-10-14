//! Schreibhilfen im Editor ohne Language Server: Einfüge-Liste per `/` am Zeilenanfang und
//! Erklärungen beim Überfahren von Markdown-/Typst-Syntax.

use devmark_writing::authoring::{self, Action, Syntax};
use gpui_kit::component::input::{CompletionProvider, HoverProvider, Position, Rope, RopeExt};
use gpui_kit::{App, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Hover, HoverContents, MarkupContent, MarkupKind, Range, TextEdit,
};

pub struct WritingAssist {
    pub syntax: Syntax,
}

impl WritingAssist {
    /// Text vor dem Cursor in der aktuellen Zeile.
    fn line_prefix(text: &Rope, offset: usize) -> (usize, String) {
        let position = text.offset_to_position(offset);
        let line_start = text.position_to_offset(&Position::new(position.line, 0));
        (line_start, text.slice(line_start..offset).to_string())
    }
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
        let (line_start, prefix) = Self::line_prefix(text, offset);
        let indent = prefix.len() - prefix.trim_start().len();
        let Some(query) = prefix.trim_start().strip_prefix('/') else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let query = query.to_lowercase();
        let slash = line_start + indent;
        let range = Range::new(text.offset_to_position(slash), text.offset_to_position(offset));

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
                let snippet = authoring::apply(action, self.syntax, "", 0..0).apply_to("");
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
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    /// `/` öffnet die Liste; weitere Zeichen verfeinern sie (ohne `/` am Zeilenanfang liefert
    /// `completions` nichts, dann bleibt die Liste zu).
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        new_text == "/" || (new_text.chars().count() == 1 && new_text.chars().all(char::is_alphanumeric))
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
}
