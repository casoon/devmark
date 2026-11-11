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
use crate::frontmatter::Metadata;
use crate::markdown::{
    HtmlToken, admonition_kind, callout_kind, callout_label, contrast_on_white, normalize,
    parse_hex_color, parser_options, percent_decode, plain_text, preprocess_directives,
    preprocess_mdx, slugify, split_frontmatter, tokenize_html,
};

/// Name des gemeinsamen Stylesheets in der Ausgabewurzel.
pub const STYLESHEET: &str = "style.css";
/// Erste Zeile des Stylesheets – kennzeichnet einen Ordner als DevMark-Export.
const STYLESHEET_MARKER: &str = "/* DevMark HTML-Export */";
/// Einstellungen eines Ordner-Exports, relativ zur Exportwurzel (TOML: `title`, `lang`,
/// `accent`, `base_url`).
pub const SETTINGS_FILE: &str = ".devmark/export.toml";
/// Länge, auf die eine aus dem ersten Absatz gewonnene Beschreibung gekürzt wird.
const DESCRIPTION_LEN: usize = 160;

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
    /// Vorgaben des Exports; das Frontmatter der Seite geht vor.
    pub site: &'a Metadata,
}

pub struct Page {
    pub title: String,
    pub html: String,
    pub warnings: Vec<Warning>,
    /// Lokale Bilder und Dateien, die neben die Seiten kopiert werden müssen.
    pub assets: Vec<Asset>,
    /// Inhalt von `<main>`.
    content: String,
    lang: String,
    /// `description`/`subtitle` aus dem Frontmatter, sonst der Anfang des ersten Absatzes.
    description: Option<String>,
    /// Akzentfarbe als `#rrggbb`.
    accent: Option<String>,
    /// Ordnertiefe der Seite unter der Exportwurzel.
    depth: usize,
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
    let lang = page_lang(metadata.lang.as_deref().or(options.site.lang.as_deref()));

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
    let description = metadata
        .subtitle
        .clone()
        .or_else(|| first_paragraph(&events));
    let accent = metadata
        .accent
        .as_deref()
        .or(options.site.accent.as_deref())
        .and_then(|value| writer.accent(value));

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
    let mut page = Page {
        title,
        html: String::new(),
        warnings: writer.warnings,
        assets: writer.assets,
        content,
        lang,
        description,
        accent,
        depth: writer.rel_dir.components().count(),
    };
    let nav = if options.with_index {
        index_link(&page)
    } else {
        String::new()
    };
    page.html = document(&page, &nav, None);
    page
}

fn page_lang(lang: Option<&str>) -> String {
    lang.and_then(|lang| lang.split(['-', '_']).next())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "de".into())
}

/// Anfang des ersten Absatzes als Beschreibung, an einer Wortgrenze gekürzt.
fn first_paragraph(events: &[Event]) -> Option<String> {
    let start = events
        .iter()
        .position(|e| matches!(e, Event::Start(Tag::Paragraph)))?;
    let end = events[start..]
        .iter()
        .position(|e| matches!(e, Event::End(TagEnd::Paragraph)))?;
    let text = plain_text(&events[start..start + end]);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= DESCRIPTION_LEN {
        return Some(text);
    }
    let cut: String = text.chars().take(DESCRIPTION_LEN).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    Some(format!("{}…", cut.trim_end_matches([',', ';', ':', '.'])))
}

fn is_german(lang: &str) -> bool {
    lang == "de"
}

/// Kopfzeile einer Seite mit Link zurück zur Übersicht.
fn index_link(page: &Page) -> String {
    let label = if is_german(&page.lang) {
        "Übersicht"
    } else {
        "Overview"
    };
    format!(
        "<nav class=\"top\"><a href=\"{}index.html\">{label}</a></nav>\n",
        "../".repeat(page.depth)
    )
}

/// Vollständige HTML-Datei. `url`: öffentliche Adresse der Seite (mit `base_url`).
fn document(page: &Page, nav: &str, url: Option<&str>) -> String {
    let prefix = "../".repeat(page.depth);
    let title = escape(&page.title);
    let mut head = String::new();
    if let Some(description) = &page.description {
        head.push_str(&format!(
            "<meta name=\"description\" content=\"{}\">\n",
            escape(description)
        ));
    }
    if let Some(url) = url {
        head.push_str(&format!(
            "<link rel=\"canonical\" href=\"{url}\">\n<meta property=\"og:type\" content=\"article\">\n<meta property=\"og:url\" content=\"{url}\">\n<meta property=\"og:title\" content=\"{title}\">\n"
        ));
        if let Some(description) = &page.description {
            head.push_str(&format!(
                "<meta property=\"og:description\" content=\"{}\">\n",
                escape(description)
            ));
        }
    }
    if let Some(accent) = &page.accent {
        head.push_str(&format!("<style>:root {{ --brand: {accent}; }}</style>\n"));
    }
    // The page navigation follows the text, so it comes after it for keyboard and screen
    // readers; the stylesheet places it as a sidebar on wide screens.
    let jump;
    let (layout, top, bottom) = if nav.contains("class=\"site\"") {
        let label = if is_german(&page.lang) {
            "Alle Seiten ↓"
        } else {
            "All pages ↓"
        };
        jump = format!("<nav class=\"top jump\"><a href=\"#pages\">{label}</a></nav>\n");
        (" class=\"with-nav\"", jump.as_str(), nav)
    } else {
        ("", nav, "")
    };
    format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"DevMark\">\n<title>{title}</title>\n{head}<link rel=\"stylesheet\" href=\"{prefix}{STYLESHEET}\">\n</head>\n<body{layout}>\n{top}<main>\n{content}</main>\n{bottom}</body>\n</html>\n",
        lang = page.lang,
        content = page.content,
    )
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

    let site = read_settings(&root);
    let base_url = site
        .base_url
        .as_deref()
        .map(|url| format!("{}/", url.trim().trim_end_matches('/')));

    std::fs::create_dir_all(out_dir)?;
    let mut warnings = Vec::new();
    let mut copied = HashSet::new();
    let mut rendered = Vec::new();
    for source in &sources {
        let text = read(source)?;
        let page = markdown_to_html(
            &text,
            &PageOptions {
                path: source,
                root: &root,
                pages: &pages,
                with_index,
                site: &site,
            },
        );
        let rel = source
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| PathBuf::from(source.file_name().unwrap_or_default()))
            .with_extension("html");
        rendered.push((source.clone(), rel, page));
    }

    let entries: Vec<(PathBuf, String)> = rendered
        .iter()
        .map(|(_, rel, page)| (rel.clone(), page.title.clone()))
        .collect();
    let site_title = site.title.clone().unwrap_or_else(|| {
        root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let multi = entries.len() > 1;
    for (source, rel, page) in rendered {
        let html = if multi {
            let nav = site_nav(&page, rel.as_path(), &site_title, &entries);
            let url = base_url
                .as_ref()
                .map(|base| format!("{base}{}", encode_path(&rel)));
            document(&page, &nav, url.as_deref())
        } else {
            page.html.clone()
        };
        let target = out_dir.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, html)?;
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
    }
    std::fs::write(out_dir.join(STYLESHEET), stylesheet())?;
    if with_index {
        std::fs::write(
            out_dir.join("index.html"),
            index_page(&site_title, &site, &entries, base_url.as_deref()),
        )?;
    }
    if let Some(base) = &base_url {
        std::fs::write(
            out_dir.join("sitemap.xml"),
            sitemap(base, &entries, with_index),
        )?;
    }
    let entry = if with_index || has_index {
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

/// Liest `.devmark/export.toml` der Exportwurzel; fehlt die Datei, gelten die Vorgaben.
fn read_settings(root: &Path) -> Metadata {
    std::fs::read_to_string(root.join(SETTINGS_FILE))
        .map(|text| Metadata::parse_toml(&text))
        .unwrap_or_default()
}

/// Navigation über alle Seiten, nach Ordnern gruppiert; die aktuelle Seite ist markiert.
fn site_nav(
    page: &Page,
    current: &Path,
    site_title: &str,
    entries: &[(PathBuf, String)],
) -> String {
    let prefix = "../".repeat(page.depth);
    let german = is_german(&page.lang);
    let label = if german { "Seiten" } else { "Pages" };
    let mut out = format!(
        "<nav class=\"site\" id=\"pages\" aria-label=\"{label}\">\n<p class=\"site-title\"><a href=\"{prefix}index.html\">{}</a></p>\n",
        escape(site_title)
    );
    for (dir, pages) in grouped(entries) {
        if !dir.as_os_str().is_empty() {
            out.push_str(&format!(
                "<p class=\"group\">{}</p>\n",
                escape(&folder_label(&dir))
            ));
        }
        out.push_str("<ul>\n");
        for (rel, title) in pages {
            let current = if rel.as_path() == current {
                " aria-current=\"page\""
            } else {
                ""
            };
            out.push_str(&format!(
                "<li><a href=\"{prefix}{}\"{current}>{}</a></li>\n",
                encode_path(rel),
                escape(title)
            ));
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</nav>\n");
    out
}

/// Seiten nach Ordner, Ordner und Titel alphabetisch.
fn grouped(entries: &[(PathBuf, String)]) -> BTreeMap<PathBuf, Vec<(&PathBuf, &String)>> {
    let mut groups: BTreeMap<PathBuf, Vec<(&PathBuf, &String)>> = BTreeMap::new();
    for (rel, title) in entries {
        let dir = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        groups.entry(dir).or_default().push((rel, title));
    }
    for pages in groups.values_mut() {
        pages.sort_by_key(|(_, title)| title.to_lowercase());
    }
    groups
}

fn folder_label(dir: &Path) -> String {
    dir.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// `sitemap.xml` mit allen Seiten (und der Übersicht) unter `base`.
fn sitemap(base: &str, entries: &[(PathBuf, String)], with_index: bool) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    if with_index {
        out.push_str(&format!(
            "  <url><loc>{}index.html</loc></url>\n",
            escape(base)
        ));
    }
    for (rel, _) in entries {
        out.push_str(&format!(
            "  <url><loc>{}{}</loc></url>\n",
            escape(base),
            encode_path(rel)
        ));
    }
    out.push_str("</urlset>\n");
    out
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

    /// Prüft eine Akzentfarbe; ungültige fallen mit Hinweis weg.
    fn accent(&mut self, value: &str) -> Option<String> {
        let Some(rgb) = parse_hex_color(value) else {
            self.warn_once(
                "accent",
                format!(
                    "Akzentfarbe „{value}“ nicht erkannt (erwartet z. B. \"#0a7f6f\") – ignoriert."
                ),
            );
            return None;
        };
        let contrast = contrast_on_white(rgb);
        if contrast < 4.5 {
            let ratio = format!("{contrast:.1}").replace('.', ",");
            self.warn_once(
                "accent",
                format!("Akzentfarbe „{value}“ hat zu wenig Kontrast zu Weiß ({ratio}:1, nötig sind 4,5:1)."),
            );
        }
        Some(format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2))
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
fn index_page(
    title: &str,
    site: &Metadata,
    entries: &[(PathBuf, String)],
    base_url: Option<&str>,
) -> String {
    let mut content = format!("<h1>{}</h1>\n", escape(title));
    for (dir, pages) in grouped(entries) {
        if !dir.as_os_str().is_empty() {
            content.push_str(&format!("<h2>{}</h2>\n", escape(&folder_label(&dir))));
        }
        content.push_str("<ul class=\"pages\">\n");
        for (rel, page_title) in pages {
            content.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>\n",
                encode_path(rel),
                escape(page_title)
            ));
        }
        content.push_str("</ul>\n");
    }
    let page = Page {
        title: title.to_string(),
        html: String::new(),
        warnings: Vec::new(),
        assets: Vec::new(),
        content,
        lang: page_lang(site.lang.as_deref()),
        description: site.subtitle.clone(),
        accent: site
            .accent
            .as_deref()
            .and_then(parse_hex_color)
            .map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}")),
        depth: 0,
    };
    let url = base_url.map(|base| format!("{base}index.html"));
    document(&page, "", url.as_deref())
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
                site: &Metadata::default(),
            },
        )
    }

    #[test]
    fn accent_and_description() {
        let page = page(
            "---\naccent: \"#0F766E\"\n---\n# Titel\n\nErster Absatz mit **fettem** Text.\n\nZweiter.\n",
            &[],
        );
        assert!(
            page.html
                .contains("<style>:root { --brand: #0f766e; }</style>")
        );
        assert!(
            page.html
                .contains("<meta name=\"description\" content=\"Erster Absatz mit fettem Text.\">"),
            "{}",
            page.html
        );
        assert!(page.warnings.is_empty());

        let page = super::tests::page(
            "---\naccent: \"#ffd700\"\ndescription: Kurz\n---\nText\n",
            &[],
        );
        assert!(page.html.contains("content=\"Kurz\""));
        assert!(page.warnings[0].message.contains("Kontrast"));
        let page = super::tests::page("---\naccent: blau\n---\nText\n", &[]);
        assert!(!page.html.contains("--brand"));
        assert!(page.warnings[0].message.contains("nicht erkannt"));
    }

    #[test]
    fn long_description_is_cut_at_a_word() {
        let text = "Wort ".repeat(60);
        let description =
            first_paragraph(&Parser::new_ext(&text, parser_options()).collect::<Vec<_>>()).unwrap();
        assert!(description.ends_with("Wort…"), "{description}");
        assert!(description.chars().count() <= DESCRIPTION_LEN + 1);
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

        std::fs::create_dir_all(src.join(".devmark")).unwrap();
        std::fs::write(
            src.join(SETTINGS_FILE),
            "title = \"Handbuch\"\nbase_url = \"https://example.com/notes\"\naccent = \"#0f766e\"\n",
        )
        .unwrap();

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
        // Navigation über alle Seiten, aktuelle Seite markiert, nach dem Text.
        assert!(plan.contains("<body class=\"with-nav\">"));
        assert!(
            plan.contains("<a href=\"../index.html\">Handbuch</a>"),
            "{plan}"
        );
        assert!(plan.contains("<a href=\"../Projekt/plan.html\" aria-current=\"page\">Plan</a>"));
        assert!(plan.contains("<a href=\"../Erste%20Notiz.html\">Erste</a>"));
        assert!(plan.find("</main>") < plan.find("<nav class=\"site\""));
        assert!(plan.contains(
            "<link rel=\"canonical\" href=\"https://example.com/notes/Projekt/plan.html\">"
        ));
        assert!(plan.contains("--brand: #0f766e"));
        let sitemap = std::fs::read_to_string(out.join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://example.com/notes/index.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/notes/Erste%20Notiz.html</loc>"));
        assert!(out.join("bild.png").is_file());
        let index = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(index.contains("<a href=\"Erste%20Notiz.html\">Erste</a>"));
        assert!(index.contains("<h2>Projekt</h2>"));
        assert!(index.contains("<h1>Handbuch</h1>"));
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
