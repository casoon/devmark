//! Schreib-Werkzeuge ohne UI-Abhängigkeit: Markdown/MDX → Typst, Typst → PDF.

pub mod frontmatter;
pub mod markdown;
pub mod math;
pub mod pdf;

pub use markdown::{Conversion, Options, markdown_to_typst};
pub use pdf::{CompileError, compile_pdf};
