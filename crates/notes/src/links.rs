//! Verknüpfungen zwischen Notizen: `[[Titel|id]]`. Die ID ist maßgeblich – der Titel ist nur
//! Anzeigetext, Umbenennen bricht also keinen Link. `[[Titel]]` ohne ID wird über den Titel
//! aufgelöst.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLink {
    /// Byte-Bereich von `[[` bis einschließlich `]]`.
    pub range: Range<usize>,
    pub title: String,
    pub id: Option<String>,
}

/// Alle `[[…]]`-Verknüpfungen im Text (nicht über Zeilenumbrüche hinweg).
pub fn note_links(text: &str) -> Vec<NoteLink> {
    let mut links = Vec::new();
    let mut from = 0;
    while let Some(start) = text[from..].find("[[").map(|i| from + i) {
        let inner_start = start + 2;
        let Some(len) = text[inner_start..].find("]]") else {
            break;
        };
        let inner = &text[inner_start..inner_start + len];
        if inner.contains(['\n', '[']) {
            from = inner_start;
            continue;
        }
        let end = inner_start + len + 2;
        let (title, id) = match inner.rsplit_once('|') {
            Some((title, id)) => (title, Some(id.trim().to_string()).filter(|id| !id.is_empty())),
            None => (inner, None),
        };
        if !title.trim().is_empty() || id.is_some() {
            links.push(NoteLink { range: start..end, title: title.trim().to_string(), id });
        }
        from = end;
    }
    links
}

/// `[[Titel|id]]`; Zeichen, die das Format stören würden, werden im Titel ersetzt.
pub fn link_markup(title: &str, id: &str) -> String {
    let title = title.replace('|', "/").replace(['[', ']'], "");
    format!("[[{}|{id}]]", title.trim())
}

/// Text mit Verknüpfungen nur als Titel (Vorschau, Export).
pub fn strip_note_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for link in note_links(text) {
        out.push_str(&text[last..link.range.start]);
        out.push_str(&link.title);
        last = link.range.end;
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_links_with_and_without_id() {
        let text = "Siehe [[Einkauf|abc-1]] und [[Größe]]; nicht [[\n]] oder [[]].";
        let links = note_links(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].title, "Einkauf");
        assert_eq!(links[0].id.as_deref(), Some("abc-1"));
        assert_eq!(&text[links[0].range.clone()], "[[Einkauf|abc-1]]");
        assert_eq!(links[1].title, "Größe");
        assert_eq!(links[1].id, None);
    }

    #[test]
    fn markup_and_strip() {
        assert_eq!(link_markup(" A|B [x] ", "id"), "[[A/B x|id]]");
        assert_eq!(strip_note_links("vor [[A|1]] nach [[B]]"), "vor A nach B");
    }
}
