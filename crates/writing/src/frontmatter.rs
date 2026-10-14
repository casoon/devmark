//! Minimaler Frontmatter-Leser (YAML-Teilmenge bzw. TOML-Schlüssel) für Dokument-Metadaten.
//!
//! Unterstützt nur das, was Markdown-/MDX-Dateien typischerweise enthalten:
//! `key: wert`, `key: "wert"`, `key: [a, b]` und Listen mit `- eintrag`.
//! Verschachtelte Strukturen werden übersprungen.

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub date: Option<String>,
    pub lang: Option<String>,
    pub keywords: Vec<String>,
    pub toc: Option<bool>,
    pub numbering: Option<bool>,
    pub paper: Option<String>,
}

impl Metadata {
    pub fn parse_yaml(text: &str) -> Self {
        Self::from_pairs(parse_pairs(text, ':'))
    }

    pub fn parse_toml(text: &str) -> Self {
        Self::from_pairs(parse_pairs(text, '='))
    }

    fn from_pairs(pairs: Vec<(String, Value)>) -> Self {
        let mut meta = Self::default();
        for (key, value) in pairs {
            match key.to_ascii_lowercase().as_str() {
                "title" => meta.title = value.scalar(),
                "subtitle" | "description" => {
                    if meta.subtitle.is_none() {
                        meta.subtitle = value.scalar();
                    }
                }
                "author" | "authors" => meta.authors = value.list(),
                "date" => meta.date = value.scalar(),
                "lang" | "language" | "locale" => meta.lang = value.scalar(),
                "tags" | "keywords" => meta.keywords = value.list(),
                "toc" => meta.toc = value.bool(),
                "numbering" | "number_sections" | "number-sections" => {
                    meta.numbering = value.bool()
                }
                "paper" | "papersize" => meta.paper = value.scalar(),
                _ => {}
            }
        }
        meta
    }
}

#[derive(Debug)]
enum Value {
    Scalar(String),
    List(Vec<String>),
}

impl Value {
    fn scalar(self) -> Option<String> {
        match self {
            Value::Scalar(s) if !s.is_empty() => Some(s),
            Value::List(items) => items.into_iter().next(),
            _ => None,
        }
    }

    fn list(self) -> Vec<String> {
        match self {
            Value::Scalar(s) if s.is_empty() => Vec::new(),
            // „a, b“ als Komma-Liste nur bei Schlüsselwörtern sinnvoll; Autoren bleiben ganz.
            Value::Scalar(s) => vec![s],
            Value::List(items) => items,
        }
    }

    fn bool(self) -> Option<bool> {
        match self.scalar()?.to_ascii_lowercase().as_str() {
            "true" | "yes" | "on" => Some(true),
            "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
}

fn parse_pairs(text: &str, separator: char) -> Vec<(String, Value)> {
    let mut pairs: Vec<(String, Value)> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let trimmed = line.trim();
        if indented {
            // Listeneintrag zum zuletzt gelesenen Schlüssel.
            if let Some(item) = trimmed.strip_prefix("- ").or(trimmed.strip_prefix('-').filter(|s| s.is_empty()))
                && let Some((_, value)) = pairs.last_mut() {
                    let item = unquote(item.trim());
                    match value {
                        Value::List(items) => items.push(item),
                        Value::Scalar(s) if s.is_empty() => *value = Value::List(vec![item]),
                        Value::Scalar(_) => {}
                    }
                }
            continue;
        }
        let Some((key, value)) = trimmed.split_once(separator) else {
            continue;
        };
        let key = key.trim().trim_matches('"').to_string();
        let value = value.trim();
        let value = if value.starts_with('[') && value.ends_with(']') {
            Value::List(
                value[1..value.len() - 1]
                    .split(',')
                    .map(|item| unquote(item.trim()))
                    .filter(|item| !item.is_empty())
                    .collect(),
            )
        } else {
            Value::Scalar(unquote(value))
        };
        pairs.push((key, value));
    }
    pairs
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    // Kommentar am Zeilenende (nur bei ungequoteten Werten).
    let value = if !value.starts_with('"') && !value.starts_with('\'') {
        value.split(" #").next().unwrap_or(value).trim()
    } else {
        value
    };
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].replace("\\\"", "\"");
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_and_lists() {
        let meta = Metadata::parse_yaml(
            "title: \"Hallo: Welt\"\nauthor:\n  - Ada\n  - Bob\ntags: [rust, typst]\ntoc: true\nlang: de # Kommentar\nnested:\n  key: x\n",
        );
        assert_eq!(meta.title.as_deref(), Some("Hallo: Welt"));
        assert_eq!(meta.authors, ["Ada", "Bob"]);
        assert_eq!(meta.keywords, ["rust", "typst"]);
        assert_eq!(meta.toc, Some(true));
        assert_eq!(meta.lang.as_deref(), Some("de"));
    }

    #[test]
    fn parses_toml() {
        let meta = Metadata::parse_toml("title = \"Buch\"\ndate = 2026-10-14\n");
        assert_eq!(meta.title.as_deref(), Some("Buch"));
        assert_eq!(meta.date.as_deref(), Some("2026-10-14"));
    }
}
