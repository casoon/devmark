//! Schreib-Werkzeuge ohne UI-Abhängigkeit: Markdown/MDX → Typst, Typst → PDF,
//! Formatieraktionen und Dokumentvorlagen.

pub mod authoring;
pub mod frontmatter;
pub mod markdown;
pub mod math;
pub mod pdf;
pub mod templates;

pub use markdown::{Conversion, Options, markdown_to_typst};
pub use pdf::{CompileError, Compiled, compile_pdf};
