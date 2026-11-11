//! Markdown/MDX → Typst.
//!
//! Ziel ist ein lesbares, idiomatisches Typst-Dokument, mit dem man direkt in Typst
//! weiterarbeiten kann – kein Zwischenformat. Grundlage ist `pulldown-cmark` (CommonMark +
//! GFM: Tabellen, Fußnoten, Durchstreichen, Aufgabenlisten, Hinweisblöcke, Mathe,
//! Definitionslisten). MDX wird vorverarbeitet: `import`/`export` und JS-Ausdrücke fallen
//! weg, bekannte Komponenten (Callout, Note, Tip, Warning, details …) werden abgebildet,
//! unbekannte durch ihren Inhalt ersetzt. Alles, was verloren geht, landet in `warnings`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, LinkType, Parser, Tag, TagEnd,
};

use crate::Warning;
use crate::frontmatter::Metadata;
use crate::math;

#[derive(Debug, Clone)]
pub struct Options {
    /// Sprache für Silbentrennung, Anführungszeichen und Beschriftungen (überschreibbar per Frontmatter).
    pub lang: String,
    /// Papierformat (Typst-Name, z. B. `a4`, `us-letter`).
    pub paper: String,
    /// MDX-Vorverarbeitung aktivieren.
    pub mdx: bool,
    /// Vollständiges Dokument mit Seiten-/Textvorgaben und Titelblock erzeugen.
    pub standalone: bool,
    /// Ordner der Quelldatei – für Bildpfade und Existenzprüfung.
    pub base_dir: Option<PathBuf>,
    /// Ordner der Zieldatei – Bildpfade werden relativ dazu geschrieben.
    pub output_dir: Option<PathBuf>,
    /// Name der Quelldatei für den Kopfkommentar.
    pub source_name: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lang: "de".into(),
            paper: "a4".into(),
            mdx: false,
            standalone: true,
            base_dir: None,
            output_dir: None,
            source_name: None,
        }
    }
}

impl Options {
    /// Vorgaben für eine Datei: MDX nach Endung, Bildpfade relativ zur Datei.
    pub fn for_file(path: &Path) -> Self {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let path = path.as_path();
        Self {
            mdx: path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx")),
            base_dir: path.parent().map(Path::to_path_buf),
            output_dir: path.parent().map(Path::to_path_buf),
            source_name: path.file_name().map(|n| n.to_string_lossy().to_string()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conversion {
    pub typst: String,
    pub metadata: Metadata,
    pub warnings: Vec<Warning>,
}

pub fn markdown_to_typst(source: &str, options: &Options) -> Conversion {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let (metadata, body) = split_frontmatter(source);
    let frontmatter_lines = source[..source.len() - body.len()].matches('\n').count();
    let mut warnings = Vec::new();
    let (body, mdx_origins) = if options.mdx {
        preprocess_mdx(body, &mut warnings)
    } else {
        (body.to_string(), (0..=body.matches('\n').count()).collect())
    };
    let (body, directive_origins) = preprocess_directives(&body);
    // Zeile im vorverarbeiteten Text → Zeile (ab 1) in der Quelldatei.
    let line_origins = directive_origins
        .iter()
        .map(|&line| frontmatter_lines + mdx_origins.get(line).copied().unwrap_or(line) + 1)
        .collect();

    let lang = metadata
        .lang
        .as_deref()
        .and_then(|lang| lang.split(['-', '_']).next())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| options.lang.clone());

    let accent = metadata.accent.as_deref().and_then(|value| {
        let line = source[..source.len() - body.len()]
            .lines()
            .position(|l| l.trim_start().to_ascii_lowercase().starts_with("accent"))
            .map(|ix| ix + 1);
        let warn = |message: String| Warning { line, message };
        let Some(rgb) = parse_hex_color(value) else {
            warnings.push(warn(format!(
                "Akzentfarbe „{value}“ nicht erkannt (erwartet z. B. \"#0a7f6f\") – ignoriert."
            )));
            return None;
        };
        let contrast = contrast_on_white(rgb);
        if contrast < 4.5 {
            let ratio = format!("{contrast:.1}").replace('.', ",");
            warnings.push(warn(format!(
                "Akzentfarbe „{value}“ hat zu wenig Kontrast zu Papierweiß ({ratio}:1, nötig sind 4,5:1) – Überschriften und Links sind schwer lesbar."
            )));
        }
        Some(format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2))
    });

    let mut renderer = Renderer::new(options);
    renderer.warnings = warnings;
    renderer.line_origins = line_origins;
    renderer.render_markdown(&body);
    let body = renderer.finish_body();

    let mut out = String::new();
    if options.standalone {
        out.push_str(&renderer.preamble(&metadata, &lang, &body));
    }
    out.push_str(&body);
    if !out.ends_with('\n') {
        out.push('\n');
    }

    Conversion {
        typst: out,
        metadata,
        warnings: renderer.warnings,
    }
}

// ---------------------------------------------------------------------------
// Frontmatter und MDX-Vorverarbeitung

pub(crate) fn split_frontmatter(source: &str) -> (Metadata, &str) {
    for (fence, parse) in [
        ("---", Metadata::parse_yaml as fn(&str) -> Metadata),
        ("+++", Metadata::parse_toml),
    ] {
        let Some(rest) = source.strip_prefix(fence) else {
            continue;
        };
        let Some(rest) = rest
            .strip_prefix('\n')
            .or_else(|| rest.strip_prefix("\r\n"))
        else {
            continue;
        };
        let mut offset = 0;
        for line in rest.split_inclusive('\n') {
            let trimmed = line.trim_end();
            if trimmed == fence || (fence == "---" && trimmed == "...") {
                let meta = parse(&rest[..offset]);
                return (meta, &rest[offset + line.len()..]);
            }
            offset += line.len();
        }
    }
    (Metadata::default(), source)
}

/// Hängt `text` an `out` an und merkt für jede darin beginnende Zeile die Ursprungszeile
/// `origin` – so lassen sich Hinweise nach der Vorverarbeitung der Quellzeile zuordnen.
fn push_tracked(out: &mut String, origins: &mut Vec<usize>, text: &str, origin: usize) {
    for (ix, part) in text.split_inclusive('\n').enumerate() {
        if (ix > 0 || out.is_empty() || out.ends_with('\n')) && !part.is_empty() {
            origins.push(origin);
        }
        out.push_str(part);
    }
}

pub(crate) fn preprocess_mdx(body: &str, warnings: &mut Vec<Warning>) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(body.len());
    let mut origins = Vec::new();
    let mut fence: Option<String> = None;
    let mut math_block = false;
    let mut skipping_statement = 0i32; // Klammertiefe eines mehrzeiligen import/export
    let mut in_comment = false; // `{/* … */}`
    let mut expr_depth = 0i32; // mehrzeiliger `{…}`-Ausdruck, wird verworfen
    let mut in_tag = false; // JSX-Tag, der am Zeilenanfang begann und über Zeilen geht
    let mut removed_statements = 0;
    let mut removed_expressions = 0;

    for (line_no, line) in body.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str())
                && trimmed.trim_end().chars().all(|c| open.starts_with(c))
            {
                fence = None;
            }
            push_tracked(&mut out, &mut origins, line, line_no);
            continue;
        }
        if skipping_statement > 0 {
            skipping_statement += bracket_balance(line);
            continue;
        }
        if !in_tag && expr_depth == 0 && !in_comment {
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                let marker: String = trimmed
                    .chars()
                    .take_while(|&c| c == '`' || c == '~')
                    .collect();
                fence = Some(marker);
                push_tracked(&mut out, &mut origins, line, line_no);
                continue;
            }
            if trimmed.starts_with("$$") {
                // `$$ … $$` auf einer Zeile schaltet nicht um.
                let single_line =
                    trimmed.trim_end().len() > 2 && trimmed.trim_end().ends_with("$$");
                if !single_line {
                    math_block = !math_block;
                }
                push_tracked(&mut out, &mut origins, line, line_no);
                continue;
            }
            if math_block {
                push_tracked(&mut out, &mut origins, line, line_no);
                continue;
            }
            if !line.starts_with([' ', '\t'])
                && (line.starts_with("import ") || line.starts_with("export "))
            {
                removed_statements += 1;
                skipping_statement = bracket_balance(line).max(0);
                continue;
            }
        }

        let chars: Vec<char> = line.chars().collect();
        let mut cleaned = String::with_capacity(line.len());
        let mut i = 0;
        let mut code_run = 0usize;
        let mut math_run = 0usize;
        let tag_line = trimmed.starts_with('<');
        while i < chars.len() {
            let c = chars[i];
            if in_comment {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    let mut j = i + 2;
                    while chars.get(j).is_some_and(|c| *c == ' ' || *c == '\t') {
                        j += 1;
                    }
                    if chars.get(j) == Some(&'}') {
                        in_comment = false;
                        i = j + 1;
                        continue;
                    }
                }
                i += 1;
                continue;
            }
            if expr_depth > 0 {
                match c {
                    '{' => expr_depth += 1,
                    '}' => expr_depth -= 1,
                    _ => {}
                }
                i += 1;
                continue;
            }
            if c == '`' && math_run == 0 {
                let run = chars[i..].iter().take_while(|&&c| c == '`').count();
                if code_run == 0 {
                    code_run = run;
                } else if code_run == run {
                    code_run = 0;
                }
                cleaned.extend(&chars[i..i + run]);
                i += run;
                continue;
            }
            if c == '$' && code_run == 0 && !in_tag && !(i > 0 && chars[i - 1] == '\\') {
                // Inline-Mathe: `{…}` darin ist LaTeX, kein JS.
                let run = chars[i..].iter().take_while(|&&c| c == '$').count();
                if math_run == 0 {
                    math_run = run;
                } else if math_run == run {
                    math_run = 0;
                }
                cleaned.extend(&chars[i..i + run]);
                i += run;
                continue;
            }
            if code_run == 0 && math_run == 0 {
                if !in_tag
                    && c == '<'
                    && chars
                        .get(i + 1)
                        .is_some_and(|n| n.is_ascii_alphabetic() || *n == '/')
                {
                    in_tag = true;
                } else if in_tag && c == '>' {
                    in_tag = false;
                } else if c == '{' {
                    let rest: String = chars[i + 1..].iter().collect();
                    if !in_tag && rest.trim_start().starts_with("/*") {
                        in_comment = true;
                        i += 1 + rest.find("/*").unwrap() + 2;
                        continue;
                    }
                    match matching_brace(&chars[i..]) {
                        Some(len) => {
                            if in_tag {
                                // JSX-Attribut `prop={…}` → `prop="…"`, damit es als HTML
                                // erkannt wird; Spread `{...props}` entfällt.
                                if cleaned.ends_with('=') {
                                    let inner: String = chars[i + 1..i + len - 1].iter().collect();
                                    cleaned.push('"');
                                    cleaned.push_str(
                                        &inner.trim().replace('"', "'").replace('\n', " "),
                                    );
                                    cleaned.push('"');
                                }
                            } else {
                                removed_expressions += 1;
                            }
                            i += len;
                            continue;
                        }
                        // Mehrzeiliger Ausdruck: nur in Tags oder als eigener Block verwerfen,
                        // sonst würde eine einzelne `{` im Fließtext den Rest verschlucken.
                        None if in_tag || cleaned.trim().is_empty() => {
                            if in_tag && cleaned.ends_with('=') {
                                cleaned.push_str("\"\"");
                            }
                            if !in_tag {
                                removed_expressions += 1;
                            }
                            expr_depth = 1;
                            i += 1;
                            continue;
                        }
                        None => {}
                    }
                }
            }
            cleaned.push(c);
            i += 1;
        }
        // Mehrzeilige Tags auf eine Zeile ziehen, damit sie als HTML-Block erkannt werden.
        if (in_tag && tag_line) || expr_depth > 0 {
            if cleaned.ends_with('\n') {
                cleaned.pop();
                cleaned.push(' ');
            }
        } else if in_tag {
            // `<` im Fließtext ohne `>` in derselben Zeile: kein Tag.
            in_tag = false;
        }
        // Zeilen, die nur aus einem entfernten Ausdruck bestanden, nicht als Leerzeile stehen lassen.
        if cleaned.trim().is_empty() && !line.trim().is_empty() {
            continue;
        }
        push_tracked(&mut out, &mut origins, &cleaned, line_no);
    }

    if removed_statements > 0 {
        warnings.push(Warning::new(format!(
            "MDX: {removed_statements} import/export-Anweisung(en) entfernt."
        )));
    }
    if removed_expressions > 0 {
        warnings.push(Warning::new(format!(
            "MDX: {removed_expressions} JavaScript-Ausdruck/Ausdrücke {{…}} entfernt."
        )));
    }
    (out, origins)
}

/// Container-Direktiven (Docusaurus, VitePress, Starlight): `:::tip Titel` … `:::`
/// → `<Callout>`, das der Renderer als Hinweisbox setzt.
pub(crate) fn preprocess_directives(body: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(body.len());
    let mut origins = Vec::new();
    let mut fence: Option<String> = None;
    let mut open = 0usize;
    for (line_no, line) in body.split_inclusive('\n').enumerate() {
        let trimmed = line.trim();
        if let Some(marker) = &fence {
            if trimmed.starts_with(marker.as_str())
                && trimmed.chars().all(|c| marker.starts_with(c))
            {
                fence = None;
            }
            push_tracked(&mut out, &mut origins, line, line_no);
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(
                trimmed
                    .chars()
                    .take_while(|&c| c == '`' || c == '~')
                    .collect(),
            );
            push_tracked(&mut out, &mut origins, line, line_no);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(":::") {
            let rest = rest.trim_start_matches(':').trim();
            if rest.is_empty() {
                if open > 0 {
                    open -= 1;
                    push_tracked(&mut out, &mut origins, "\n</Callout>\n\n", line_no);
                    continue;
                }
            } else {
                let kind: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect();
                if !kind.is_empty() {
                    let title = rest[kind.len()..]
                        .trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .trim_start_matches("{title=")
                        .trim_end_matches('}')
                        .trim_matches(['"', '\''])
                        .trim()
                        .replace('"', "'");
                    open += 1;
                    let title = if title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{title}\"")
                    };
                    push_tracked(
                        &mut out,
                        &mut origins,
                        &format!("\n<Callout type=\"{kind}\"{title}>\n\n"),
                        line_no,
                    );
                    continue;
                }
            }
        }
        push_tracked(&mut out, &mut origins, line, line_no);
    }
    let last = origins.last().copied().unwrap_or(0);
    for _ in 0..open {
        push_tracked(&mut out, &mut origins, "\n</Callout>\n", last);
    }
    (out, origins)
}

fn bracket_balance(line: &str) -> i32 {
    line.chars()
        .map(|c| match c {
            '{' | '(' | '[' => 1,
            '}' | ')' | ']' => -1,
            _ => 0,
        })
        .sum()
}

/// Länge eines ausgeglichenen `{…}`-Ausdrucks ab `chars[0] == '{'`, sofern er in der Zeile endet.
fn matching_brace(chars: &[char]) -> Option<usize> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for (i, &c) in chars.iter().enumerate() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            '\n' => return None,
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Renderer

pub(crate) fn parser_options() -> pulldown_cmark::Options {
    use pulldown_cmark::Options as O;
    O::ENABLE_TABLES
        | O::ENABLE_FOOTNOTES
        | O::ENABLE_STRIKETHROUGH
        | O::ENABLE_TASKLISTS
        | O::ENABLE_HEADING_ATTRIBUTES
        | O::ENABLE_MATH
        | O::ENABLE_GFM
        | O::ENABLE_DEFINITION_LIST
}

enum Frame {
    Root,
    Item,
    List {
        start: Option<u64>,
        items: Vec<String>,
        loose: bool,
    },
    BlockQuote(Option<BlockQuoteKind>),
    Footnote(String),
    Code {
        lang: String,
    },
    HtmlBlock,
    Table(TableState),
    Cell,
    Image {
        dest: String,
        title: String,
    },
    Term,
    Definition,
}

#[derive(Default)]
struct TableState {
    alignments: Vec<Alignment>,
    header: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    row: Vec<String>,
}

struct OpenHtml {
    name: String,
    suffix: String,
    depth: usize,
}

struct Renderer<'a> {
    options: &'a Options,
    frames: Vec<(Frame, String)>,
    warnings: Vec<Warning>,
    /// Zeile im vorverarbeiteten Text → Quellzeile (ab 1).
    line_origins: Vec<usize>,
    /// Quellzeile des gerade verarbeiteten Blocks; `None` nach dem Durchlauf.
    line: Option<usize>,
    warned: HashSet<String>,
    footnotes: HashMap<String, String>,
    slugs: HashMap<String, usize>,
    labels: HashSet<String>,
    internal_links: Vec<String>,
    math_helpers: BTreeSet<&'static str>,
    needs: BTreeSet<&'static str>,
    html: Vec<OpenHtml>,
    /// Direkt davor endete ein eingebetteter Ausdruck (`#f[…]`); folgender Text muss das beachten.
    after_expr: bool,
    first_h1: Option<String>,
    heading_text: Option<String>,
    has_table: bool,
    /// Inhalt eines Autolinks unterdrücken (`#link("url")` zeigt ihn selbst).
    skip_link_text: bool,
    /// Verschachtelungstiefe von Links (Bild-Badges in Links).
    link_depth: usize,
}

const FOOTNOTE_START: char = '\u{E000}';
const FOOTNOTE_END: char = '\u{E001}';

impl<'a> Renderer<'a> {
    fn new(options: &'a Options) -> Self {
        Self {
            options,
            frames: vec![(Frame::Root, String::new())],
            warnings: Vec::new(),
            line_origins: Vec::new(),
            line: None,
            warned: HashSet::new(),
            footnotes: HashMap::new(),
            slugs: HashMap::new(),
            labels: HashSet::new(),
            internal_links: Vec::new(),
            math_helpers: BTreeSet::new(),
            needs: BTreeSet::new(),
            html: Vec::new(),
            after_expr: false,
            first_h1: None,
            heading_text: None,
            has_table: false,
            skip_link_text: false,
            link_depth: 0,
        }
    }

    fn warn(&mut self, message: String) {
        self.warnings.push(Warning {
            line: self.line,
            message,
        });
    }

    fn warn_once(&mut self, key: &str, message: String) {
        if self.warned.insert(key.to_string()) {
            self.warn(message);
        }
    }

    fn buf(&mut self) -> &mut String {
        &mut self.frames.last_mut().expect("root frame").1
    }

    fn write(&mut self, s: &str) {
        self.after_expr = false;
        self.buf().push_str(s);
    }

    /// Schreibt das Ende eines eingebetteten Ausdrucks (`]` oder `)`).
    fn close_expr(&mut self, s: &str) {
        self.buf().push_str(s);
        self.after_expr = true;
    }

    fn at_line_start(&mut self) -> bool {
        let buf = self.buf();
        buf.is_empty() || buf.ends_with('\n')
    }

    /// Stellt sicher, dass ein neuer Block auf einer eigenen Zeile nach einer Leerzeile beginnt.
    fn block_break(&mut self) {
        let buf = self.buf();
        if buf.is_empty() || buf.ends_with("\n\n") || buf.ends_with("[\n") {
        } else if buf.ends_with('\n') {
            buf.push('\n');
        } else {
            buf.push_str("\n\n");
        }
        self.after_expr = false;
    }

    fn line_break(&mut self) {
        if !self.at_line_start() {
            self.buf().push('\n');
        }
        self.after_expr = false;
    }

    fn text(&mut self, text: &str) {
        if let Some(heading) = &mut self.heading_text {
            heading.push_str(text);
        }
        let line_start = self.at_line_start();
        let after_expr = self.after_expr;
        // `1` und `. Text` können als getrennte Text-Events kommen.
        let current_line = self
            .buf()
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .trim_start();
        let digit_prefix =
            !current_line.is_empty() && current_line.chars().all(|c| c.is_ascii_digit());
        let mut escaped = escape_markup(text, line_start, after_expr);
        if digit_prefix && text.starts_with('.') {
            escaped.insert(0, '\\');
        }
        self.write(&escaped);
    }

    fn in_code(&self) -> bool {
        matches!(
            self.frames.last(),
            Some((Frame::Code { .. } | Frame::HtmlBlock, _))
        )
    }

    fn render_markdown(&mut self, markdown: &str) {
        let (events, offsets): (Vec<Event>, Vec<usize>) =
            Parser::new_ext(markdown, parser_options())
                .into_offset_iter()
                .map(|(event, range)| (event, range.start))
                .unzip();
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(markdown.match_indices('\n').map(|(ix, _)| ix + 1))
            .collect();
        let lines: Vec<usize> = offsets
            .iter()
            .map(|&offset| {
                let line = line_starts.partition_point(|&start| start <= offset) - 1;
                self.line_origins.get(line).copied().unwrap_or(line + 1)
            })
            .collect();
        self.render_events(&events, Some(&lines));
        self.line = None;
    }

    /// `lines`: Quellzeile je Ereignis (nur auf oberster Ebene bekannt).
    fn render_events(&mut self, events: &[Event], lines: Option<&[usize]>) {
        let mut i = 0;
        while i < events.len() {
            if let Some(lines) = lines {
                self.line = Some(lines[i]);
            }
            if let Some(skip) = self.try_figure(&events[i..]) {
                i += skip;
                continue;
            }
            self.event(&events[i]);
            i += 1;
        }
    }

    /// Absatz, der nur aus einem Bild besteht → `figure`.
    fn try_figure(&mut self, events: &[Event]) -> Option<usize> {
        let [
            Event::Start(Tag::Paragraph),
            Event::Start(Tag::Image {
                dest_url, title, ..
            }),
            rest @ ..,
        ] = events
        else {
            return None;
        };
        let end = rest
            .iter()
            .position(|e| matches!(e, Event::End(TagEnd::Image)))?;
        if !matches!(rest.get(end + 1), Some(Event::End(TagEnd::Paragraph))) {
            return None;
        }
        let alt = plain_text(&rest[..end]);
        let caption = if title.is_empty() {
            alt.clone()
        } else {
            title.to_string()
        };
        self.block_break();
        let image = self.image_call(dest_url, &alt, None);
        match image {
            ImageCall::Image(call) if !caption.trim().is_empty() => {
                let caption = escape_markup(&caption, false, false);
                self.write(&format!("#figure(\n  {call},\n  caption: [{caption}],\n)"));
            }
            ImageCall::Image(call) => self.write(&format!("#align(center, {call})")),
            ImageCall::Placeholder(text) => self.write(&text),
        }
        self.write("\n");
        Some(2 + end + 2)
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(*tag),
            Event::Text(_) if self.skip_link_text => {}
            Event::Text(text) => {
                if self.in_code() {
                    self.buf().push_str(text);
                } else if let Some((Frame::Image { .. }, _)) = self.frames.last() {
                    self.buf().push_str(text);
                } else {
                    self.text(text);
                }
            }
            Event::Code(code) => {
                if let Some(heading) = &mut self.heading_text {
                    heading.push_str(code);
                }
                self.inline_code(code);
            }
            Event::InlineMath(latex) => self.math(latex, false),
            Event::DisplayMath(latex) => self.math(latex, true),
            Event::Html(html) => {
                if let Some((Frame::HtmlBlock, buf)) = self.frames.last_mut() {
                    buf.push_str(html);
                } else {
                    self.html(html, true);
                }
            }
            Event::InlineHtml(html) => self.html(html, false),
            Event::FootnoteReference(label) => {
                self.write(&format!("{FOOTNOTE_START}{label}{FOOTNOTE_END}"));
                self.after_expr = true;
            }
            Event::SoftBreak => {
                if let Some(heading) = &mut self.heading_text {
                    heading.push(' ');
                }
                self.write("\n");
            }
            Event::HardBreak => self.write(" \\\n"),
            Event::Rule => {
                self.block_break();
                self.write("#line(length: 100%, stroke: 0.5pt + luma(180))\n");
            }
            Event::TaskListMarker(done) => {
                self.needs.insert("task");
                self.write(&format!("#task({done}) "));
            }
        }
    }

    fn start(&mut self, tag: &Tag) {
        match tag {
            Tag::Paragraph => {
                self.mark_loose();
                self.block_break();
            }
            Tag::Heading { level, id, .. } => {
                self.block_break();
                let marks = "=".repeat(heading_level(*level));
                self.write(&format!("{marks} "));
                self.heading_text = Some(String::new());
                // Explizite ID merken wir uns über den Text-Puffer nicht; sie geht als Label raus.
                if let Some(id) = id {
                    self.heading_text = Some(format!("{FOOTNOTE_START}id:{id}{FOOTNOTE_END}"));
                }
            }
            Tag::BlockQuote(kind) => {
                self.block_break();
                self.frames.push((Frame::BlockQuote(*kind), String::new()));
            }
            Tag::CodeBlock(kind) => {
                self.block_break();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => code_lang(info),
                    CodeBlockKind::Indented => String::new(),
                };
                self.frames.push((Frame::Code { lang }, String::new()));
            }
            Tag::HtmlBlock => self.frames.push((Frame::HtmlBlock, String::new())),
            Tag::List(start) => {
                // Unterliste direkt unter dem Text eines engen Listenpunkts.
                if matches!(self.frames.last(), Some((Frame::Item, _))) {
                    self.line_break();
                } else {
                    self.block_break();
                }
                self.frames.push((
                    Frame::List {
                        start: *start,
                        items: Vec::new(),
                        loose: false,
                    },
                    String::new(),
                ));
            }
            Tag::Item => self.frames.push((Frame::Item, String::new())),
            Tag::FootnoteDefinition(label) => self
                .frames
                .push((Frame::Footnote(label.to_string()), String::new())),
            Tag::DefinitionList => self.block_break(),
            Tag::DefinitionListTitle => self.frames.push((Frame::Term, String::new())),
            Tag::DefinitionListDefinition => self.frames.push((Frame::Definition, String::new())),
            Tag::Table(alignments) => {
                self.block_break();
                self.has_table = true;
                self.frames.push((
                    Frame::Table(TableState {
                        alignments: alignments.clone(),
                        ..Default::default()
                    }),
                    String::new(),
                ));
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some((Frame::Table(table), _)) = self.frames.last_mut() {
                    table.row.clear();
                }
            }
            Tag::TableCell => self.frames.push((Frame::Cell, String::new())),
            Tag::Emphasis => self.write("#emph["),
            Tag::Strong => self.write("#strong["),
            Tag::Strikethrough => self.write("#strike["),
            Tag::Superscript => self.write("#super["),
            Tag::Subscript => self.write("#sub["),
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => self.start_link(*link_type, dest_url),
            Tag::Image {
                dest_url, title, ..
            } => self.frames.push((
                Frame::Image {
                    dest: dest_url.to_string(),
                    title: title.to_string(),
                },
                String::new(),
            )),
            Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.line_break(),
            TagEnd::Heading(level) => {
                let text = self.heading_text.take().unwrap_or_default();
                let label = match text
                    .strip_prefix(FOOTNOTE_START)
                    .and_then(|s| s.strip_prefix("id:"))
                    .and_then(|s| s.split_once(FOOTNOTE_END))
                {
                    Some((id, _)) => self.unique_label(&slugify(id)),
                    None => self.unique_label(&slugify(&text)),
                };
                if level == HeadingLevel::H1 && self.first_h1.is_none() {
                    self.first_h1 = Some(text.trim().to_string());
                }
                self.write(&format!(" <{label}>\n"));
            }
            TagEnd::BlockQuote(_) => {
                let (frame, body) = self.pop_frame();
                let Frame::BlockQuote(kind) = frame else {
                    return;
                };
                let body = body.trim();
                match kind {
                    None => self.write(&format!("#quote(block: true)[\n{body}\n]")),
                    Some(kind) => {
                        self.needs.insert("callout");
                        self.write(&format!(
                            "#callout(kind: \"{}\")[\n{body}\n]",
                            callout_kind(kind)
                        ));
                    }
                }
                self.after_expr = true;
                self.line_break();
            }
            TagEnd::CodeBlock => {
                let (frame, code) = self.pop_frame();
                let Frame::Code { lang } = frame else { return };
                let fence = "`".repeat(longest_run(&code, '`').max(2) + 1);
                let code = code.strip_suffix('\n').unwrap_or(&code);
                self.write(&format!("{fence}{lang}\n{code}\n{fence}\n"));
            }
            TagEnd::HtmlBlock => {
                let (_, html) = self.pop_frame();
                self.html(&html, true);
            }
            TagEnd::List(_) => {
                let (frame, _) = self.pop_frame();
                let Frame::List {
                    start,
                    items,
                    loose,
                } = frame
                else {
                    return;
                };
                let separator = if loose { "\n\n" } else { "\n" };
                let mut lines = Vec::with_capacity(items.len());
                for (ix, item) in items.iter().enumerate() {
                    let marker = match start {
                        Some(start) => format!("{}. ", start + ix as u64),
                        None => "- ".to_string(),
                    };
                    lines.push(format!("{marker}{}", indent(item.trim(), 2)));
                }
                self.write(&lines.join(separator));
                self.write("\n");
            }
            TagEnd::Item => {
                let (_, body) = self.pop_frame();
                if let Some((Frame::List { items, .. }, _)) = self.frames.last_mut() {
                    items.push(body);
                }
            }
            TagEnd::FootnoteDefinition => {
                let (frame, body) = self.pop_frame();
                if let Frame::Footnote(label) = frame {
                    self.footnotes.insert(label, body.trim().to_string());
                }
            }
            TagEnd::DefinitionList => {}
            TagEnd::DefinitionListTitle => {
                let (_, term) = self.pop_frame();
                self.line_break();
                self.write(&format!("/ {}: ", term.trim()));
            }
            TagEnd::DefinitionListDefinition => {
                let (_, body) = self.pop_frame();
                self.write(&indent(body.trim(), 2));
                self.write("\n");
            }
            TagEnd::Table => {
                let (frame, _) = self.pop_frame();
                if let Frame::Table(table) = frame {
                    let table = render_table(table);
                    self.write(&table);
                    self.after_expr = true;
                    self.line_break();
                }
            }
            TagEnd::TableHead => {
                if let Some((Frame::Table(table), _)) = self.frames.last_mut() {
                    table.header = Some(std::mem::take(&mut table.row));
                }
            }
            TagEnd::TableRow => {
                if let Some((Frame::Table(table), _)) = self.frames.last_mut() {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                let (_, cell) = self.pop_frame();
                if let Some((Frame::Table(table), _)) = self.frames.last_mut() {
                    table.row.push(cell.trim().to_string());
                }
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript => self.close_expr("]"),
            TagEnd::Link => {
                self.link_depth -= 1;
                if self.skip_link_text {
                    self.skip_link_text = false;
                    self.after_expr = true;
                } else {
                    self.close_expr("]");
                }
            }
            TagEnd::Image => {
                let (frame, alt) = self.pop_frame();
                let Frame::Image { dest, title } = frame else {
                    return;
                };
                let alt = if alt.is_empty() { title } else { alt };
                match self.image_call(&dest, &alt, Some("1.2em")) {
                    ImageCall::Image(call) => {
                        self.write(&format!("#box({call}"));
                        self.close_expr(")");
                    }
                    ImageCall::Placeholder(text) => self.write(&text),
                }
            }
            TagEnd::MetadataBlock(_) => {}
        }
    }

    fn pop_frame(&mut self) -> (Frame, String) {
        // Offene HTML-Wrapper dieses Rahmens schließen, sonst stimmen die Klammern nicht.
        let depth = self.frames.len();
        while self.html.last().is_some_and(|open| open.depth >= depth) {
            let open = self.html.pop().unwrap();
            self.close_expr(&open.suffix);
        }
        let frame = self.frames.pop().expect("frame");
        self.after_expr = false;
        frame
    }

    fn mark_loose(&mut self) {
        // Absätze direkt in einem Listenpunkt → lockere Liste (Leerzeile zwischen Punkten).
        let n = self.frames.len();
        if n >= 2
            && let (Frame::Item, _) = &self.frames[n - 1]
            && let (Frame::List { loose, .. }, _) = &mut self.frames[n - 2]
        {
            *loose = true;
        }
    }

    fn unique_label(&mut self, slug: &str) -> String {
        let base = if slug.is_empty() {
            "abschnitt".to_string()
        } else {
            slug.to_string()
        };
        let count = self.slugs.entry(base.clone()).or_insert(0);
        let label = if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        self.labels.insert(label.clone());
        label
    }

    fn inline_code(&mut self, code: &str) {
        if !code.contains('`') && !code.is_empty() {
            self.write(&format!("`{code}`"));
            self.after_expr = false;
        } else {
            self.write(&format!("#raw(\"{}\"", escape_string(code)));
            self.close_expr(")");
        }
    }

    fn math(&mut self, latex: &str, display: bool) {
        match math::latex_to_typst(latex.trim()) {
            Ok(typst) if !typst.trim().is_empty() => {
                math::used_helpers(&typst, &mut self.math_helpers);
                if display {
                    self.write(&format!("$ {typst} $"));
                } else {
                    self.write(&format!("${typst}$"));
                }
            }
            Ok(_) => {}
            Err(err) => {
                self.warn(format!(
                    "Formel nicht umwandelbar, als LaTeX-Code übernommen: {} ({})",
                    latex.trim(),
                    err.lines().next().unwrap_or_default()
                ));
                let block = if display { ", block: true" } else { "" };
                self.write(&format!(
                    "#raw(\"{}\", lang: \"latex\"{block}",
                    escape_string(latex.trim())
                ));
                self.close_expr(")");
            }
        }
    }

    fn start_link(&mut self, link_type: LinkType, dest: &str) {
        self.link_depth += 1;
        let dest = dest.trim();
        if dest.is_empty() {
            self.write("#[");
            return;
        }
        if let Some(target) = dest.strip_prefix('#') {
            let label = slugify(&percent_decode(target));
            self.internal_links.push(label.clone());
            self.write(&format!("#link(<{label}>)["));
            return;
        }
        let url = match link_type {
            LinkType::Email if !dest.starts_with("mailto:") => format!("mailto:{dest}"),
            _ => dest.to_string(),
        };
        if link_type == LinkType::Autolink {
            // Text = URL: `#link("…")` zeigt die URL selbst an.
            self.skip_link_text = true;
            self.write(&format!("#link(\"{}\")", escape_string(&url)));
            return;
        }
        self.write(&format!("#link(\"{}\")[", escape_string(&url)));
    }

    fn image_call(&mut self, dest: &str, alt: &str, height: Option<&str>) -> ImageCall {
        let label = if alt.trim().is_empty() {
            "Bild"
        } else {
            alt.trim()
        };
        if dest.starts_with("http://") || dest.starts_with("https://") || dest.starts_with("data:")
        {
            self.warn_once(
                &format!("remote:{dest}"),
                format!("Entferntes Bild nicht eingebettet (offline): {dest}"),
            );
            if self.link_depth > 0 {
                // z. B. Badge in einem Link: nur der Alternativtext.
                return ImageCall::Placeholder(escape_markup(label, false, false));
            }
            return ImageCall::Placeholder(format!(
                "#link(\"{}\")[{}]",
                escape_string(dest),
                escape_markup(label, false, false)
            ));
        }
        let decoded = percent_decode(dest);
        let ext = Path::new(&decoded)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "pdf"
        ) {
            self.warn_once(
                &format!("format:{dest}"),
                format!("Bildformat nicht unterstützt: {dest}"),
            );
            return ImageCall::Placeholder(format!("\\[{}\\]", escape_markup(label, false, false)));
        }
        let mut path = decoded.clone();
        if let Some(base) = &self.options.base_dir {
            let absolute = base.join(decoded.trim_start_matches('/'));
            if !absolute.exists() {
                self.warn_once(
                    &format!("missing:{dest}"),
                    format!("Bild nicht gefunden: {dest}"),
                );
                return ImageCall::Placeholder(format!(
                    "\\[Bild fehlt: {}\\]",
                    escape_markup(&decoded, false, false)
                ));
            }
            if let Some(output) = &self.options.output_dir
                && let Some(relative) = relative_path(&absolute, output)
            {
                path = relative.to_string_lossy().replace('\\', "/");
            }
        }
        let mut args = format!("image(\"{}\"", escape_string(&path));
        if let Some(height) = height {
            args.push_str(&format!(", height: {height}"));
        }
        if !alt.trim().is_empty() {
            args.push_str(&format!(", alt: \"{}\"", escape_string(alt.trim())));
        }
        args.push(')');
        ImageCall::Image(args)
    }

    // ---- HTML / JSX ----

    fn html(&mut self, html: &str, block: bool) {
        for token in tokenize_html(html) {
            match token {
                HtmlToken::Comment(text) => {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    if block {
                        self.block_break();
                        for line in text.lines() {
                            self.write(&format!("// {}\n", line.trim()));
                        }
                    } else {
                        self.write(&format!("/* {} */", text.replace("*/", "* /")));
                    }
                }
                HtmlToken::Text(text) => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    if block {
                        self.render_fragment(&text);
                    } else {
                        self.text(&decode_entities(&text));
                    }
                }
                HtmlToken::Open {
                    name,
                    attrs,
                    self_closing,
                } => self.html_open(&name, &attrs, self_closing || is_void(&name), block),
                HtmlToken::Close { name } => self.html_close(&name),
            }
        }
    }

    /// Markdown innerhalb eines HTML-/JSX-Blocks (MDX erlaubt das).
    fn render_fragment(&mut self, markdown: &str) {
        let events: Vec<Event> = Parser::new_ext(markdown.trim(), parser_options())
            .map(Event::into_static)
            .collect();
        // Ein einzelner Absatz ohne Umbruch bleibt inline (z. B. `<summary>Titel</summary>`).
        let inline = matches!(
            events.as_slice(),
            [
                Event::Start(Tag::Paragraph),
                ..,
                Event::End(TagEnd::Paragraph)
            ]
        ) && events
            .iter()
            .filter(|e| matches!(e, Event::Start(Tag::Paragraph)))
            .count()
            == 1
            && !self.at_line_start();
        if inline {
            self.render_events(&events[1..events.len() - 1], None);
        } else {
            self.render_events(&events, None);
        }
    }

    fn html_open(&mut self, name: &str, attrs: &[(String, String)], void: bool, block: bool) {
        let attr = |key: &str| {
            attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.clone())
        };
        let lower = name.to_ascii_lowercase();

        if let Some(kind) = admonition_kind(
            name,
            attr("type")
                .or(attr("variant"))
                .or(attr("kind"))
                .or(attr("status"))
                .as_deref(),
        ) {
            self.needs.insert("callout");
            self.block_break();
            let title = attr("title")
                .map(|t| format!(", title: [{}]", escape_markup(&t, false, false)))
                .unwrap_or_default();
            self.write(&format!("#callout(kind: \"{kind}\"{title})[\n"));
            if !void {
                self.push_html(name, "\n]\n");
            } else {
                self.close_expr("]\n");
            }
            return;
        }

        match lower.as_str() {
            "br" => self.write(" \\ "),
            "hr" => {
                self.block_break();
                self.write("#line(length: 100%, stroke: 0.5pt + luma(180))\n");
            }
            "img" => {
                let src = attr("src").unwrap_or_default();
                let alt = attr("alt").unwrap_or_default();
                let width = attr("width").and_then(|w| css_length(&w));
                match self.image_call(&src, &alt, None) {
                    ImageCall::Image(mut call) => {
                        if let Some(width) = width {
                            call.insert_str(call.len() - 1, &format!(", width: {width}"));
                        }
                        if block {
                            self.block_break();
                            self.write(&format!("#align(center, {call}"));
                            self.close_expr(")");
                            self.line_break();
                        } else {
                            self.write(&format!("#box({call}"));
                            self.close_expr(")");
                        }
                    }
                    ImageCall::Placeholder(text) => self.write(&text),
                }
            }
            "input" if attr("type").as_deref() == Some("checkbox") => {
                self.needs.insert("task");
                let done = attrs.iter().any(|(k, _)| k == "checked");
                self.write(&format!("#task({done}) "));
            }
            "details" => {
                self.needs.insert("callout");
                self.block_break();
                self.write("#callout(kind: \"details\")[\n");
                self.push_html(name, "\n]\n");
            }
            "summary" => {
                self.write("#strong[");
                self.push_html(name, "]\n\n");
            }
            "sup" => self.wrap_html(name, "#super[", "]"),
            "sub" => self.wrap_html(name, "#sub[", "]"),
            "kbd" => {
                self.needs.insert("kbd");
                self.wrap_html(name, "#kbd[", "]");
            }
            "mark" => self.wrap_html(name, "#highlight[", "]"),
            "u" | "ins" => self.wrap_html(name, "#underline[", "]"),
            "del" | "s" | "strike" => self.wrap_html(name, "#strike[", "]"),
            "b" | "strong" => self.wrap_html(name, "#strong[", "]"),
            "i" | "em" | "cite" | "dfn" | "var" => self.wrap_html(name, "#emph[", "]"),
            "small" => self.wrap_html(name, "#text(size: 0.85em)[", "]"),
            "code" | "samp" | "tt" => {
                self.wrap_html(name, "#text(font: \"DejaVu Sans Mono\", size: 0.9em)[", "]")
            }
            "center" => {
                self.block_break();
                self.wrap_html(name, "#align(center)[\n", "\n]\n");
            }
            "a" => match attr("href") {
                Some(href) => {
                    let prefix = format!("#link(\"{}\")[", escape_string(&href));
                    self.wrap_html(name, &prefix, "]");
                }
                None => self.push_html(name, ""),
            },
            "p" | "div" | "section" | "article" | "figure" | "blockquote" | "figcaption"
                if !void =>
            {
                self.block_break();
                self.push_html(name, "\n");
            }
            "tabitem" => {
                // Docusaurus/Starlight: <TabItem label="…"> → Zwischenüberschrift.
                self.block_break();
                if let Some(label) = attr("label").or(attr("value")) {
                    self.write(&format!("#strong[{}]", escape_markup(&label, false, false)));
                    self.close_expr("\n\n");
                }
                self.push_html(name, "\n");
            }
            "span" | "tabs" | "fragment" | "" => {
                if !void {
                    self.push_html(name, "");
                }
            }
            _ if !name.chars().next().is_some_and(char::is_uppercase)
                && !is_html_element(&lower) =>
            {
                // Kein HTML, kein JSX: wörtlich gemeint (z. B. `<Platzhalter>` in Fließtext).
                let literal = format!(
                    "<{name}{}{}>",
                    attrs
                        .iter()
                        .map(|(k, v)| if v.is_empty() {
                            format!(" {k}")
                        } else {
                            format!(" {k}=\"{v}\"")
                        })
                        .collect::<String>(),
                    if void { "/" } else { "" }
                );
                self.text(&literal);
            }
            _ => {
                let what = if name.chars().next().is_some_and(char::is_uppercase) {
                    "MDX-Komponente"
                } else {
                    "HTML-Element"
                };
                self.warn_once(
                    &format!("tag:{name}"),
                    format!("{what} <{name}> ohne Entsprechung – nur der Inhalt wurde übernommen."),
                );
                if !void {
                    self.push_html(name, "");
                }
            }
        }
    }

    fn wrap_html(&mut self, name: &str, prefix: &str, suffix: &str) {
        self.write(prefix);
        self.push_html(name, suffix);
    }

    fn push_html(&mut self, name: &str, suffix: &str) {
        self.html.push(OpenHtml {
            name: name.to_ascii_lowercase(),
            suffix: suffix.to_string(),
            depth: self.frames.len(),
        });
    }

    fn html_close(&mut self, name: &str) {
        if !name.chars().next().is_some_and(char::is_uppercase)
            && !name.is_empty()
            && !is_html_element(&name.to_ascii_lowercase())
            && !matches!(
                name.to_ascii_lowercase().as_str(),
                "tabitem" | "tabs" | "fragment"
            )
        {
            self.text(&format!("</{name}>"));
            return;
        }
        let name = name.to_ascii_lowercase();
        let depth = self.frames.len();
        let Some(pos) = self
            .html
            .iter()
            .rposition(|open| open.name == name && open.depth == depth)
        else {
            return;
        };
        while self.html.len() > pos {
            let open = self.html.pop().unwrap();
            if open.suffix.starts_with('\n') {
                // Blockende: vorher auf eigene Zeile.
                let suffix = open.suffix.trim_start_matches('\n').to_string();
                self.line_break();
                self.close_expr(&suffix);
            } else {
                self.close_expr(&open.suffix);
            }
        }
    }

    // ---- Abschluss ----

    fn finish_body(&mut self) -> String {
        while self.frames.len() > 1 {
            self.pop_frame();
        }
        while let Some(open) = self.html.pop() {
            self.close_expr(&open.suffix);
        }
        let mut body = std::mem::take(&mut self.frames[0].1);

        // Interne Links auf unbekannte Anker würden Typst-Fehler erzeugen → als Text belassen.
        let missing: BTreeSet<String> = self
            .internal_links
            .iter()
            .filter(|label| !self.labels.contains(*label))
            .cloned()
            .collect();
        for label in missing {
            self.warn(format!(
                "Interner Link auf unbekannten Abschnitt „#{label}“ – als Text übernommen."
            ));
            body = body.replace(&format!("#link(<{label}>)["), "#[");
        }

        self.resolve_footnotes(body)
    }

    fn resolve_footnotes(&mut self, mut body: String) -> String {
        let mut used = HashSet::new();
        // Ersetzter Text kann selbst Verweise enthalten; die werden in späteren Runden aufgelöst.
        while let Some(start) = body.find(FOOTNOTE_START) {
            let Some(len) = body[start..].find(FOOTNOTE_END) else {
                break;
            };
            let label = body[start + FOOTNOTE_START.len_utf8()..start + len].to_string();
            let end = start + len + FOOTNOTE_END.len_utf8();
            let typst_label = format!("fn-{}", slugify(&label));
            let replacement = if used.contains(&label) {
                format!("#footnote(<{typst_label}>)")
            } else if let Some(text) = self.footnotes.get(&label) {
                used.insert(label.clone());
                format!("#footnote[{text}]<{typst_label}>")
            } else {
                self.warn(format!("Fußnote „{label}“ ohne Definition."));
                format!("\\[^{}\\]", escape_markup(&label, false, false))
            };
            body.replace_range(start..end, &replacement);
        }
        let unused = self.footnotes.keys().filter(|k| !used.contains(*k)).count();
        if unused > 0 {
            self.warn(format!("{unused} Fußnote(n) ohne Verweis weggelassen."));
        }
        body.trim().to_string()
    }

    fn preamble(&self, meta: &Metadata, lang: &str, body: &str) -> String {
        let german = lang == "de";
        let mut out = String::new();
        match &self.options.source_name {
            Some(name) => out.push_str(&format!("// Aus „{name}“ mit DevMark erzeugt.\n\n")),
            None => out.push_str("// Mit DevMark aus Markdown erzeugt.\n\n"),
        }

        let title = meta.title.clone().or_else(|| self.first_h1.clone());
        let mut document = Vec::new();
        if let Some(title) = &title {
            document.push(format!("title: \"{}\"", escape_string(title)));
        }
        if !meta.authors.is_empty() {
            let authors: Vec<String> = meta
                .authors
                .iter()
                .map(|a| format!("\"{}\"", escape_string(a)))
                .collect();
            document.push(format!("author: ({},)", authors.join(", ")));
        }
        if !meta.keywords.is_empty() {
            let keywords: Vec<String> = meta
                .keywords
                .iter()
                .map(|k| format!("\"{}\"", escape_string(k)))
                .collect();
            document.push(format!("keywords: ({},)", keywords.join(", ")));
        }
        if let Some(date) = meta.date.as_deref().and_then(typst_date) {
            document.push(format!("date: {date}"));
        }
        if !document.is_empty() {
            out.push_str(&format!("#set document({})\n", document.join(", ")));
        }
        let paper = meta
            .paper
            .clone()
            .unwrap_or_else(|| self.options.paper.clone());
        out.push_str(&format!(
            "#set page(paper: \"{}\", margin: (x: 2.5cm, top: 2.5cm, bottom: 3cm), numbering: \"1\")\n",
            escape_string(&paper)
        ));
        out.push_str(&format!(
            "#set text(lang: \"{}\", size: 11pt)\n",
            escape_string(lang)
        ));
        out.push_str("#set par(justify: true)\n");
        if meta.numbering == Some(true) {
            out.push_str("#set heading(numbering: \"1.1\")\n");
        }
        out.push_str("#show heading: set block(above: 1.4em, below: 0.8em)\n");
        let accent = meta
            .accent
            .as_deref()
            .and_then(parse_hex_color)
            .map(|(r, g, b)| format!("rgb(\"#{r:02x}{g:02x}{b:02x}\")"));
        if let Some(accent) = &accent {
            out.push_str(&format!("#let accent = {accent}\n"));
            out.push_str("#show heading: set text(fill: accent)\n");
        }
        out.push_str("#show raw.where(block: true): set block(fill: luma(246), inset: 10pt, radius: 3pt, width: 100%)\n");
        if accent.is_some() {
            out.push_str("#show link: set text(fill: accent)\n");
        } else {
            out.push_str("#show link: set text(fill: rgb(\"#1a5fb4\"))\n");
        }
        if body.contains("#quote(block: true)") {
            out.push_str("#show quote.where(block: true): it => block(inset: (left: 12pt, y: 2pt), stroke: (left: 2pt + luma(200)), it.body)\n");
        }
        if self.has_table {
            out.push_str("#set table(stroke: 0.5pt + luma(190), inset: 6pt)\n");
            if accent.is_some() {
                out.push_str("#set table(fill: (_, y) => if y == 0 { accent.lighten(88%) })\n");
                out.push_str(
                    "#show table.cell.where(y: 0): set text(weight: \"bold\", fill: accent.darken(20%))\n",
                );
            } else {
                out.push_str("#show table.cell.where(y: 0): set text(weight: \"bold\")\n");
            }
        }

        let mut helpers = String::new();
        if self.needs.contains("callout") {
            let label = |kind| callout_label(kind, german).unwrap_or_default();
            let (note, tip, important, warning, caution) = (
                label("note"),
                label("tip"),
                label("important"),
                label("warning"),
                label("caution"),
            );
            // Hinweise in der Akzentfarbe; Tipp/Warnung/Vorsicht behalten ihre Signalfarben.
            let note_color = if accent.is_some() {
                "accent"
            } else {
                "rgb(\"#0969da\")"
            };
            helpers.push_str(&format!(
                "#let callout(kind: \"note\", title: none, body) = {{\n  let styles = (\n    note: ({note_color}, [{note}]),\n    tip: (rgb(\"#1a7f37\"), [{tip}]),\n    important: (rgb(\"#8250df\"), [{important}]),\n    warning: (rgb(\"#9a6700\"), [{warning}]),\n    caution: (rgb(\"#cf222e\"), [{caution}]),\n    details: (luma(110), none),\n  )\n  let (accent, label) = styles.at(kind, default: styles.note)\n  let heading = if title != none {{ title }} else {{ label }}\n  block(\n    width: 100%,\n    inset: (left: 12pt, rest: 10pt),\n    stroke: (left: 3pt + accent),\n    fill: accent.lighten(93%),\n    radius: (right: 3pt),\n    {{\n      if heading != none {{ text(weight: \"bold\", fill: accent, heading); parbreak() }}\n      body\n    }},\n  )\n}}\n"
            ));
        }
        if self.needs.contains("task") {
            helpers.push_str("#let task(done) = box(width: 0.75em, height: 0.75em, baseline: 0.05em, stroke: 0.6pt, radius: 1pt, inset: 0pt, if done { align(center + horizon, text(size: 0.7em)[✓]) })\n");
        }
        if self.needs.contains("kbd") {
            helpers.push_str("#let kbd(it) = box(inset: (x: 3pt), outset: (y: 2pt), stroke: 0.5pt + luma(150), radius: 2pt, fill: luma(248), text(size: 0.85em, it))\n");
        }
        helpers.push_str(&math::helper_definitions(&self.math_helpers));
        if !helpers.is_empty() {
            out.push_str("\n// Hilfsfunktionen\n");
            out.push_str(&helpers);
        }

        if let Some(title) = &meta.title {
            out.push('\n');
            out.push_str("#align(center)[\n");
            out.push_str(&format!(
                "  #text(size: 1.9em, weight: \"bold\")[{}]\n",
                escape_markup(title, false, false)
            ));
            if let Some(subtitle) = &meta.subtitle {
                out.push_str(&format!(
                    "  #v(0.2em)\n  #text(size: 1.2em)[{}]\n",
                    escape_markup(subtitle, false, false)
                ));
            }
            let mut byline: Vec<String> = meta
                .authors
                .iter()
                .map(|a| escape_markup(a, false, false))
                .collect();
            if let Some(date) = &meta.date {
                byline.push(escape_markup(date, false, false));
            }
            if !byline.is_empty() {
                out.push_str(&format!("  #v(0.4em)\n  {}\n", byline.join(" · ")));
            }
            out.push_str("]\n#v(1.5em)\n");
        }
        if meta.toc == Some(true) {
            out.push_str("\n#outline()\n");
        }
        if !body.is_empty() {
            out.push('\n');
        }
        out
    }
}

enum ImageCall {
    Image(String),
    Placeholder(String),
}

fn render_table(table: TableState) -> String {
    let columns = table
        .header
        .iter()
        .chain(table.rows.iter())
        .map(Vec::len)
        .chain(std::iter::once(table.alignments.len()))
        .max()
        .unwrap_or(1)
        .max(1);
    let mut out = format!("#table(\n  columns: {columns},\n");
    if table.alignments.iter().any(|a| *a != Alignment::None) {
        let aligns: Vec<&str> = (0..columns)
            .map(|ix| match table.alignments.get(ix) {
                Some(Alignment::Left) => "left",
                Some(Alignment::Center) => "center",
                Some(Alignment::Right) => "right",
                _ => "auto",
            })
            .collect();
        out.push_str(&format!("  align: ({},),\n", aligns.join(", ")));
    }
    let row_line = |row: &[String]| {
        let mut cells: Vec<String> = row.iter().map(|c| format!("[{c}]")).collect();
        cells.resize(columns, "[]".to_string());
        cells.join(", ")
    };
    if let Some(header) = &table.header {
        out.push_str(&format!("  table.header({}),\n", row_line(header)));
    }
    for row in &table.rows {
        out.push_str(&format!("  {},\n", row_line(row)));
    }
    out.push(')');
    out
}

// ---------------------------------------------------------------------------
// Hilfsfunktionen

/// Escaped Text für Typst-Markup. `line_start`: Text beginnt eine Zeile (Listen-/Überschriften-
/// syntax). `after_expr`: direkt davor endet ein `#ausdruck`, der sonst weitergelesen würde.
pub fn escape_markup(text: &str, line_start: bool, after_expr: bool) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut at_start = line_start;
    let mut prev: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        let in_url = out.ends_with("http:")
            || out.ends_with("https:")
            || out.ends_with("http:/")
            || out.ends_with("https:/");
        let escape = match c {
            '\\' | '#' | '*' | '_' | '`' | '$' | '<' | '>' | '@' | '[' | ']' | '~' => true,
            '=' | '+' => at_start,
            '-' => at_start || matches!(next, Some('-' | '?')) || prev == Some('-'),
            // `//` und `/*` wären Kommentare, `/ ` am Zeilenanfang eine Begriffsliste.
            '/' => at_start || ((matches!(next, Some('/' | '*')) || prev == Some('/')) && !in_url),
            // `1.` am Zeilenanfang wäre eine nummerierte Liste.
            '.' if at_start && prev.is_some_and(|p| p.is_ascii_digit()) => true,
            // Nach `#f[…]` würde `(` als Argumentliste und `.x` als Feldzugriff gelesen.
            '(' => prev.is_none() && after_expr,
            '.' => {
                prev.is_none() && after_expr && next.is_some_and(|n| n.is_alphabetic() || n == '_')
            }
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
        if c == '\n' {
            at_start = true;
        } else if !(c == ' ' || c == '\t' || (at_start && c.is_ascii_digit())) {
            at_start = false;
        }
        prev = Some(c);
    }
    out
}

/// Escaped Text für ein Typst-String-Literal.
pub fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn indent(text: &str, width: usize) -> String {
    let pad = " ".repeat(width);
    let mut out = String::with_capacity(text.len());
    for (ix, line) in text.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(&pad);
            }
        }
        out.push_str(line);
    }
    out
}

fn heading_level(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// GitHub-artiger Anker: klein, Buchstaben/Ziffern, Leerzeichen → `-`.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        // Typst-Labels erlauben nur Bezeichner-Zeichen; `₂` o. Ä. fällt weg.
        if c.is_alphabetic() || c.is_ascii_digit() || c == '_' || c == '-' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    // Typst-Labels dürfen nicht mit Ziffer oder `-` beginnen? Doch, aber sicher ist sicher.
    if out.starts_with(|c: char| c == '-' || c.is_ascii_digit()) {
        out.insert(0, 's');
    }
    out
}

fn code_lang(info: &str) -> String {
    // MDX/Docusaurus: ```ts title="x.ts" {1,3}
    let first = info
        .split(|c: char| c.is_whitespace() || c == '{' || c == ',')
        .next()
        .unwrap_or("");
    first
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '#' | '.' | '_'))
        .collect()
}

fn longest_run(text: &str, ch: char) -> usize {
    let mut best = 0;
    let mut current = 0;
    for c in text.chars() {
        if c == ch {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

pub(crate) fn plain_text(events: &[Event]) -> String {
    let mut out = String::new();
    for event in events {
        match event {
            Event::Text(t) | Event::Code(t) | Event::InlineMath(t) => out.push_str(t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out
}

pub(crate) fn callout_kind(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "note",
        BlockQuoteKind::Tip => "tip",
        BlockQuoteKind::Important => "important",
        BlockQuoteKind::Warning => "warning",
        BlockQuoteKind::Caution => "caution",
    }
}

/// `#rgb` oder `#rrggbb` (das `#` darf fehlen) → RGB.
fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |s: &str| u8::from_str_radix(s, 16).ok();
    match hex.len() {
        3 => {
            let mut digits = hex.chars().map(|c| channel(&c.to_string().repeat(2)));
            Some((digits.next()??, digits.next()??, digits.next()??))
        }
        6 => Some((
            channel(&hex[0..2])?,
            channel(&hex[2..4])?,
            channel(&hex[4..6])?,
        )),
        _ => None,
    }
}

/// Kontrastverhältnis (WCAG) einer Farbe zu Weiß, 1 bis 21.
fn contrast_on_white((r, g, b): (u8, u8, u8)) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    1.05 / (luminance + 0.05)
}

/// Überschrift einer Hinweisbox ohne eigenen Titel; `details` hat keine.
pub(crate) fn callout_label(kind: &str, german: bool) -> Option<&'static str> {
    Some(match (kind, german) {
        ("note", true) => "Hinweis",
        ("tip", true) => "Tipp",
        ("important", true) => "Wichtig",
        ("warning", true) => "Warnung",
        ("caution", true) => "Vorsicht",
        ("note", false) => "Note",
        ("tip", false) => "Tip",
        ("important", false) => "Important",
        ("warning", false) => "Warning",
        ("caution", false) => "Caution",
        _ => return None,
    })
}

/// Bekannte Hinweis-Komponenten (MDX: Docusaurus, Starlight, Nextra, Mintlify …).
pub(crate) fn admonition_kind(name: &str, variant: Option<&str>) -> Option<&'static str> {
    let by_name = |n: &str| -> Option<&'static str> {
        Some(match n.to_ascii_lowercase().as_str() {
            "note" | "info" | "information" | "default" | "secondary" => "note",
            "tip" | "success" | "check" | "hint" => "tip",
            "important" => "important",
            "warning" | "warn" => "warning",
            "caution" | "danger" | "error" | "alert" => "caution",
            _ => return None,
        })
    };
    let first_upper = name.chars().next().is_some_and(char::is_uppercase);
    match name.to_ascii_lowercase().as_str() {
        "callout" | "admonition" | "aside" | "alert" | "notice" => {
            Some(variant.and_then(by_name).unwrap_or("note"))
        }
        _ if first_upper => by_name(name),
        _ => None,
    }
}

pub(crate) fn is_html_element(name: &str) -> bool {
    matches!(
        name,
        "a" | "abbr"
            | "address"
            | "area"
            | "article"
            | "aside"
            | "audio"
            | "b"
            | "bdi"
            | "bdo"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "canvas"
            | "caption"
            | "center"
            | "cite"
            | "code"
            | "col"
            | "colgroup"
            | "data"
            | "dd"
            | "del"
            | "details"
            | "dfn"
            | "dialog"
            | "div"
            | "dl"
            | "dt"
            | "em"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "font"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hr"
            | "html"
            | "i"
            | "iframe"
            | "img"
            | "input"
            | "ins"
            | "kbd"
            | "label"
            | "legend"
            | "li"
            | "link"
            | "main"
            | "map"
            | "mark"
            | "meta"
            | "meter"
            | "nav"
            | "noscript"
            | "object"
            | "ol"
            | "optgroup"
            | "option"
            | "output"
            | "p"
            | "picture"
            | "pre"
            | "progress"
            | "q"
            | "rp"
            | "rt"
            | "ruby"
            | "s"
            | "samp"
            | "script"
            | "section"
            | "select"
            | "small"
            | "source"
            | "span"
            | "strike"
            | "strong"
            | "style"
            | "sub"
            | "summary"
            | "sup"
            | "svg"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "time"
            | "title"
            | "tr"
            | "track"
            | "tt"
            | "u"
            | "ul"
            | "var"
            | "video"
            | "wbr"
    )
}

fn is_void(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "br" | "hr" | "img" | "input" | "meta" | "link" | "source" | "wbr" | "col" | "area"
    )
}

fn css_length(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse::<f64>().ok().map(|p| format!("{p}%"));
    }
    let number = value.trim_end_matches("px");
    number
        .parse::<f64>()
        .ok()
        .map(|px| format!("{}pt", (px * 0.75 * 100.0).round() / 100.0))
}

fn typst_date(date: &str) -> Option<String> {
    let date = date.get(..10)?;
    let mut parts = date.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    (1..=12).contains(&month).then_some(())?;
    (1..=31).contains(&day).then_some(())?;
    Some(format!(
        "datetime(year: {year}, month: {month}, day: {day})"
    ))
}

pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Relativer Pfad von `dir` zu `target` (beide absolut oder beide relativ).
pub(crate) fn relative_path(target: &Path, dir: &Path) -> Option<PathBuf> {
    let target = normalize(target);
    let dir = normalize(dir);
    let target_parts: Vec<Component> = target.components().collect();
    let dir_parts: Vec<Component> = dir.components().collect();
    if target_parts.first() != dir_parts.first() {
        return None;
    }
    let common = target_parts
        .iter()
        .zip(dir_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = PathBuf::new();
    for _ in common..dir_parts.len() {
        out.push("..");
    }
    for part in &target_parts[common..] {
        out.push(part.as_os_str());
    }
    Some(out)
}

pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Mini-HTML-Tokenizer (für HTML-Blöcke und JSX-Tags)

#[derive(Debug, PartialEq)]
pub(crate) enum HtmlToken {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    Close {
        name: String,
    },
    Text(String),
    Comment(String),
}

pub(crate) fn tokenize_html(html: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let mut text = String::new();
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            flush_text(&mut text, &mut tokens);
            let end = after.find("-->").unwrap_or(after.len());
            tokens.push(HtmlToken::Comment(after[..end].to_string()));
            rest = after.get(end + 3..).unwrap_or("");
            continue;
        }
        if rest.starts_with('<')
            && let Some((token, len)) = parse_tag(rest)
        {
            flush_text(&mut text, &mut tokens);
            tokens.push(token);
            rest = &rest[len..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        text.push(c);
        rest = &rest[c.len_utf8()..];
    }
    flush_text(&mut text, &mut tokens);
    tokens
}

fn flush_text(text: &mut String, tokens: &mut Vec<HtmlToken>) {
    if !text.is_empty() {
        tokens.push(HtmlToken::Text(std::mem::take(text)));
    }
}

fn parse_tag(input: &str) -> Option<(HtmlToken, usize)> {
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut i = 1;
    let closing = chars.get(i).is_some_and(|(_, c)| *c == '/');
    if closing {
        i += 1;
    }
    let name_start = i;
    while chars
        .get(i)
        .is_some_and(|(_, c)| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        i += 1;
    }
    let name: String = chars[name_start..i].iter().map(|(_, c)| c).collect();
    // `<>` / `</>` sind JSX-Fragmente.
    if name.is_empty() && !matches!(chars.get(i), Some((_, '>'))) {
        return None;
    }
    if !name.is_empty() && !name.starts_with(|c: char| c.is_alphabetic()) {
        return None;
    }
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while chars.get(i).is_some_and(|(_, c)| c.is_whitespace()) {
            i += 1;
        }
        let (_, c) = *chars.get(i)?;
        match c {
            '>' => {
                let len = chars[i].0 + 1;
                let token = if closing {
                    HtmlToken::Close { name }
                } else {
                    HtmlToken::Open {
                        name,
                        attrs,
                        self_closing,
                    }
                };
                return Some((token, len));
            }
            '/' => {
                self_closing = true;
                i += 1;
            }
            '{' => {
                // JSX-Spread `{...props}` überspringen.
                let mut depth = 0;
                while let Some((_, c)) = chars.get(i) {
                    i += 1;
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                let key_start = i;
                while chars
                    .get(i)
                    .is_some_and(|(_, c)| !c.is_whitespace() && !matches!(c, '=' | '>' | '/'))
                {
                    i += 1;
                }
                let key: String = chars[key_start..i].iter().map(|(_, c)| c).collect();
                if key.is_empty() {
                    return None;
                }
                let mut value = String::new();
                if chars.get(i).is_some_and(|(_, c)| *c == '=') {
                    i += 1;
                    match chars.get(i).map(|(_, c)| *c) {
                        Some(q @ ('"' | '\'')) => {
                            i += 1;
                            while let Some((_, c)) = chars.get(i) {
                                i += 1;
                                if *c == q {
                                    break;
                                }
                                value.push(*c);
                            }
                            // Aus JSX `{'text'}` / `{`text`}` (siehe preprocess_mdx).
                            for inner in ['\'', '`'] {
                                if value.len() >= 2
                                    && value.starts_with(inner)
                                    && value.ends_with(inner)
                                {
                                    value = value[1..value.len() - 1].to_string();
                                }
                            }
                        }
                        Some('{') => {
                            let mut depth = 0;
                            while let Some((_, c)) = chars.get(i) {
                                i += 1;
                                match c {
                                    '{' => depth += 1,
                                    '}' => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                if depth > 0 && !(*c == '{' && depth == 1) {
                                    value.push(*c);
                                }
                            }
                            // `{"text"}` / `{'text'}` / `{`text`}` → text
                            let trimmed = value.trim();
                            for q in ['"', '\'', '`'] {
                                if trimmed.len() >= 2
                                    && trimmed.starts_with(q)
                                    && trimmed.ends_with(q)
                                {
                                    value = trimmed[1..trimmed.len() - 1].to_string();
                                    break;
                                }
                            }
                        }
                        _ => {
                            while chars
                                .get(i)
                                .is_some_and(|(_, c)| !c.is_whitespace() && *c != '>')
                            {
                                value.push(chars[i].1);
                                i += 1;
                            }
                        }
                    }
                }
                attrs.push((key, decode_entities(&value)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(md: &str) -> String {
        let options = Options {
            standalone: false,
            ..Options::default()
        };
        markdown_to_typst(md, &options).typst
    }

    fn mdx(md: &str) -> Conversion {
        let options = Options {
            standalone: false,
            mdx: true,
            ..Options::default()
        };
        markdown_to_typst(md, &options)
    }

    #[test]
    fn headings_get_labels() {
        assert_eq!(
            body("# Hallo Welt\n\n## Größe & Form"),
            "= Hallo Welt <hallo-welt>\n\n== Größe & Form <größe--form>\n"
        );
    }

    #[test]
    fn inline_formatting() {
        assert_eq!(
            body("Ein *kursiver*, **fetter** und ~~alter~~ Text mit `code`."),
            "Ein #emph[kursiver], #strong[fetter] und #strike[alter] Text mit `code`.\n"
        );
    }

    #[test]
    fn escapes_typst_syntax() {
        assert_eq!(
            body("Preis: 5 $ #1 @user a < b > c a_b"),
            "Preis: 5 \\$ \\#1 \\@user a \\< b \\> c a\\_b\n"
        );
        assert_eq!(body("1\\. nicht nummeriert"), "1\\. nicht nummeriert\n");
        assert_eq!(body("a -- b // c"), "a \\-\\- b \\/\\/ c\n");
        assert_eq!(body("Siehe https://typst.app"), "Siehe https://typst.app\n");
    }

    #[test]
    fn text_after_expression_is_protected() {
        assert_eq!(body("**fett**.weiter"), "#strong[fett]\\.weiter\n");
    }

    #[test]
    fn lists_nest_with_indentation() {
        assert_eq!(body("- a\n- b\n  - c\n"), "- a\n- b\n  - c\n");
        assert_eq!(body("3. drei\n4. vier\n"), "3. drei\n4. vier\n");
        assert_eq!(
            body("- [x] fertig\n- [ ] offen\n"),
            "- #task(true) fertig\n- #task(false) offen\n"
        );
    }

    #[test]
    fn loose_lists_keep_blank_lines() {
        assert_eq!(
            body("- a\n\n  zweiter Absatz\n\n- b\n"),
            "- a\n\n  zweiter Absatz\n\n- b\n"
        );
    }

    #[test]
    fn code_blocks() {
        assert_eq!(
            body("```rust title=\"x\"\nfn main() {}\n```"),
            "```rust\nfn main() {}\n```\n"
        );
        assert_eq!(body("````md\n```\n````"), "````md\n```\n````\n");
    }

    #[test]
    fn links_and_internal_anchors() {
        assert_eq!(
            body("[Typst](https://typst.app)"),
            "#link(\"https://typst.app\")[Typst]\n"
        );
        assert_eq!(
            body("# Ziel\n\n[hin](#ziel)"),
            "= Ziel <ziel>\n\n#link(<ziel>)[hin]\n"
        );
        assert_eq!(body("[weg](#nirgends)"), "#[weg]\n");
    }

    #[test]
    fn footnotes_become_inline() {
        assert_eq!(
            body("Text[^1] und nochmal[^1].\n\n[^1]: Die Notiz."),
            "Text#footnote[Die Notiz.]<fn-s1> und nochmal#footnote(<fn-s1>).\n"
        );
    }

    #[test]
    fn tables() {
        assert_eq!(
            body("| A | B |\n|:--|--:|\n| 1 | 2 |\n"),
            "#table(\n  columns: 2,\n  align: (left, right,),\n  table.header([A], [B]),\n  [1], [2],\n)\n"
        );
    }

    #[test]
    fn math() {
        assert_eq!(
            body("Es gilt $a^2 + b^2 = c^2$."),
            "Es gilt $a^(2) + b^(2) = c^(2)$.\n"
        );
        assert_eq!(body("$$\n\\frac{1}{2}\n$$"), "$ frac(1,2) $\n");
    }

    #[test]
    fn github_alerts_and_quotes() {
        assert_eq!(
            body("> [!WARNING]\n> Achtung"),
            "#callout(kind: \"warning\")[\nAchtung\n]\n"
        );
        assert_eq!(body("> Zitat"), "#quote(block: true)[\nZitat\n]\n");
    }

    #[test]
    fn frontmatter_is_read() {
        let conversion = markdown_to_typst(
            "---\ntitle: Test\nlang: en-US\n---\n\nText",
            &Options::default(),
        );
        assert_eq!(conversion.metadata.title.as_deref(), Some("Test"));
        assert!(conversion.typst.contains("#set text(lang: \"en\""));
        assert!(conversion.typst.contains("#set document(title: \"Test\")"));
    }

    #[test]
    fn mdx_imports_and_components() {
        let conversion = mdx(
            "import Callout from './Callout'\nexport const meta = {\n  a: 1,\n}\n\n# Titel\n\n<Callout type=\"warning\" title=\"Achtung\">\n\nInhalt **fett**\n\n</Callout>\n\n{/* Kommentar */}\n\nText {props.x} weiter.\n",
        );
        assert_eq!(
            conversion.typst,
            "= Titel <titel>\n\n#callout(kind: \"warning\", title: [Achtung])[\nInhalt #strong[fett]\n]\n\nText  weiter.\n"
        );
        assert_eq!(conversion.warnings.len(), 2);
    }

    #[test]
    fn mdx_jsx_attributes() {
        let conversion = mdx(
            "<Callout type={\"warning\"} {...rest}>\n\nText\n\n</Callout>\n\n<Chart data={[1, 2]} />\n",
        );
        assert_eq!(conversion.typst, "#callout(kind: \"warning\")[\nText\n]\n");
    }

    #[test]
    fn mdx_keeps_math_braces() {
        assert_eq!(
            mdx("Energie $E = \\frac{hc}{\\lambda}$ {x}\n").typst,
            "Energie $E = frac(h c,lambda)$\n"
        );
        assert!(mdx("$$\n\\sqrt{2}\n$$\n").typst.contains("mitexsqrt(2)"));
    }

    #[test]
    fn mdx_multiline_jsx() {
        let conversion = mdx("<Steps\n  items={[\n    { a: 1 },\n  ]}\n/>\n\nText\n");
        assert_eq!(conversion.typst, "Text\n");
    }

    #[test]
    fn container_directives() {
        assert_eq!(
            body(":::tip Mein Titel\n\nInhalt\n\n:::\n"),
            "#callout(kind: \"tip\", title: [Mein Titel])[\nInhalt\n]\n"
        );
        assert_eq!(
            body(":::note[Starlight]\nText\n:::\n"),
            "#callout(kind: \"note\", title: [Starlight])[\nText\n]\n"
        );
    }

    #[test]
    fn mdx_component_without_blank_lines() {
        let conversion = mdx("<Note>\nKurzer *Hinweis*\n</Note>\n");
        assert_eq!(
            conversion.typst,
            "#callout(kind: \"note\")[\nKurzer #emph[Hinweis]\n]\n"
        );
    }

    #[test]
    fn warnings_carry_source_line() {
        let options = Options {
            standalone: false,
            ..Options::default()
        };
        let source =
            "---\ntitle: T\n---\n:::tip\nText\n:::\n\n![x](http://a/b.png)\n\n[weg](#nirgends)\n";
        let warnings = markdown_to_typst(source, &options).warnings;
        assert_eq!(warnings[0].line, Some(8), "{warnings:?}");
        assert!(warnings[0].message.contains("Entferntes Bild"));
        // Interne Links werden erst am Ende geprüft – ohne Zeile.
        assert_eq!(warnings[1].line, None);

        let conversion = mdx("import A from './a'\n\n<Foo>\n\nInhalt\n\n</Foo>\n");
        let foo = conversion
            .warnings
            .iter()
            .find(|w| w.message.contains("<Foo>"))
            .unwrap();
        assert_eq!(foo.line, Some(3));
        assert_eq!(foo.to_string(), format!("Zeile 3: {}", foo.message));
    }

    #[test]
    fn accent_color_from_frontmatter() {
        let options = Options::default();
        let conversion = markdown_to_typst(
            "---\ntitle: T\naccent: \"#0A7F6F\"\n---\n> [!NOTE]\n> Box\n\n| A |\n|---|\n| 1 |\n",
            &options,
        );
        assert!(conversion.warnings.is_empty(), "{:?}", conversion.warnings);
        let typst = &conversion.typst;
        assert!(typst.contains("#let accent = rgb(\"#0a7f6f\")"));
        assert!(typst.contains("#show heading: set text(fill: accent)"));
        assert!(typst.contains("#show link: set text(fill: accent)"));
        assert!(typst.contains("note: (accent, [Hinweis])"));
        assert!(typst.contains("fill: accent.darken(20%)"));
        crate::compile_pdf(typst, Path::new("/tmp/devmark-accent.typ")).unwrap();

        let pale = markdown_to_typst("---\naccent: \"#ffd700\"\n---\nText\n", &options);
        assert_eq!(pale.warnings.len(), 1);
        assert_eq!(pale.warnings[0].line, Some(2));
        assert!(
            pale.warnings[0].message.contains("(1,4:1"),
            "{:?}",
            pale.warnings
        );
        assert!(pale.typst.contains("#let accent"));

        let broken = markdown_to_typst("---\naccent: grün\n---\nText\n", &options);
        assert!(broken.warnings[0].message.contains("nicht erkannt"));
        assert!(!broken.typst.contains("accent"));
    }

    #[test]
    fn hex_colors_and_contrast() {
        assert_eq!(parse_hex_color("#0a7f6f"), Some((10, 127, 111)));
        assert_eq!(parse_hex_color("f80"), Some((255, 136, 0)));
        assert_eq!(parse_hex_color("#12345"), None);
        assert!((contrast_on_white((0, 0, 0)) - 21.0).abs() < 0.01);
        assert!((contrast_on_white((255, 255, 255)) - 1.0).abs() < 0.01);
        assert!(contrast_on_white((10, 127, 111)) > 4.5);
    }

    #[test]
    fn unknown_component_keeps_content() {
        let conversion = mdx("<Foo bar={1}>\n\nInhalt\n\n</Foo>\n");
        assert_eq!(conversion.typst, "Inhalt\n");
        assert_eq!(conversion.warnings.len(), 1);
    }

    #[test]
    fn inline_html() {
        assert_eq!(
            body("H<sub>2</sub>O und x<sup>2</sup>, <kbd>Cmd</kbd>"),
            "H#sub[2]O und x#super[2], #kbd[Cmd]\n"
        );
        assert_eq!(body("<!-- geheim -->\n\nText"), "// geheim\n\nText\n");
    }

    #[test]
    fn details_block() {
        assert_eq!(
            body("<details>\n<summary>Mehr</summary>\n\nVersteckt\n\n</details>\n"),
            "#callout(kind: \"details\")[\n#strong[Mehr]\n\nVersteckt\n]\n"
        );
    }

    #[test]
    fn html_tokenizer() {
        assert_eq!(
            tokenize_html("<a href=\"x\">t</a><br/>"),
            vec![
                HtmlToken::Open {
                    name: "a".into(),
                    attrs: vec![("href".into(), "x".into())],
                    self_closing: false
                },
                HtmlToken::Text("t".into()),
                HtmlToken::Close { name: "a".into() },
                HtmlToken::Open {
                    name: "br".into(),
                    attrs: vec![],
                    self_closing: true
                },
            ]
        );
    }

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative_path(Path::new("/a/b/img/x.png"), Path::new("/a/c")),
            Some(PathBuf::from("../b/img/x.png"))
        );
    }
}
