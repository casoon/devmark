//! Verknüpfungen zwischen Notizen als normale Markdown-Links mit relativem Pfad:
//! `[Titel](Ordner/Notiz%20A.md)`. So funktionieren sie auch auf GitHub und in anderen
//! Editoren. Notiz-IDs sind Pfade relativ zum Vault mit `/` als Trenner.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLink {
    /// Byte-Bereich von `[` bis einschließlich `)`.
    pub range: Range<usize>,
    pub text: String,
    /// Byte-Bereich des Ziels (bei `<…>` samt spitzen Klammern).
    pub dest_range: Range<usize>,
    /// Ziel wie geschrieben, dekodiert, ohne `#Anker`.
    pub target: String,
    /// `#Anker` (roh) oder leer.
    pub anchor: String,
}

/// Alle Links auf Markdown-Dateien (relativ, keine URLs, keine Bilder, nicht in Code).
pub fn note_links(text: &str) -> Vec<NoteLink> {
    let mut links = Vec::new();
    let mut in_fence = false;
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            links.extend(line_links(line, line_start));
        }
        line_start += line.len();
    }
    links
}

fn line_links(line: &str, offset: usize) -> Vec<NoteLink> {
    let bytes = line.as_bytes();
    let mut links = Vec::new();
    let mut in_code = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'`' => in_code = !in_code,
            b'\\' => i += 1,
            b'[' if !in_code && (i == 0 || bytes[i - 1] != b'!') => {
                if let Some(link) = parse_link(line, i) {
                    i = link.range.end;
                    if is_note_target(&link.target) {
                        links.push(NoteLink {
                            range: link.range.start + offset..link.range.end + offset,
                            dest_range: link.dest_range.start + offset
                                ..link.dest_range.end + offset,
                            ..link
                        });
                    }
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    links
}

/// `[text](ziel "titel")` ab `start` (Position von `[`).
fn parse_link(line: &str, start: usize) -> Option<NoteLink> {
    let close = start + 1 + line[start + 1..].find(']')?;
    let text = &line[start + 1..close];
    if text.contains('[') || !line[close + 1..].starts_with('(') {
        return None;
    }
    let dest_start = close + 2;
    let rest = &line[dest_start..];
    let (raw, dest_end) = if let Some(inner) = rest.strip_prefix('<') {
        let end = inner.find('>')?;
        (&inner[..end], dest_start + end + 2)
    } else {
        let end = rest.find([' ', ')', '\t']).unwrap_or(rest.len());
        (&rest[..end], dest_start + end)
    };
    let end = dest_end + line[dest_end..].find(')')? + 1;
    let (path, anchor) = match raw.find('#') {
        Some(ix) => (&raw[..ix], &raw[ix..]),
        None => (raw, ""),
    };
    Some(NoteLink {
        range: start..end,
        text: text.to_string(),
        dest_range: dest_start..dest_end,
        target: percent_decode(path),
        anchor: anchor.to_string(),
    })
}

fn is_note_target(target: &str) -> bool {
    !target.is_empty()
        && !target.contains("://")
        && !target.starts_with(['/', '#'])
        && !target.starts_with("mailto:")
        && target.to_lowercase().ends_with(".md")
}

/// Ziel eines Links aus `source` (Notiz-ID) → Notiz-ID; `None`, wenn es den Vault verlässt.
pub fn resolve(source: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = source.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// Relativer, kodierter Linkpfad von Notiz `from` zu Notiz `to`.
pub fn relative(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from
        .split('/')
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, d)| d.to_vec())
        .unwrap_or_default();
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> = vec!["..".into(); from_dir.len() - common];
    parts.extend(to_parts[common..].iter().map(|p| percent_encode(p)));
    parts.join("/")
}

/// `[Titel](relativer/pfad.md)` von Notiz `from` auf Notiz `to`.
pub fn link_markup(title: &str, from: &str, to: &str) -> String {
    let title = title.replace(['[', ']'], "");
    format!("[{}]({})", title.trim(), relative(from, to))
}

/// Text mit Verknüpfungen nur als Linktext (Vorschau, Druck).
pub fn strip_note_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for link in note_links(text) {
        out.push_str(&text[last..link.range.start]);
        out.push_str(&link.text);
        last = link.range.end;
    }
    out.push_str(&text[last..]);
    out
}

/// Links nach Umbenennen/Verschieben anpassen. `old_source`/`new_source`: Pfad der Notiz
/// selbst vorher/nachher; `moved` liefert für eine verschobene Notiz-ID die neue. Linktext,
/// der dem alten Titel entspricht, bekommt den neuen. `None`, wenn sich nichts ändert.
pub fn rewrite_links(
    content: &str,
    old_source: &str,
    new_source: &str,
    moved: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut last = 0;
    let mut changed = false;
    for link in note_links(content) {
        let Some(old_target) = resolve(old_source, &link.target) else {
            continue;
        };
        let new_target = moved(&old_target);
        if new_target.is_none() && old_source == new_source {
            continue;
        }
        let new_target = new_target.unwrap_or_else(|| old_target.clone());
        let text = if link.text == stem(&old_target) {
            stem(&new_target).to_string()
        } else {
            link.text.clone()
        };
        out.push_str(&content[last..link.range.start]);
        out.push('[');
        out.push_str(&text);
        out.push_str(&content[link.range.start + 1 + link.text.len()..link.dest_range.start]);
        out.push_str(&relative(new_source, &new_target));
        out.push_str(&link.anchor);
        out.push_str(&content[link.dest_range.end..link.range.end]);
        last = link.range.end;
        changed = true;
    }
    changed.then(|| {
        out.push_str(&content[last..]);
        out
    })
}

/// Dateiname ohne `.md`.
pub fn stem(id: &str) -> &str {
    let name = id.rsplit('/').next().unwrap_or(id);
    name.strip_suffix(".md").unwrap_or(name)
}

fn percent_encode(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for c in part.chars() {
        match c {
            ' ' | '%' | '(' | ')' | '<' | '>' | '[' | ']' | '#' | '?' => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(byte) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_links_only() {
        let text = "Siehe [Ein Kauf](Ein%20Kauf.md) und [B](<Ordner/B c.md#teil>), \
                    nicht [Web](https://x.md), ![Bild](a.md), `[Code](c.md)`, [Pdf](a.pdf)\n\
                    ```\n[Zaun](z.md)\n```\n";
        let links = note_links(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "Ein Kauf.md");
        assert_eq!(&text[links[0].range.clone()], "[Ein Kauf](Ein%20Kauf.md)");
        assert_eq!(links[1].target, "Ordner/B c.md");
        assert_eq!(links[1].anchor, "#teil");
        assert_eq!(&text[links[1].dest_range.clone()], "<Ordner/B c.md#teil>");
    }

    #[test]
    fn resolves_and_relativizes() {
        assert_eq!(
            resolve("a/b/x.md", "../c/y.md").as_deref(),
            Some("a/c/y.md")
        );
        assert_eq!(resolve("x.md", "../y.md"), None);
        assert_eq!(relative("a/b/x.md", "a/c/y z.md"), "../c/y%20z.md");
        assert_eq!(relative("x.md", "a/y.md"), "a/y.md");
        assert_eq!(relative("a/x.md", "y.md"), "../y.md");
        assert_eq!(link_markup("Y [1]", "a/x.md", "a/y.md"), "[Y 1](y.md)");
    }

    #[test]
    fn rewrites_moved_targets_and_sources() {
        let content = "[Alt](Alt.md) und [anders](Alt.md#k) und [C](sub/C.md)";
        let moved = |id: &str| (id == "Alt.md").then(|| "Neu.md".to_string());
        assert_eq!(
            rewrite_links(content, "x.md", "x.md", moved).unwrap(),
            "[Neu](Neu.md) und [anders](Neu.md#k) und [C](sub/C.md)"
        );
        // Quelle selbst verschoben: alle Pfade relativ zur neuen Lage.
        assert_eq!(
            rewrite_links("[C](sub/C.md)", "x.md", "sub/x.md", |_| None).unwrap(),
            "[C](C.md)"
        );
        assert_eq!(
            rewrite_links("[C](sub/C.md)", "x.md", "x.md", |_| None),
            None
        );
    }

    #[test]
    fn strip() {
        assert_eq!(
            strip_note_links("vor [A](A.md) nach [W](https://w)"),
            "vor A nach [W](https://w)"
        );
    }
}
