//! Formatier- und Einfügeaktionen für Markdown und Typst – unabhängig von der UI.
//!
//! Jede Aktion arbeitet auf Text + Auswahl und liefert Änderungen samt neuer Auswahl.
//! So können Toolbar, Tastenkürzel, Einfüge-Palette (`/`) und Spickzettel dieselbe Logik
//! nutzen, und man muss die Syntax nicht kennen.

use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Markdown,
    Typst,
}

impl Syntax {
    pub fn for_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "md" | "mdx" | "markdown" => Some(Self::Markdown),
            "typ" => Some(Self::Typst),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Typst => "Typst",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Heading(u8),
    Bold,
    Italic,
    Strike,
    Code,
    Link,
    BulletList,
    NumberedList,
    TaskList,
    Quote,
    Image,
    Table,
    CodeBlock,
    Footnote,
    Callout,
    MathInline,
    MathBlock,
    Rule,
    PageBreak,
    Outline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Text,
    Paragraph,
    Insert,
}

impl Action {
    pub const ALL: &'static [Action] = &[
        Action::Heading(1),
        Action::Heading(2),
        Action::Heading(3),
        Action::Bold,
        Action::Italic,
        Action::Strike,
        Action::Code,
        Action::Link,
        Action::BulletList,
        Action::NumberedList,
        Action::TaskList,
        Action::Quote,
        Action::Image,
        Action::Table,
        Action::CodeBlock,
        Action::Footnote,
        Action::Callout,
        Action::MathInline,
        Action::MathBlock,
        Action::Rule,
        Action::PageBreak,
        Action::Outline,
    ];

    pub fn available(self, syntax: Syntax) -> bool {
        match self {
            Action::TaskList => syntax == Syntax::Markdown,
            Action::PageBreak | Action::Outline => syntax == Syntax::Typst,
            _ => true,
        }
    }

    pub fn group(self) -> Group {
        match self {
            Action::Bold | Action::Italic | Action::Strike | Action::Code | Action::Link => Group::Text,
            Action::Heading(_)
            | Action::BulletList
            | Action::NumberedList
            | Action::TaskList
            | Action::Quote => Group::Paragraph,
            _ => Group::Insert,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Heading(1) => "Überschrift 1",
            Action::Heading(2) => "Überschrift 2",
            Action::Heading(_) => "Überschrift 3",
            Action::Bold => "Fett",
            Action::Italic => "Kursiv",
            Action::Strike => "Durchgestrichen",
            Action::Code => "Code im Text",
            Action::Link => "Link",
            Action::BulletList => "Aufzählung",
            Action::NumberedList => "Nummerierte Liste",
            Action::TaskList => "Aufgabenliste",
            Action::Quote => "Zitat",
            Action::Image => "Bild",
            Action::Table => "Tabelle",
            Action::CodeBlock => "Codeblock",
            Action::Footnote => "Fußnote",
            Action::Callout => "Hinweisbox",
            Action::MathInline => "Formel im Text",
            Action::MathBlock => "Formel (abgesetzt)",
            Action::Rule => "Trennlinie",
            Action::PageBreak => "Seitenumbruch",
            Action::Outline => "Inhaltsverzeichnis",
        }
    }

    /// Kurze Beschriftung für Toolbar-Buttons.
    pub fn short_label(self) -> &'static str {
        match self {
            Action::Heading(1) => "H1",
            Action::Heading(2) => "H2",
            Action::Heading(_) => "H3",
            Action::Bold => "F",
            Action::Italic => "K",
            Action::Strike => "S",
            Action::Code => "</>",
            Action::Link => "Link",
            Action::BulletList => "•",
            Action::NumberedList => "1.",
            Action::TaskList => "☐",
            Action::Quote => "„“",
            Action::Image => "Bild",
            Action::Table => "Tabelle",
            Action::CodeBlock => "Code",
            Action::Footnote => "Fußnote",
            Action::Callout => "Hinweis",
            Action::MathInline => "x²",
            Action::MathBlock => "∑",
            Action::Rule => "—",
            Action::PageBreak => "Umbruch",
            Action::Outline => "Inhalt",
        }
    }

    /// Erklärung für Tooltip, Palette und Spickzettel.
    pub fn description(self) -> &'static str {
        match self {
            Action::Heading(_) => "Macht die aktuelle Zeile zur Überschrift; nochmals klicken entfernt sie.",
            Action::Bold => "Hebt die Auswahl fett hervor.",
            Action::Italic => "Setzt die Auswahl kursiv.",
            Action::Strike => "Streicht die Auswahl durch.",
            Action::Code => "Zeigt die Auswahl als Code in Festbreitenschrift.",
            Action::Link => "Verlinkt die Auswahl; danach die Adresse eintragen.",
            Action::BulletList => "Macht die ausgewählten Zeilen zu einer Aufzählung.",
            Action::NumberedList => "Macht die ausgewählten Zeilen zu einer nummerierten Liste.",
            Action::TaskList => "Liste mit Kästchen zum Abhaken.",
            Action::Quote => "Setzt die ausgewählten Zeilen als Zitat ab.",
            Action::Image => "Fügt ein Bild mit Bildunterschrift ein; danach den Dateipfad eintragen.",
            Action::Table => "Fügt eine Tabelle mit Kopfzeile ein.",
            Action::CodeBlock => "Fügt einen Block für Quelltext ein.",
            Action::Footnote => "Fügt eine Fußnote an der Cursorposition ein.",
            Action::Callout => "Fügt eine farbige Hinweisbox ein.",
            Action::MathInline => "Formel im Fließtext.",
            Action::MathBlock => "Abgesetzte Formel in eigener Zeile.",
            Action::Rule => "Horizontale Trennlinie.",
            Action::PageBreak => "Beginnt eine neue Seite.",
            Action::Outline => "Fügt ein automatisches Inhaltsverzeichnis ein.",
        }
    }

    /// Beispiel der erzeugten Syntax (Spickzettel / Hilfe).
    pub fn example(self, syntax: Syntax) -> &'static str {
        use Syntax::*;
        match (self, syntax) {
            (Action::Heading(1), Markdown) => "# Überschrift",
            (Action::Heading(2), Markdown) => "## Überschrift",
            (Action::Heading(_), Markdown) => "### Überschrift",
            (Action::Heading(1), Typst) => "= Überschrift",
            (Action::Heading(2), Typst) => "== Überschrift",
            (Action::Heading(_), Typst) => "=== Überschrift",
            (Action::Bold, Markdown) => "**fett**",
            (Action::Bold, Typst) => "*fett*",
            (Action::Italic, Markdown) => "_kursiv_",
            (Action::Italic, Typst) => "_kursiv_",
            (Action::Strike, Markdown) => "~~durchgestrichen~~",
            (Action::Strike, Typst) => "#strike[durchgestrichen]",
            (Action::Code, _) => "`code`",
            (Action::Link, Markdown) => "[Text](https://example.com)",
            (Action::Link, Typst) => "#link(\"https://example.com\")[Text]",
            (Action::BulletList, _) => "- Punkt",
            (Action::NumberedList, Markdown) => "1. Punkt",
            (Action::NumberedList, Typst) => "+ Punkt",
            (Action::TaskList, _) => "- [ ] Aufgabe",
            (Action::Quote, Markdown) => "> Zitat",
            (Action::Quote, Typst) => "#quote(block: true)[Zitat]",
            (Action::Image, Markdown) => "![Beschreibung](bild.png)",
            (Action::Image, Typst) => "#figure(image(\"bild.png\"), caption: [Beschreibung])",
            (Action::Table, Markdown) => "| A | B |\n| --- | --- |\n| 1 | 2 |",
            (Action::Table, Typst) => "#table(columns: 2, [A], [B], [1], [2])",
            (Action::CodeBlock, _) => "```rust\nfn main() {}\n```",
            (Action::Footnote, Markdown) => "Text[^1]\n\n[^1]: Fußnote",
            (Action::Footnote, Typst) => "Text#footnote[Fußnote]",
            (Action::Callout, Markdown) => "> [!NOTE]\n> Hinweis",
            (Action::Callout, Typst) => "#block(fill: luma(240), inset: 10pt)[Hinweis]",
            (Action::MathInline, _) => "$a^2 + b^2$",
            (Action::MathBlock, Markdown) => "$$\n\\frac{a}{b}\n$$",
            (Action::MathBlock, Typst) => "$ a / b $",
            (Action::Rule, Markdown) => "---",
            (Action::Rule, Typst) => "#line(length: 100%)",
            (Action::PageBreak, _) => "#pagebreak()",
            (Action::Outline, _) => "#outline()",
        }
    }

    /// Stichwörter für die Suche in der Einfüge-Palette.
    pub fn keywords(self) -> &'static str {
        match self {
            Action::Heading(_) => "überschrift titel heading h kapitel",
            Action::Bold => "fett bold stark",
            Action::Italic => "kursiv italic betont",
            Action::Strike => "durchgestrichen strike",
            Action::Code => "code inline",
            Action::Link => "link url verweis",
            Action::BulletList => "liste aufzählung bullet punkt",
            Action::NumberedList => "liste nummeriert zahl numbered",
            Action::TaskList => "aufgabe todo checkbox task",
            Action::Quote => "zitat quote",
            Action::Image => "bild image foto abbildung grafik",
            Action::Table => "tabelle table",
            Action::CodeBlock => "code block quelltext",
            Action::Footnote => "fußnote footnote anmerkung",
            Action::Callout => "hinweis box warnung callout note tipp",
            Action::MathInline => "formel mathe math inline",
            Action::MathBlock => "formel mathe math gleichung block",
            Action::Rule => "linie trennlinie rule hr",
            Action::PageBreak => "seite umbruch pagebreak",
            Action::Outline => "inhalt verzeichnis toc outline",
        }
    }
}

/// Eine Textänderung im Originaltext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub range: Range<usize>,
    pub text: String,
}

/// Ergebnis einer Aktion: Änderungen (nicht überlappend, aufsteigend) und die Auswahl
/// danach in Koordinaten des neuen Texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub changes: Vec<Change>,
    pub selection: Range<usize>,
}

impl Edit {
    /// Wendet die Änderungen auf `text` an (für Tests und UI ohne Teiländerungen).
    pub fn apply_to(&self, text: &str) -> String {
        let mut out = text.to_string();
        for change in self.changes.iter().rev() {
            out.replace_range(change.range.clone(), &change.text);
        }
        out
    }
}

pub fn apply(action: Action, syntax: Syntax, text: &str, selection: Range<usize>) -> Edit {
    let selection = clamp(text, selection);
    match action {
        Action::Bold => match syntax {
            Syntax::Markdown => wrap(text, selection, "**", "**", "fetter Text"),
            Syntax::Typst => wrap(text, selection, "*", "*", "fetter Text"),
        },
        Action::Italic => wrap(text, selection, "_", "_", "kursiver Text"),
        Action::Strike => match syntax {
            Syntax::Markdown => wrap(text, selection, "~~", "~~", "Text"),
            Syntax::Typst => wrap(text, selection, "#strike[", "]", "Text"),
        },
        Action::Code => wrap(text, selection, "`", "`", "code"),
        Action::Link => link(text, selection, syntax),
        Action::Heading(level) => heading(text, selection, syntax, level.clamp(1, 6)),
        Action::BulletList => line_prefix(text, selection, ListKind::Bullet, syntax),
        Action::NumberedList => line_prefix(text, selection, ListKind::Numbered, syntax),
        Action::TaskList => line_prefix(text, selection, ListKind::Task, syntax),
        Action::Quote => match syntax {
            Syntax::Markdown => line_prefix(text, selection, ListKind::Quote, syntax),
            Syntax::Typst => block_wrap(text, selection, "#quote(block: true)[\n", "\n]", "Zitat"),
        },
        Action::Image => match syntax {
            Syntax::Markdown => {
                let alt = selected_or(text, &selection, "Beschreibung");
                insert_block(text, selection, &format!("![{alt}]("), "bild.png", ")")
            }
            Syntax::Typst => {
                let caption = selected_or(text, &selection, "Beschreibung");
                insert_block(text, selection, "#figure(\n  image(\"", "bild.png", &format!("\"),\n  caption: [{caption}],\n)"))
            }
        },
        Action::Table => match syntax {
            Syntax::Markdown => insert_block(text, selection, "| ", "Spalte 1", " | Spalte 2 |\n| --- | --- |\n| Wert | Wert |"),
            Syntax::Typst => insert_block(
                text,
                selection,
                "#table(\n  columns: 2,\n  table.header([",
                "Spalte 1",
                "], [Spalte 2]),\n  [Wert], [Wert],\n)",
            ),
        },
        Action::CodeBlock => {
            let code = selected_or(text, &selection, "Code");
            insert_block(text, selection, "```", "text", &format!("\n{code}\n```"))
        }
        Action::Footnote => footnote(text, selection, syntax),
        Action::Callout => match syntax {
            Syntax::Markdown => {
                let body = selected_or(text, &selection, "Hinweistext");
                let body = body.lines().collect::<Vec<_>>().join("\n> ");
                insert_block(text, selection, "> [!NOTE]\n> ", &body, "")
            }
            Syntax::Typst => block_wrap(
                text,
                selection,
                "#block(fill: luma(240), inset: 10pt, radius: 4pt, width: 100%)[\n  *Hinweis:* ",
                "\n]",
                "Hinweistext",
            ),
        },
        Action::MathInline => wrap(text, selection, "$", "$", "a^2 + b^2"),
        Action::MathBlock => match syntax {
            Syntax::Markdown => block_wrap(text, selection, "$$\n", "\n$$", "a^2 + b^2 = c^2"),
            Syntax::Typst => block_wrap(text, selection, "$ ", " $", "a^2 + b^2 = c^2"),
        },
        Action::Rule => match syntax {
            Syntax::Markdown => insert_block(text, selection, "---", "", ""),
            Syntax::Typst => insert_block(text, selection, "#line(length: 100%)", "", ""),
        },
        Action::PageBreak => insert_block(text, selection, "#pagebreak()", "", ""),
        Action::Outline => insert_block(text, selection, "#outline()", "", ""),
    }
}

fn clamp(text: &str, selection: Range<usize>) -> Range<usize> {
    let floor = |mut i: usize| {
        i = i.min(text.len());
        while !text.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let (a, b) = (floor(selection.start), floor(selection.end));
    a.min(b)..a.max(b)
}

fn selected_or(text: &str, selection: &Range<usize>, placeholder: &str) -> String {
    let selected = &text[selection.clone()];
    if selected.trim().is_empty() {
        placeholder.to_string()
    } else {
        selected.to_string()
    }
}

/// Umschließt die Auswahl; ist sie schon umschlossen, wird die Markierung entfernt.
fn wrap(text: &str, selection: Range<usize>, pre: &str, post: &str, placeholder: &str) -> Edit {
    let selected = &text[selection.clone()];
    // Markierungen direkt um die Auswahl → entfernen.
    if selection.start >= pre.len()
        && text[..selection.start].ends_with(pre)
        && text[selection.end..].starts_with(post)
    {
        let start = selection.start - pre.len();
        return Edit {
            changes: vec![Change { range: start..selection.end + post.len(), text: selected.to_string() }],
            selection: start..start + selected.len(),
        };
    }
    // Markierungen in der Auswahl → entfernen.
    if selected.len() >= pre.len() + post.len() && selected.starts_with(pre) && selected.ends_with(post) {
        let inner = &selected[pre.len()..selected.len() - post.len()];
        return Edit {
            changes: vec![Change { range: selection.clone(), text: inner.to_string() }],
            selection: selection.start..selection.start + inner.len(),
        };
    }
    let inner = if selected.is_empty() { placeholder } else { selected };
    let start = selection.start + pre.len();
    Edit {
        changes: vec![Change { range: selection, text: format!("{pre}{inner}{post}") }],
        selection: start..start + inner.len(),
    }
}

fn link(text: &str, selection: Range<usize>, syntax: Syntax) -> Edit {
    let selected = &text[selection.clone()];
    let looks_like_url = selected.starts_with("http://") || selected.starts_with("https://");
    let (label, url) = match (selected.is_empty(), looks_like_url) {
        (true, _) => ("Linktext".to_string(), "https://".to_string()),
        (false, true) => (selected.to_string(), selected.to_string()),
        (false, false) => (selected.to_string(), "https://".to_string()),
    };
    let (replacement, url_offset) = match syntax {
        Syntax::Markdown => (format!("[{label}]({url})"), label.len() + 3),
        Syntax::Typst => (format!("#link(\"{url}\")[{label}]"), 7),
    };
    let start = selection.start;
    // Ohne Auswahl den Linktext markieren, sonst die Adresse.
    let select = if selected.is_empty() {
        let label_offset = match syntax {
            Syntax::Markdown => 1,
            Syntax::Typst => url.len() + 10,
        };
        start + label_offset..start + label_offset + label.len()
    } else {
        start + url_offset..start + url_offset + url.len()
    };
    Edit { changes: vec![Change { range: selection, text: replacement }], selection: select }
}

/// Bereich der vollständigen Zeilen, die die Auswahl berührt.
fn line_range(text: &str, selection: &Range<usize>) -> Range<usize> {
    let start = text[..selection.start].rfind('\n').map_or(0, |i| i + 1);
    // Endet die Auswahl direkt hinter einem Zeilenumbruch, gehört die nächste Zeile nicht dazu.
    let end_anchor = if selection.end > selection.start && text[..selection.end].ends_with('\n') {
        selection.end - 1
    } else {
        selection.end
    };
    let end = text[end_anchor..].find('\n').map_or(text.len(), |i| end_anchor + i);
    start..end.max(start)
}

fn heading_prefix(line: &str, syntax: Syntax) -> (usize, usize) {
    let marker = match syntax {
        Syntax::Markdown => '#',
        Syntax::Typst => '=',
    };
    let level = line.chars().take_while(|&c| c == marker).count();
    if level > 0 && (line.len() == level || line[level..].starts_with(' ')) {
        let len = if line.len() > level { level + 1 } else { level };
        (level, len)
    } else {
        (0, 0)
    }
}

fn heading(text: &str, selection: Range<usize>, syntax: Syntax, level: u8) -> Edit {
    let lines = line_range(text, &selection);
    let line = &text[lines.clone()];
    let (current, prefix_len) = heading_prefix(line, syntax);
    let content = line[prefix_len..].trim_start();
    if current == level as usize {
        // Gleiche Ebene → Überschrift entfernen.
        return Edit {
            changes: vec![Change { range: lines.start..lines.start + prefix_len, text: String::new() }],
            selection: lines.start..lines.start + content.len(),
        };
    }
    let marker = match syntax {
        Syntax::Markdown => "#",
        Syntax::Typst => "=",
    };
    let prefix = format!("{} ", marker.repeat(level as usize));
    let content = if content.is_empty() { "Überschrift" } else { content };
    let new_line = format!("{prefix}{content}");
    let start = lines.start + prefix.len();
    Edit {
        changes: vec![Change { range: lines, text: new_line }],
        selection: start..start + content.len(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Bullet,
    Numbered,
    Task,
    Quote,
}

/// Länge eines vorhandenen Listen-/Zitatpräfixes nach Einrückung und welcher Art er ist.
fn existing_prefix(line: &str, syntax: Syntax) -> Option<(ListKind, usize)> {
    if line.starts_with("- [ ] ") || line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        return Some((ListKind::Task, 6));
    }
    if line.starts_with("- ") || line.starts_with("* ") || (syntax == Syntax::Markdown && line.starts_with("+ ")) {
        return Some((ListKind::Bullet, 2));
    }
    if syntax == Syntax::Typst && line.starts_with("+ ") {
        return Some((ListKind::Numbered, 2));
    }
    if line.starts_with("> ") {
        return Some((ListKind::Quote, 2));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return Some((ListKind::Numbered, digits + 2));
    }
    None
}

fn line_prefix(text: &str, selection: Range<usize>, kind: ListKind, syntax: Syntax) -> Edit {
    let lines = line_range(text, &selection);
    let block = &text[lines.clone()];
    let rows: Vec<&str> = block.split('\n').collect();
    let content_rows: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            let trimmed = row.trim_start();
            (&row[..row.len() - trimmed.len()], trimmed)
        })
        .collect();
    let all_have = content_rows
        .iter()
        .filter(|(_, row)| !row.is_empty())
        .all(|(_, row)| existing_prefix(row, syntax).is_some_and(|(k, _)| k == kind))
        && content_rows.iter().any(|(_, row)| !row.is_empty());

    let mut number = 0;
    let new_rows: Vec<String> = content_rows
        .iter()
        .map(|(indent, row)| {
            let stripped = match existing_prefix(row, syntax) {
                Some((_, len)) => &row[len..],
                None => row,
            };
            if all_have {
                return format!("{indent}{stripped}");
            }
            if row.is_empty() && rows.len() > 1 {
                return String::new();
            }
            number += 1;
            let prefix = match (kind, syntax) {
                (ListKind::Bullet, _) => "- ".to_string(),
                (ListKind::Numbered, Syntax::Markdown) => format!("{number}. "),
                (ListKind::Numbered, Syntax::Typst) => "+ ".to_string(),
                (ListKind::Task, _) => "- [ ] ".to_string(),
                (ListKind::Quote, _) => "> ".to_string(),
            };
            format!("{indent}{prefix}{stripped}")
        })
        .collect();
    let new_block = new_rows.join("\n");
    let selection = if rows.len() == 1 {
        // Einzelne Zeile: Cursor ans Zeilenende.
        let end = lines.start + new_block.len();
        end..end
    } else {
        lines.start..lines.start + new_block.len()
    };
    Edit { changes: vec![Change { range: lines, text: new_block }], selection }
}

/// Fügt einen Block als eigenen Absatz ein (Leerzeilen davor/danach nach Bedarf) und
/// markiert `placeholder`.
fn insert_block(text: &str, selection: Range<usize>, before: &str, placeholder: &str, after: &str) -> Edit {
    let line_start = text[..selection.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[selection.end..].find('\n').map_or(text.len(), |i| selection.end + i);
    let line_empty = text[line_start..selection.start].trim().is_empty()
        && text[selection.end..line_end].trim().is_empty();

    // Leere Zeile → dort einfügen; sonst nach der aktuellen Zeile als neuer Absatz.
    let (range, mut lead) = if line_empty {
        (line_start..line_end, String::new())
    } else if selection.is_empty() {
        (line_end..line_end, "\n\n".to_string())
    } else {
        (selection.clone(), String::new())
    };
    if !line_empty && !selection.is_empty() {
        // Auswahl wird ersetzt (z. B. Code → Codeblock); Absatz drumherum herstellen.
        if !text[..range.start].is_empty() && !text[..range.start].ends_with("\n\n") {
            lead = if text[..range.start].ends_with('\n') { "\n".into() } else { "\n\n".into() };
        }
    } else if line_empty && range.start > 0 && !text[..range.start].ends_with("\n\n") {
        lead = "\n".into();
    }
    let rest = &text[range.end..];
    let trail = if rest.is_empty() || rest.starts_with("\n\n") {
        ""
    } else if rest.starts_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let start = range.start + lead.len() + before.len();
    Edit {
        changes: vec![Change { range, text: format!("{lead}{before}{placeholder}{after}{trail}") }],
        selection: start..start + placeholder.len(),
    }
}

/// Wie `insert_block`, aber die Auswahl (oder der Platzhalter) steht zwischen `pre` und `post`.
fn block_wrap(text: &str, selection: Range<usize>, pre: &str, post: &str, placeholder: &str) -> Edit {
    let content = selected_or(text, &selection, placeholder);
    insert_block(text, selection, pre, &content, post)
}

fn footnote(text: &str, selection: Range<usize>, syntax: Syntax) -> Edit {
    match syntax {
        Syntax::Typst => {
            let body = selected_or(text, &selection, "Fußnotentext");
            let replacement = format!("#footnote[{body}]");
            let start = selection.start + "#footnote[".len();
            Edit {
                changes: vec![Change { range: selection, text: replacement }],
                selection: start..start + body.len(),
            }
        }
        Syntax::Markdown => {
            let mut n = 1;
            while text.contains(&format!("[^{n}]")) {
                n += 1;
            }
            let reference = format!("[^{n}]");
            let body = selected_or(text, &selection, "Fußnotentext");
            // Verweis an die Cursorposition (Auswahl wird zum Fußnotentext), Definition ans Ende.
            let separator = if text.ends_with("\n\n") {
                ""
            } else if text.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            let definition = format!("{separator}{reference}: {body}\n");
            let removed = selection.len();
            let new_len = text.len() - removed + reference.len();
            let body_start = new_len + separator.len() + reference.len() + 2;
            Edit {
                changes: vec![
                    Change { range: selection, text: reference },
                    Change { range: text.len()..text.len(), text: definition },
                ],
                selection: body_start..body_start + body.len(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(action: Action, syntax: Syntax, text: &str, sel: Range<usize>) -> (String, String) {
        let edit = apply(action, syntax, text, sel);
        let out = edit.apply_to(text);
        let selected = out[edit.selection.clone()].to_string();
        (out, selected)
    }

    #[test]
    fn bold_wraps_and_toggles() {
        assert_eq!(run(Action::Bold, Syntax::Markdown, "ein Wort", 4..8), ("ein **Wort**".into(), "Wort".into()));
        assert_eq!(run(Action::Bold, Syntax::Markdown, "ein **Wort**", 6..10), ("ein Wort".into(), "Wort".into()));
        assert_eq!(run(Action::Bold, Syntax::Typst, "x", 1..1), ("x*fetter Text*".into(), "fetter Text".into()));
    }

    #[test]
    fn headings_set_change_and_remove() {
        assert_eq!(run(Action::Heading(2), Syntax::Markdown, "Titel", 2..2), ("## Titel".into(), "Titel".into()));
        assert_eq!(run(Action::Heading(1), Syntax::Markdown, "## Titel", 4..4), ("# Titel".into(), "Titel".into()));
        assert_eq!(run(Action::Heading(1), Syntax::Typst, "= Titel", 3..3), ("Titel".into(), "Titel".into()));
        assert_eq!(run(Action::Heading(2), Syntax::Typst, "a\n\nb", 2..2), ("a\n== Überschrift\nb".into(), "Überschrift".into()));
    }

    #[test]
    fn lists_prefix_lines_and_toggle() {
        let (out, _) = run(Action::NumberedList, Syntax::Markdown, "a\nb\nc", 0..5);
        assert_eq!(out, "1. a\n2. b\n3. c");
        let (out, _) = run(Action::NumberedList, Syntax::Markdown, &out, 0..out.len());
        assert_eq!(out, "a\nb\nc");
        let (out, _) = run(Action::BulletList, Syntax::Typst, "+ a\n+ b", 0..7);
        assert_eq!(out, "- a\n- b");
        let (out, _) = run(Action::Quote, Syntax::Markdown, "Zitat", 0..0);
        assert_eq!(out, "> Zitat");
    }

    #[test]
    fn links() {
        assert_eq!(run(Action::Link, Syntax::Markdown, "Typst", 0..5), ("[Typst](https://)".into(), "https://".into()));
        assert_eq!(run(Action::Link, Syntax::Typst, "", 0..0), ("#link(\"https://\")[Linktext]".into(), "Linktext".into()));
    }

    #[test]
    fn blocks_become_paragraphs() {
        let (out, selected) = run(Action::Table, Syntax::Markdown, "Text", 4..4);
        assert_eq!(out, "Text\n\n| Spalte 1 | Spalte 2 |\n| --- | --- |\n| Wert | Wert |");
        assert_eq!(selected, "Spalte 1");
        let (out, selected) = run(Action::Image, Syntax::Typst, "a\n\n\n\nb", 3..3);
        assert_eq!(out, "a\n\n#figure(\n  image(\"bild.png\"),\n  caption: [Beschreibung],\n)\n\nb");
        assert_eq!(selected, "bild.png");
        let (out, _) = run(Action::CodeBlock, Syntax::Markdown, "x = 1", 0..5);
        assert_eq!(out, "```text\nx = 1\n```");
    }

    #[test]
    fn footnotes() {
        let (out, selected) = run(Action::Footnote, Syntax::Markdown, "Satz.", 5..5);
        assert_eq!(out, "Satz.[^1]\n\n[^1]: Fußnotentext\n");
        assert_eq!(selected, "Fußnotentext");
        let (out, selected) = run(Action::Footnote, Syntax::Typst, "Satz.", 5..5);
        assert_eq!(out, "Satz.#footnote[Fußnotentext]");
        assert_eq!(selected, "Fußnotentext");
    }

    #[test]
    fn every_action_produces_valid_selection() {
        let samples = ["", "Text", "Zeile 1\nZeile 2\n", "äöü ß"];
        for syntax in [Syntax::Markdown, Syntax::Typst] {
            for &action in Action::ALL.iter().filter(|a| a.available(syntax)) {
                for text in samples {
                    for sel in [0..0, 0..text.len(), text.len()..text.len()] {
                        let edit = apply(action, syntax, text, sel);
                        let out = edit.apply_to(text);
                        assert!(out.get(edit.selection.clone()).is_some(), "{action:?} {syntax:?} {text:?}");
                    }
                }
            }
        }
    }
}
