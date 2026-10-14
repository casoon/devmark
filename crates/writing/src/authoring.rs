//! Formatier- und Einfügeaktionen für Markdown und Typst – unabhängig von der UI.
//!
//! Jede Aktion arbeitet auf Text + Auswahl und liefert Änderungen samt Platzhaltern, die
//! nacheinander per Tab angesprungen werden können (siehe [`Tabstops`]).
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

/// Ergebnis einer Aktion: Änderungen (nicht überlappend, aufsteigend) und die Platzhalter
/// in Koordinaten des neuen Texts, in Tab-Reihenfolge. Der erste ist die Auswahl danach;
/// es gibt immer mindestens einen (ggf. leer = nur Cursor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub changes: Vec<Change>,
    pub placeholders: Vec<Range<usize>>,
}

impl Edit {
    fn new(changes: Vec<Change>, selection: Range<usize>) -> Self {
        Self { changes, placeholders: vec![selection] }
    }

    /// Auswahl direkt nach der Aktion.
    pub fn selection(&self) -> Range<usize> {
        self.placeholders[0].clone()
    }

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
                // Erst der Pfad, dann die Beschreibung.
                let mut edit = insert_block(text, selection, &["![", &alt, "](", "bild.png", ")"]);
                edit.placeholders.rotate_left(1);
                edit
            }
            Syntax::Typst => {
                let caption = selected_or(text, &selection, "Beschreibung");
                insert_block(text, selection, &["#figure(\n  image(\"", "bild.png", "\"),\n  caption: [", &caption, "],\n)"])
            }
        },
        Action::Table => match syntax {
            Syntax::Markdown => insert_block(
                text,
                selection,
                &["| ", "Spalte 1", " | ", "Spalte 2", " |\n| --- | --- |\n| ", "Wert", " | ", "Wert", " |"],
            ),
            Syntax::Typst => insert_block(
                text,
                selection,
                &["#table(\n  columns: 2,\n  table.header([", "Spalte 1", "], [", "Spalte 2", "]),\n  [", "Wert", "], [", "Wert", "],\n)"],
            ),
        },
        Action::CodeBlock => {
            let code = selected_or(text, &selection, "Code");
            insert_block(text, selection, &["```", "text", "\n", &code, "\n```"])
        }
        Action::Footnote => footnote(text, selection, syntax),
        Action::Callout => match syntax {
            Syntax::Markdown => {
                let body = selected_or(text, &selection, "Hinweistext");
                let body = body.lines().collect::<Vec<_>>().join("\n> ");
                insert_block(text, selection, &["> [!NOTE]\n> ", &body])
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
            Syntax::Markdown => insert_block(text, selection, &["---"]),
            Syntax::Typst => insert_block(text, selection, &["#line(length: 100%)"]),
        },
        Action::PageBreak => insert_block(text, selection, &["#pagebreak()"]),
        Action::Outline => insert_block(text, selection, &["#outline()"]),
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
        return Edit::new(
            vec![Change { range: start..selection.end + post.len(), text: selected.to_string() }],
            start..start + selected.len(),
        );
    }
    // Markierungen in der Auswahl → entfernen.
    if selected.len() >= pre.len() + post.len() && selected.starts_with(pre) && selected.ends_with(post) {
        let inner = &selected[pre.len()..selected.len() - post.len()];
        return Edit::new(
            vec![Change { range: selection.clone(), text: inner.to_string() }],
            selection.start..selection.start + inner.len(),
        );
    }
    let inner = if selected.is_empty() { placeholder } else { selected };
    let start = selection.start + pre.len();
    Edit::new(vec![Change { range: selection, text: format!("{pre}{inner}{post}") }], start..start + inner.len())
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
    let label_offset = match syntax {
        Syntax::Markdown => 1,
        Syntax::Typst => url.len() + 10,
    };
    let label_range = start + label_offset..start + label_offset + label.len();
    let url_range = start + url_offset..start + url_offset + url.len();
    // Ohne Auswahl erst Linktext, dann Adresse; ist die Auswahl schon eine Adresse, nur den
    // Linktext; sonst nur die Adresse.
    let placeholders = match (selected.is_empty(), looks_like_url) {
        (true, _) => vec![label_range, url_range],
        (false, true) => vec![label_range],
        (false, false) => vec![url_range],
    };
    Edit { changes: vec![Change { range: selection, text: replacement }], placeholders }
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
        return Edit::new(
            vec![Change { range: lines.start..lines.start + prefix_len, text: String::new() }],
            lines.start..lines.start + content.len(),
        );
    }
    let marker = match syntax {
        Syntax::Markdown => "#",
        Syntax::Typst => "=",
    };
    let prefix = format!("{} ", marker.repeat(level as usize));
    let content = if content.is_empty() { "Überschrift" } else { content };
    let new_line = format!("{prefix}{content}");
    let start = lines.start + prefix.len();
    Edit::new(vec![Change { range: lines, text: new_line }], start..start + content.len())
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
    Edit::new(vec![Change { range: lines, text: new_block }], selection)
}

/// Fügt einen Block als eigenen Absatz ein (Leerzeilen davor/danach nach Bedarf). `parts`
/// wechselt zwischen festem Text und Platzhaltern (ungerade Indizes); ohne Platzhalter steht
/// der Cursor danach am Blockende.
fn insert_block(text: &str, selection: Range<usize>, parts: &[&str]) -> Edit {
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
    let mut offset = range.start + lead.len();
    let mut placeholders = Vec::new();
    for (ix, part) in parts.iter().enumerate() {
        if ix % 2 == 1 {
            placeholders.push(offset..offset + part.len());
        }
        offset += part.len();
    }
    if placeholders.is_empty() {
        placeholders.push(offset..offset);
    }
    Edit { changes: vec![Change { range, text: format!("{lead}{}{trail}", parts.concat()) }], placeholders }
}

/// Wie `insert_block`, aber die Auswahl (oder der Platzhalter) steht zwischen `pre` und `post`.
fn block_wrap(text: &str, selection: Range<usize>, pre: &str, post: &str, placeholder: &str) -> Edit {
    let content = selected_or(text, &selection, placeholder);
    insert_block(text, selection, &[pre, &content, post])
}

fn footnote(text: &str, selection: Range<usize>, syntax: Syntax) -> Edit {
    match syntax {
        Syntax::Typst => {
            let body = selected_or(text, &selection, "Fußnotentext");
            let replacement = format!("#footnote[{body}]");
            let start = selection.start + "#footnote[".len();
            Edit::new(vec![Change { range: selection, text: replacement }], start..start + body.len())
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
            Edit::new(
                vec![
                    Change { range: selection, text: reference },
                    Change { range: text.len()..text.len(), text: definition },
                ],
                body_start..body_start + body.len(),
            )
        }
    }
}

/// Platzhalter einer eingefügten Vorlage, die per Tab nacheinander angesprungen werden
/// (wie Snippets in VS Code/Zettlr). Merkt sich den Text, um Eingaben nachzuverfolgen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabstops {
    ranges: Vec<Range<usize>>,
    current: usize,
    text: String,
}

impl Tabstops {
    /// Nur sinnvoll ab zwei Platzhaltern; `text` ist der Text nach dem Einfügen.
    pub fn new(placeholders: Vec<Range<usize>>, text: String) -> Option<Self> {
        (placeholders.len() > 1).then_some(Self { ranges: placeholders, current: 0, text })
    }

    pub fn current(&self) -> Range<usize> {
        self.ranges[self.current].clone()
    }

    /// Nächster Platzhalter; `None` beim letzten (Modus endet).
    pub fn next_range(&mut self) -> Option<Range<usize>> {
        if self.current + 1 == self.ranges.len() {
            return None;
        }
        self.current += 1;
        Some(self.current())
    }

    /// Vorheriger Platzhalter (bleibt beim ersten stehen).
    pub fn prev_range(&mut self) -> Range<usize> {
        self.current = self.current.saturating_sub(1);
        self.current()
    }

    /// Liegt `offset` innerhalb der eingefügten Vorlage (erster bis letzter Platzhalter)?
    pub fn contains(&self, offset: usize) -> bool {
        let first = self.ranges.first().map_or(0, |r| r.start);
        let last = self.ranges.last().map_or(0, |r| r.end);
        (first..=last).contains(&offset)
    }

    /// Gleicht mit dem neuen Text ab (`cursor` = Cursor danach). Änderungen im aktuellen
    /// Platzhalter verschieben die folgenden; `false` bei Änderungen außerhalb (Modus endet).
    pub fn sync(&mut self, text: &str, cursor: usize) -> bool {
        let (old, new_len) = text_change(&self.text, text, cursor);
        self.text = text.to_string();
        if old.is_empty() && new_len == 0 {
            return true;
        }
        let current = self.current();
        if old.start < current.start || old.end > current.end {
            return false;
        }
        let delta = new_len as isize - old.len() as isize;
        let shift = |i: usize| i.checked_add_signed(delta).unwrap_or(0);
        self.ranges[self.current].end = shift(current.end);
        for range in &mut self.ranges[self.current + 1..] {
            *range = shift(range.start)..shift(range.end);
        }
        true
    }
}

/// Ermittelt die Änderung von `old` zu `new` als (ersetzter Bereich in `old`, Länge des neuen
/// Texts). Bei Mehrdeutigkeit (z. B. gleiches Zeichen hinter dem Cursor) endet die Änderung
/// am `cursor` – dort, wo getippt wurde.
fn text_change(old: &str, new: &str, cursor: usize) -> (Range<usize>, usize) {
    let max_suffix = new.len().saturating_sub(cursor).min(old.len());
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    while !new.is_char_boundary(new.len() - suffix) || !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    let max_prefix = (old.len() - suffix).min(new.len() - suffix);
    let mut prefix = old.bytes().zip(new.bytes()).take(max_prefix).take_while(|(a, b)| a == b).count();
    while !new.is_char_boundary(prefix) || !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    (prefix..old.len() - suffix, new.len() - suffix - prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(action: Action, syntax: Syntax, text: &str, sel: Range<usize>) -> (String, String) {
        let edit = apply(action, syntax, text, sel);
        let out = edit.apply_to(text);
        let selected = out[edit.selection()].to_string();
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

    fn texts(out: &str, edit: &Edit) -> Vec<String> {
        edit.placeholders.iter().map(|r| out[r.clone()].to_string()).collect()
    }

    #[test]
    fn placeholders_in_tab_order() {
        let edit = apply(Action::Table, Syntax::Markdown, "", 0..0);
        assert_eq!(texts(&edit.apply_to(""), &edit), ["Spalte 1", "Spalte 2", "Wert", "Wert"]);
        let edit = apply(Action::Table, Syntax::Typst, "", 0..0);
        assert_eq!(texts(&edit.apply_to(""), &edit), ["Spalte 1", "Spalte 2", "Wert", "Wert"]);
        let edit = apply(Action::Image, Syntax::Markdown, "Text", 4..4);
        assert_eq!(texts(&edit.apply_to("Text"), &edit), ["bild.png", "Beschreibung"]);
        let edit = apply(Action::Image, Syntax::Typst, "", 0..0);
        assert_eq!(texts(&edit.apply_to(""), &edit), ["bild.png", "Beschreibung"]);
        for syntax in [Syntax::Markdown, Syntax::Typst] {
            let edit = apply(Action::Link, syntax, "", 0..0);
            assert_eq!(texts(&edit.apply_to(""), &edit), ["Linktext", "https://"]);
        }
        let edit = apply(Action::CodeBlock, Syntax::Markdown, "", 0..0);
        assert_eq!(texts(&edit.apply_to(""), &edit), ["text", "Code"]);
    }

    #[test]
    fn tabstops_shift_after_typing_in_first_placeholder() {
        let edit = apply(Action::Table, Syntax::Markdown, "", 0..0);
        let text = edit.apply_to("");
        let mut stops = Tabstops::new(edit.placeholders.clone(), text.clone()).unwrap();
        // „Spalte 1“ durch „Name“ ersetzen: erst ein Zeichen, dann weitere.
        let first = stops.current();
        let mut typed = text.clone();
        typed.replace_range(first.clone(), "N");
        assert!(stops.sync(&typed, first.start + 1));
        typed.insert_str(first.start + 1, "ame");
        assert!(stops.sync(&typed, first.start + 4));
        assert_eq!(&typed[stops.current()], "Name");
        let second = stops.next_range().unwrap();
        assert_eq!(&typed[second], "Spalte 2");
        assert_eq!(&typed[stops.next_range().unwrap()], "Wert");
        assert_eq!(&typed[stops.prev_range()], "Spalte 2");
        stops.next_range();
        assert_eq!(&typed[stops.next_range().unwrap()], "Wert");
        assert_eq!(stops.next_range(), None);
    }

    #[test]
    fn tabstops_handle_ambiguous_and_outside_edits() {
        // Typst-Bild: Pfad leeren und „\"“ tippen – gleiches Zeichen wie dahinter.
        let edit = apply(Action::Image, Syntax::Typst, "", 0..0);
        let text = edit.apply_to("");
        let mut stops = Tabstops::new(edit.placeholders.clone(), text.clone()).unwrap();
        let path = stops.current();
        let mut typed = text.clone();
        typed.replace_range(path.clone(), "");
        assert!(stops.sync(&typed, path.start));
        typed.insert(path.start, '"');
        assert!(stops.sync(&typed, path.start + 1));
        assert_eq!(&typed[stops.current()], "\"");
        assert_eq!(&typed[stops.next_range().unwrap()], "Beschreibung");
        // Änderung außerhalb des aktuellen Platzhalters beendet den Modus.
        typed.insert_str(0, "äö");
        assert!(!stops.sync(&typed, 2));
        // Ein einzelner Platzhalter braucht keinen Tab-Modus.
        assert!(Tabstops::new(vec![Range { start: 0, end: 1 }], "x".into()).is_none());
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
                        for range in &edit.placeholders {
                            assert!(out.get(range.clone()).is_some(), "{action:?} {syntax:?} {text:?}");
                        }
                    }
                }
            }
        }
    }
}
