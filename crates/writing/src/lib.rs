//! Schreib-Werkzeuge ohne UI-Abhängigkeit: Markdown/MDX → Typst, Typst → PDF,
//! Formatieraktionen und Dokumentvorlagen.

pub mod authoring;
pub mod frontmatter;
pub mod markdown;
pub mod math;
pub mod pdf;
pub mod templates;

pub use markdown::{Conversion, Options, markdown_to_typst};
pub use pdf::{CompileError, Compiled, Rendered, RenderedPage, compile_pdf, render_pages};

/// Hinweis beim Umwandeln oder Setzen: etwas ging verloren oder wurde ersetzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Zeile (ab 1) in der Quelldatei, sofern bekannt.
    pub line: Option<usize>,
    pub message: String,
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "Zeile {line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}
