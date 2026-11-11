//! Markdown → statisches HTML: flache Seiten plus ein Stylesheet, offline und ohne JavaScript.
//!
//! Relative Links auf mitexportierte Markdown-Dateien werden zu `.html`-Links, Ordner bleiben
//! Ordner, Hinweisboxen erscheinen wie im PDF. Formeln bleiben als LaTeX-Code stehen, weil
//! ihre Darstellung im Browser ein Skript bräuchte.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use pulldown_cmark::{CowStr, Event, Parser, Tag, TagEnd, html};

use crate::Warning;
use crate::markdown::{
    HtmlToken, admonition_kind, callout_kind, callout_label, normalize, parser_options,
    percent_decode, plain_text, preprocess_directives, preprocess_mdx, slugify, split_frontmatter,
    tokenize_html,
};

/// Name des gemeinsamen Stylesheets in der Ausgabewurzel.
pub const STYLESHEET: &str = "style.css";
/// Erste Zeile des Stylesheets – kennzeichnet einen Ordner als DevMark-Export.
const STYLESHEET_MARKER: &str = "/* DevMark HTML-Export */";

pub struct PageOptions<'a> {
    /// Quelldatei (absolut); relative Links und Bilder werden zu ihrem Ordner aufgelöst.
    pub path: &'a Path,
    /// Wurzel des Exports; die Seite liegt relativ dazu wie die Quelle.
    pub root: &'a Path,
    /// Mitexportierte Markdown-Dateien (absolut, normalisiert). Links darauf werden zu
    /// `.html`-Links, Links auf andere Markdown-Dateien bleiben nur als Text stehen.
    pub pages: &'a HashSet<PathBuf>,
    /// Kopfzeile mit Link auf die Übersicht `index.html`.
    pub with_index: bool,
}

pub struct Page {
    pub title: String,
    pub html: String,
    pub warnings: Vec<Warning>,
    /// Lokale Bilder und Dateien, die neben die Seiten kopiert werden müssen.
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asset {
    pub source: PathBuf,
    /// Ziel relativ zur Ausgabewurzel.
    pub target: PathBuf,
}

/// Wandelt ein Markdown-/MDX-Dokument in eine vollständige HTML-Seite um.
pub fn markdown_to_html(source: &str, options: &PageOptions) -> Page {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let (metadata, body) = split_frontmatter(source);
    let mut warnings = Vec::new();
    let mdx = options
        .path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mdx"));
    let body = if mdx {
        preprocess_mdx(body, &mut warnings).0
    } else {
        body.to_string()
    };
    let (body, _) = preprocess_directives(&body);
    let lang = metadata
        .lang
        .as_deref()
        .and_then(|lang| lang.split(['-', '_']).next())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "de".into());

    let events: Vec<Event> = Parser::new_ext(&body, parser_options()).collect();
    let page_dir = options
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let rel_dir = page_dir
        .strip_prefix(options.root)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut writer = Writer {
        options,
        german: lang == "de",
        page_dir,
        rel_dir,
        warnings,
        warned: HashSet::new(),
        assets: Vec::new(),
        links: Vec::new(),
        skipping: None,
    };
    let (events, first_h1) = writer.rewrite(events);

    let mut content = String::new();
    if first_h1.is_none()
        && let Some(title) = &metadata.title
    {
        content.push_str(&format!("<h1>{}</h1>\n", escape(title)));
    }
    html::push_html(&mut content, events.into_iter());

    let title = metadata
        .title
        .clone()
        .or(first_h1)
        .unwrap_or_else(|| file_stem(options.path));
    let prefix = "../".repeat(writer.rel_dir.components().count());
    let nav = if options.with_index {
        let label = if writer.german {
            "Übersicht"
        } else {
            "Overview"
        };
        format!("<nav><a href=\"{prefix}index.html\">{label}</a></nav>\n")
    } else {
        String::new()
    };
    let html = format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"DevMark\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"{prefix}{STYLESHEET}\">\n</head>\n<body>\n{nav}<main>\n{content}</main>\n</body>\n</html>\n",
        escape(&title)
    );
    Page {
        title,
        html,
        warnings: writer.warnings,
        assets: writer.assets,
    }
}

/// Ergebnis eines Exports.
pub struct Export {
    pub pages: usize,
    /// Hinweise je Quelldatei.
    pub warnings: Vec<(PathBuf, Warning)>,
    /// Einstiegsseite: `index.html` bzw. die einzige Seite.
    pub entry: PathBuf,
}

/// Exportiert `sources` (Markdown-Dateien unterhalb von `root`) nach `out_dir`: je Quelle eine
/// `.html`-Datei am gleichen relativen Ort, dazu `style.css`, verwendete Bilder und – bei
/// mehreren Seiten ohne eigene `index.md` – eine Übersicht `index.html`.
pub fn export_site(root: &Path, sources: &[PathBuf], out_dir: &Path) -> io::Result<Export> {
    export(root, sources, out_dir, &|path| {
        std::fs::read_to_string(path)
    })
}

/// Exportiert ein einzelnes Dokument mit dem Inhalt `source` (z. B. ungespeicherter
/// Editor-Text) nach `out_dir`; Links auf andere Markdown-Dateien bleiben als Text stehen.
pub fn export_document(source: &str, path: &Path, out_dir: &Path) -> io::Result<Export> {
    let root = path.parent().unwrap_or(Path::new(""));
    export(root, &[path.to_path_buf()], out_dir, &|_| {
        Ok(source.to_string())
    })
}

fn export(
    root: &Path,
    sources: &[PathBuf],
    out_dir: &Path,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> io::Result<Export> {
    let root = normalize(&std::path::absolute(root)?);
    if out_dir.exists() && std::fs::read_dir(out_dir)?.next().is_some() {
        let marked = std::fs::read_to_string(out_dir.join(STYLESHEET))
            .is_ok_and(|css| css.starts_with(STYLESHEET_MARKER));
        if !marked {
            return Err(io::Error::other(format!(
                "„{}“ ist nicht leer und kein früherer HTML-Export – nichts überschrieben.",
                out_dir.display()
            )));
        }
    }
    let sources: Vec<PathBuf> = sources
        .iter()
        .map(|source| std::path::absolute(source).map(|path| normalize(&path)))
        .collect::<io::Result<_>>()?;
    let pages: HashSet<PathBuf> = sources.iter().cloned().collect();
    let has_index = sources.iter().any(|source| {
        source.parent() == Some(root.as_path()) && file_stem(source).eq_ignore_ascii_case("index")
    });
    let with_index = sources.len() > 1 && !has_index;

    std::fs::create_dir_all(out_dir)?;
    let mut warnings = Vec::new();
    let mut copied = HashSet::new();
    let mut entries = Vec::new();
    for source in &sources {
        let text = read(source)?;
        let page = markdown_to_html(
            &text,
            &PageOptions {
                path: source,
                root: &root,
                pages: &pages,
                with_index,
            },
        );
        let rel = source
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| PathBuf::from(source.file_name().unwrap_or_default()))
            .with_extension("html");
        let target = out_dir.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &page.html)?;
        for asset in page.assets {
            if copied.insert(asset.target.clone()) {
                let target = out_dir.join(&asset.target);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&asset.source, target)?;
            }
        }
        warnings.extend(page.warnings.into_iter().map(|w| (source.clone(), w)));
        entries.push((rel, page.title));
    }
    std::fs::write(out_dir.join(STYLESHEET), stylesheet())?;
    let entry = if with_index {
        let title = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        std::fs::write(out_dir.join("index.html"), index_page(&title, &entries))?;
        out_dir.join("index.html")
    } else if has_index {
        out_dir.join("index.html")
    } else {
        out_dir.join(
            entries
                .first()
                .map(|(rel, _)| rel.clone())
                .unwrap_or_default(),
        )
    };
    Ok(Export {
        pages: entries.len(),
        warnings,
        entry,
    })
}

/// Alle Markdown-Dateien unterhalb von `root`, sortiert; versteckte Dateien und Ordner
/// (z. B. `.trash`, `.git`) bleiben außen vor.
pub fn collect_markdown(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if is_markdown(&path) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|ext| matches!(ext.as_str(), "md" | "markdown" | "mdx"))
}

struct Writer<'a> {
    options: &'a PageOptions<'a>,
    german: bool,
    /// Ordner der Quelldatei (absolut).
    page_dir: PathBuf,
    /// Ordner der Seite relativ zur Ausgabewurzel.
    rel_dir: PathBuf,
    warnings: Vec<Warning>,
    warned: HashSet<String>,
    assets: Vec<Asset>,
    /// Je offenem Link: bleibt er ein Link?
    links: Vec<bool>,
    /// Inhalt eines `<script>`/`<style>`-Elements wird verworfen.
    skipping: Option<String>,
}

impl Writer<'_> {
    fn warn_once(&mut self, key: &str, message: String) {
        if self.warned.insert(key.to_string()) {
            self.warnings.push(Warning::new(message));
        }
    }

    /// Passt die Ereignisse für HTML an; liefert außerdem den Text der ersten H1.
    fn rewrite<'e>(&mut self, events: Vec<Event<'e>>) -> (Vec<Event<'e>>, Option<String>) {
        let ids = heading_ids(&events);
        let mut first_h1 = None;
        let mut heading = 0;
        let mut out = Vec::with_capacity(events.len());
        for (ix, event) in events.iter().enumerate() {
            match event {
                Event::Start(Tag::Heading {
                    level,
                    classes,
                    attrs,
                    ..
                }) => {
                    if *level == pulldown_cmark::HeadingLevel::H1 && first_h1.is_none() {
                        first_h1 = Some(heading_text(&events[ix..]));
                    }
                    out.push(Event::Start(Tag::Heading {
                        level: *level,
                        id: Some(CowStr::from(ids[heading].clone())),
                        classes: classes.clone(),
                        attrs: attrs.clone(),
                    }));
                    heading += 1;
                }
                Event::Start(Tag::BlockQuote(Some(kind))) => {
                    out.push(Event::Html(
                        self.callout_open(callout_kind(*kind), None).into(),
                    ));
                }
                Event::End(TagEnd::BlockQuote(Some(_))) => {
                    out.push(Event::Html("</aside>\n".into()));
                }
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) => match self.link(dest_url) {
                    Some(dest) => {
                        self.links.push(true);
                        out.push(Event::Start(Tag::Link {
                            link_type: *link_type,
                            dest_url: dest.into(),
                            title: title.clone(),
                            id: id.clone(),
                        }));
                    }
                    None => self.links.push(false),
                },
                Event::End(TagEnd::Link) => {
                    if self.links.pop().unwrap_or(true) {
                        out.push(Event::End(TagEnd::Link));
                    }
                }
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) => out.push(Event::Start(Tag::Image {
                    link_type: *link_type,
                    dest_url: self.image(dest_url).into(),
                    title: title.clone(),
                    id: id.clone(),
                })),
                Event::InlineMath(latex) => {
                    self.math_warning();
                    out.push(Event::InlineHtml(
                        format!("<code class=\"math\">{}</code>", escape(latex)).into(),
                    ));
                }
                Event::DisplayMath(latex) => {
                    self.math_warning();
                    out.push(Event::InlineHtml(
                        format!("<span class=\"math-block\">{}</span>", escape(latex)).into(),
                    ));
                }
                Event::Html(raw) => out.push(Event::Html(self.raw_html(raw).into())),
                Event::InlineHtml(raw) => out.push(Event::InlineHtml(self.raw_html(raw).into())),
                other => out.push(other.clone()),
            }
        }
        (out, first_h1)
    }

    fn math_warning(&mut self) {
        self.warn_once(
            "math",
            "Formeln erscheinen als LaTeX-Code – der HTML-Export kommt ohne Skripte aus.".into(),
        );
    }

    fn callout_open(&self, kind: &str, title: Option<&str>) -> String {
        let heading = title
            .map(escape)
            .or_else(|| callout_label(kind, self.german).map(str::to_string));
        let mut out = format!("<aside class=\"callout callout-{kind}\">\n");
        if let Some(heading) = heading {
            out.push_str(&format!("<p class=\"callout-title\">{heading}</p>\n"));
        }
        out
    }

    /// Linkziel für HTML; `None`: nur der Linktext bleibt stehen.
    fn link(&mut self, dest: &str) -> Option<String> {
        if let Some(anchor) = dest.strip_prefix('#') {
            return Some(format!("#{}", slugify(&percent_decode(anchor))));
        }
        if is_external(dest) {
            return Some(dest.to_string());
        }
        let (path_part, fragment) = match dest.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (dest, None),
        };
        let decoded = percent_decode(path_part);
        let target = normalize(&self.resolve(&decoded));
        if !is_markdown(Path::new(&decoded)) {
            // Verlinkte Dateien (z. B. PDFs) mitnehmen, sofern es sie gibt.
            if target.is_file() {
                self.asset(&target, path_part);
            }
            return Some(dest.to_string());
        }
        if !self.options.pages.contains(&target) {
            self.warn_once(
                &format!("link:{decoded}"),
                format!("Link auf „{decoded}“ wird nicht mitexportiert – als Text übernommen."),
            );
            return None;
        }
        let stem = path_part
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(path_part);
        let fragment = fragment
            .map(|f| format!("#{}", slugify(&percent_decode(f))))
            .unwrap_or_default();
        Some(format!("{stem}.html{fragment}"))
    }

    fn image(&mut self, dest: &str) -> String {
        if is_external(dest) || dest.starts_with("data:") {
            return dest.to_string();
        }
        let decoded = percent_decode(dest);
        let source = normalize(&self.resolve(&decoded));
        if !source.is_file() {
            self.warn_once(
                &format!("missing:{dest}"),
                format!("Bild nicht gefunden: {dest}"),
            );
            return dest.to_string();
        }
        self.asset(&source, dest)
    }

    /// Merkt eine lokale Datei zum Kopieren vor und liefert den Pfad, unter dem die Seite sie
    /// findet. Dateien außerhalb der Wurzel landen in `bilder/` neben der Seite.
    fn asset(&mut self, source: &Path, dest: &str) -> String {
        if let Ok(rel) = source.strip_prefix(self.options.root) {
            self.assets.push(Asset {
                source: source.to_path_buf(),
                target: rel.to_path_buf(),
            });
            return dest.to_string();
        }
        let name = source.file_name().unwrap_or_default();
        self.assets.push(Asset {
            source: source.to_path_buf(),
            target: self.rel_dir.join("bilder").join(name),
        });
        format!("bilder/{}", encode_segment(&name.to_string_lossy()))
    }

    fn resolve(&self, decoded: &str) -> PathBuf {
        match decoded.strip_prefix('/') {
            Some(rooted) => self.options.root.join(rooted),
            None => self.page_dir.join(decoded),
        }
    }

    /// Rohes HTML: Hinweis-Komponenten werden zu Hinweisboxen, unbekannte Komponenten fallen
    /// weg (ihr Inhalt bleibt), Skripte und Event-Handler werden entfernt.
    fn raw_html(&mut self, raw: &str) -> String {
        let mut out = String::new();
        for token in tokenize_html(raw) {
            if let Some(skipped) = &self.skipping {
                if matches!(&token, HtmlToken::Close { name } if name.eq_ignore_ascii_case(skipped))
                {
                    self.skipping = None;
                }
                continue;
            }
            match token {
                HtmlToken::Open {
                    name,
                    attrs,
                    self_closing,
                } => {
                    let attr = |key: &str| {
                        attrs
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(key))
                            .map(|(_, v)| v.as_str())
                    };
                    let variant = attr("type")
                        .or(attr("variant"))
                        .or(attr("kind"))
                        .or(attr("status"));
                    if let Some(kind) = admonition_kind(&name, variant) {
                        out.push_str(&self.callout_open(kind, attr("title")));
                        if self_closing {
                            out.push_str("</aside>\n");
                        }
                        continue;
                    }
                    let lower = name.to_ascii_lowercase();
                    if matches!(
                        lower.as_str(),
                        "script" | "style" | "iframe" | "object" | "embed"
                    ) {
                        self.warn_once(
                            &format!("tag:{lower}"),
                            format!("<{lower}> entfernt – der HTML-Export bleibt ohne Skripte und eingebettete Inhalte."),
                        );
                        if !self_closing {
                            self.skipping = Some(name);
                        }
                        continue;
                    }
                    if !crate::markdown::is_html_element(&name) {
                        self.warn_once(
                            &format!("tag:{name}"),
                            format!("Komponente <{name}> ohne Entsprechung – nur der Inhalt wurde übernommen."),
                        );
                        continue;
                    }
                    out.push('<');
                    out.push_str(&lower);
                    for (key, value) in &attrs {
                        if key.to_ascii_lowercase().starts_with("on") {
                            continue;
                        }
                        let value = match (lower.as_str(), key.to_ascii_lowercase().as_str()) {
                            ("img", "src") => self.image(value),
                            ("a", "href") => self.link(value).unwrap_or_default(),
                            _ => value.clone(),
                        };
                        out.push_str(&format!(" {key}=\"{}\"", escape(&value)));
                    }
                    out.push_str(if self_closing { " />" } else { ">" });
                }
                HtmlToken::Close { name } => {
                    if admonition_kind(&name, None).is_some() {
                        out.push_str("</aside>\n");
                    } else if crate::markdown::is_html_element(&name) {
                        out.push_str(&format!("</{}>", name.to_ascii_lowercase()));
                    }
                }
                HtmlToken::Text(text) => out.push_str(&text),
                HtmlToken::Comment(_) => {}
            }
        }
        out
    }
}

/// Anker je Überschrift (in Reihenfolge), wie sie auch der PDF-Satz vergibt.
fn heading_ids(events: &[Event]) -> Vec<String> {
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut ids = Vec::new();
    for (ix, event) in events.iter().enumerate() {
        let Event::Start(Tag::Heading { id, .. }) = event else {
            continue;
        };
        let base = match id {
            Some(id) => id.to_string(),
            None => slugify(&heading_text(&events[ix..])),
        };
        let base = if base.is_empty() {
            "abschnitt".to_string()
        } else {
            base
        };
        let count = used.entry(base.clone()).or_insert(0);
        ids.push(if *count == 0 {
            base
        } else {
            format!("{base}-{count}")
        });
        *count += 1;
    }
    ids
}

/// Text einer Überschrift; `events[0]` ist ihr Start.
fn heading_text(events: &[Event]) -> String {
    let end = events
        .iter()
        .position(|e| matches!(e, Event::End(TagEnd::Heading(_))))
        .unwrap_or(events.len());
    plain_text(&events[1..end]).trim().to_string()
}

fn is_external(dest: &str) -> bool {
    dest.starts_with("//")
        || dest
            .split_once(':')
            .is_some_and(|(scheme, _)| !scheme.is_empty() && !scheme.contains('/'))
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Pfadsegment für eine URL: Leerzeichen und Sonderzeichen kodiert.
fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for c in segment.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            '%' => out.push_str("%25"),
            '"' => out.push_str("%22"),
            _ => out.push(c),
        }
    }
    out
}

fn encode_path(path: &Path) -> String {
    path.components()
        .map(|c| encode_segment(&c.as_os_str().to_string_lossy()))
        .collect::<Vec<_>>()
        .join("/")
}

/// Übersicht: Seiten nach Ordnern gruppiert.
fn index_page(title: &str, entries: &[(PathBuf, String)]) -> String {
    let mut groups: BTreeMap<PathBuf, Vec<(&PathBuf, &String)>> = BTreeMap::new();
    for (rel, page_title) in entries {
        let dir = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        groups.entry(dir).or_default().push((rel, page_title));
    }
    let mut body = format!("<h1>{}</h1>\n", escape(title));
    for (dir, mut pages) in groups {
        pages.sort_by_key(|(_, title)| title.to_lowercase());
        if !dir.as_os_str().is_empty() {
            let name = dir
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" / ");
            body.push_str(&format!("<h2>{}</h2>\n", escape(&name)));
        }
        body.push_str("<ul class=\"pages\">\n");
        for (rel, page_title) in pages {
            body.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>\n",
                encode_path(rel),
                escape(page_title)
            ));
        }
        body.push_str("</ul>\n");
    }
    format!(
        "<!doctype html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"DevMark\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"{STYLESHEET}\">\n</head>\n<body>\n<main>\n{body}</main>\n</body>\n</html>\n",
        escape(title)
    )
}

/// Stylesheet in den Farben der PDF-Vorlage.
fn stylesheet() -> String {
    format!("{STYLESHEET_MARKER}\n{}", include_str!("html.css"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(source: &str, pages: &[&str]) -> Page {
        let root = Path::new("/notizen");
        let pages: HashSet<PathBuf> = pages.iter().map(|p| root.join(p)).collect();
        markdown_to_html(
            source,
            &PageOptions {
                path: &root.join("Projekt/plan.md"),
                root,
                pages: &pages,
                with_index: true,
            },
        )
    }

    #[test]
    fn note_links_become_html_links() {
        let page = page(
            "# Plan\n\n[Text](../text.md#Mein%20Abschnitt), [Weg](fehlt.md), [Web](https://example.com)\n",
            &["text.md", "Projekt/plan.md"],
        );
        assert!(
            page.html
                .contains("<a href=\"../text.html#mein-abschnitt\">Text</a>"),
            "{}",
            page.html
        );
        assert!(page.html.contains(", Weg, "));
        assert!(page.html.contains("href=\"https://example.com\""));
        assert!(page.warnings[0].message.contains("fehlt.md"));
        assert!(page.html.contains("href=\"../style.css\""));
        assert!(
            page.html
                .contains("<a href=\"../index.html\">Übersicht</a>")
        );
        assert_eq!(page.title, "Plan");
    }

    #[test]
    fn callouts_like_in_the_pdf() {
        let page = page(
            "> [!NOTE]\n> Eine Box.\n\n:::tip Mein Titel\nInhalt\n:::\n",
            &[],
        );
        assert!(page.html.contains(
            "<aside class=\"callout callout-note\">\n<p class=\"callout-title\">Hinweis</p>"
        ));
        assert!(page.html.contains(
            "<aside class=\"callout callout-tip\">\n<p class=\"callout-title\">Mein Titel</p>"
        ));
        assert_eq!(page.html.matches("</aside>").count(), 2);
    }

    #[test]
    fn headings_get_anchors_and_scripts_are_removed() {
        let page = page(
            "# Titel\n\n## Zwei Wörter\n\n## Zwei Wörter\n\n[hin](#zwei-wörter)\n\n<script>alert(1)</script>\n\n<b onclick=\"x()\">fett</b>\n",
            &[],
        );
        assert!(page.html.contains("<h2 id=\"zwei-wörter\">"));
        assert!(page.html.contains("<h2 id=\"zwei-wörter-1\">"));
        // Der Browser dekodiert das Fragment und findet die Überschrift.
        assert!(
            page.html.contains("href=\"#zwei-w%C3%B6rter\""),
            "{}",
            page.html
        );
        assert!(!page.html.contains("script>"), "{}", page.html);
        assert!(!page.html.contains("alert"));
        assert!(page.html.contains("<b>fett</b>"));
    }

    #[test]
    fn frontmatter_title_and_math() {
        let page = page(
            "---\ntitle: Aus Frontmatter\nlang: en\n---\nText $x^2$\n",
            &[],
        );
        assert_eq!(page.title, "Aus Frontmatter");
        assert!(page.html.contains("<html lang=\"en\">"));
        assert!(page.html.contains("<h1>Aus Frontmatter</h1>"));
        assert!(page.html.contains("<code class=\"math\">x^2</code>"));
        assert!(page.html.contains(">Overview</a>"));
    }

    #[test]
    fn exports_a_folder() {
        let dir = std::env::temp_dir().join(format!("devmark-html-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("Notizen");
        std::fs::create_dir_all(src.join("Projekt")).unwrap();
        std::fs::create_dir_all(src.join(".trash")).unwrap();
        std::fs::write(
            src.join("Erste Notiz.md"),
            "# Erste\n\n[Plan](Projekt/plan.md)\n\n![Bild](bild.png)\n",
        )
        .unwrap();
        std::fs::write(src.join("bild.png"), b"png").unwrap();
        std::fs::write(
            src.join("Projekt/plan.md"),
            "# Plan\n\n[Zurück](../Erste%20Notiz.md)\n",
        )
        .unwrap();
        std::fs::write(src.join(".trash/alt.md"), "# Alt\n").unwrap();

        let out = dir.join("Export");
        let sources = collect_markdown(&src);
        assert_eq!(sources.len(), 2);
        let export = export_site(&src, &sources, &out).unwrap();
        assert_eq!(export.pages, 2);
        assert_eq!(export.entry, out.join("index.html"));
        let first = std::fs::read_to_string(out.join("Erste Notiz.html")).unwrap();
        assert!(first.contains("href=\"Projekt/plan.html\""));
        let plan = std::fs::read_to_string(out.join("Projekt/plan.html")).unwrap();
        assert!(plan.contains("href=\"../Erste%20Notiz.html\""));
        assert!(plan.contains("href=\"../style.css\""));
        assert!(out.join("bild.png").is_file());
        let index = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(index.contains("<a href=\"Erste%20Notiz.html\">Erste</a>"));
        assert!(index.contains("<h2>Projekt</h2>"));
        assert!(!out.join(".trash").exists());
        assert!(
            std::fs::read_to_string(out.join(STYLESHEET))
                .unwrap()
                .starts_with(STYLESHEET_MARKER)
        );

        let single = dir.join("Einzeln");
        let export = export_document(
            "# Neu\n\n[Plan](Projekt/plan.md)\n",
            &src.join("Erste Notiz.md"),
            &single,
        )
        .unwrap();
        assert_eq!(export.entry, single.join("Erste Notiz.html"));
        let text = std::fs::read_to_string(&export.entry).unwrap();
        assert!(text.contains("<h1 id=\"neu\">Neu</h1>"), "{text}");
        assert!(!text.contains("plan.html") && !text.contains("<nav>"));

        // Erneuter Export in denselben Ordner ist erlaubt, ein fremder Ordner nicht.
        export_site(&src, &sources, &out).unwrap();
        assert!(export_site(&src, &sources, &src).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
