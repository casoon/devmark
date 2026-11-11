//! Typst → PDF mit der `typst`-Bibliothek – offline, ohne externes Programm.
//!
//! Projektdateien (Bilder, `#include`) werden relativ zur Hauptdatei aus dem Dateisystem
//! geladen. Pakete (`@preview/…`) nur aus dem lokalen Typst-Cache bzw. Datenordner; es wird
//! nichts heruntergeladen.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::FsPackages;
use typst_layout::PagedDocument;

use crate::Warning;

/// Kompiliert `source` (Inhalt der Datei `main_path`) zu PDF-Bytes.
///
/// `main_path` muss nicht existieren – so lässt sich auch ungespeicherter Editor-Inhalt
/// setzen; relative Pfade werden trotzdem relativ zu seinem Ordner aufgelöst.
pub fn compile_pdf(source: &str, main_path: &Path) -> Result<Compiled, CompileError> {
    let world = DevmarkWorld::new(source, main_path);
    let warned = typst::compile::<PagedDocument>(&world);
    let warnings = warned.warnings.iter().map(|d| world.warning(d)).collect();
    let document = warned.output.map_err(|errors| {
        CompileError::Typst(errors.iter().map(|d| world.describe(d)).collect())
    })?;
    let options = typst_pdf::PdfOptions::default();
    let pdf = typst_pdf::pdf(&document, &options).map_err(|errors| {
        CompileError::Typst(errors.iter().map(|d| world.describe(d)).collect())
    })?;
    Ok(Compiled {
        pdf,
        pages: document.pages().len(),
        warnings,
    })
}

/// Kompiliert `source` und rendert die Seiten als PNG (für die Vorschau in der App).
///
/// `pixel_per_pt` bestimmt die Auflösung (2.0 ≈ Retina bei 100 % Zoom); höchstens
/// `max_pages` Seiten werden gerendert.
pub fn render_pages(
    source: &str,
    main_path: &Path,
    pixel_per_pt: f64,
    max_pages: usize,
) -> Result<Rendered, CompileError> {
    let world = DevmarkWorld::new(source, main_path);
    let warned = typst::compile::<PagedDocument>(&world);
    let warnings = warned.warnings.iter().map(|d| world.warning(d)).collect();
    let document = warned.output.map_err(|errors| {
        CompileError::Typst(errors.iter().map(|d| world.describe(d)).collect())
    })?;
    let options = typst_render::RenderOptions {
        pixel_per_pt: typst::utils::Scalar::new(pixel_per_pt),
        render_bleed: false,
    };
    let pages = document
        .pages()
        .iter()
        .take(max_pages)
        .map(|page| {
            let pixmap = typst_render::render(page, &options);
            RenderedPage {
                width: pixmap.width(),
                height: pixmap.height(),
                png: pixmap.encode_png().unwrap_or_default(),
            }
        })
        .collect();
    Ok(Rendered {
        pages,
        total_pages: document.pages().len(),
        warnings,
    })
}

pub struct Rendered {
    pub pages: Vec<RenderedPage>,
    pub total_pages: usize,
    pub warnings: Vec<Warning>,
}

pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

pub struct Compiled {
    pub pdf: Vec<u8>,
    pub pages: usize,
    pub warnings: Vec<Warning>,
}

#[derive(Debug)]
pub enum CompileError {
    Typst(Vec<String>),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompileError::Typst(errors) => write!(f, "{}", errors.join("\n")),
        }
    }
}

impl std::error::Error for CompileError {}

/// Schriften einmal pro Prozess laden: eingebettete (Libertinus, New Computer Modern,
/// DejaVu Sans Mono) plus Systemschriften.
fn font_store() -> &'static FontStore {
    static FONTS: OnceLock<FontStore> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut store = FontStore::new();
        store.extend(fonts::embedded());
        store.extend(fonts::system());
        store
    })
}

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

struct DevmarkWorld {
    main: FileId,
    main_file: PathBuf,
    files: FileStore<Loader>,
}

impl DevmarkWorld {
    fn new(source: &str, main_path: &Path) -> Self {
        let main_path = std::path::absolute(main_path).unwrap_or_else(|_| main_path.to_path_buf());
        // Wurzel = Dateisystem-Wurzel, damit auch `../bilder/x.png` erreichbar bleibt.
        let root = main_path
            .ancestors()
            .last()
            .unwrap_or(Path::new("/"))
            .to_path_buf();
        let vpath = VirtualPath::virtualize(&root, &main_path)
            .ok()
            .unwrap_or_else(|| VirtualPath::new("main.typ").expect("valid path"));
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();
        let loader = Loader {
            main,
            main_source: Bytes::from_string(source.to_string()),
            project: FsRoot::new(root),
            packages: [FsPackages::system_data(), FsPackages::system_cache()]
                .into_iter()
                .flatten()
                .collect(),
        };
        Self {
            main,
            main_file: main_path,
            files: FileStore::new(loader),
        }
    }

    /// Fehlermeldung mit Zeilenangabe, sofern sie in der Hauptdatei liegt.
    fn describe(&self, diagnostic: &SourceDiagnostic) -> String {
        let location = match self.location(diagnostic) {
            Some((file, line)) => format!(" ({file}, Zeile {line})"),
            None => String::new(),
        };
        format!("Fehler{location}: {}", self.message(diagnostic))
    }

    /// Warnung; liegt sie in der Hauptdatei, trägt sie deren Zeile.
    fn warning(&self, diagnostic: &SourceDiagnostic) -> Warning {
        match self.location(diagnostic) {
            Some((_, line)) if diagnostic.span.id() == Some(self.main) => Warning {
                line: Some(line),
                message: self.message(diagnostic),
            },
            Some((file, line)) => Warning::new(format!(
                "{file}, Zeile {line}: {}",
                self.message(diagnostic)
            )),
            None => Warning::new(self.message(diagnostic)),
        }
    }

    fn message(&self, diagnostic: &SourceDiagnostic) -> String {
        let mut message = diagnostic.message.to_string();
        for hint in &diagnostic.hints {
            message.push_str(&format!("\n  Hinweis: {}", hint.v));
        }
        message
    }

    /// Datei und Zeile (ab 1) einer Meldung.
    fn location(&self, diagnostic: &SourceDiagnostic) -> Option<(String, usize)> {
        let id = diagnostic.span.id()?;
        let range = self.range(diagnostic.span)?;
        let line = self.source(id).ok()?.lines().byte_to_line(range.start)?;
        let file = if id == self.main {
            self.main_file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            format!("{:?}", id.vpath())
        };
        Some((file, line + 1))
    }
}

struct Loader {
    main: FileId,
    main_source: Bytes,
    project: FsRoot,
    packages: Vec<FsPackages>,
}

impl FileLoader for Loader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            return Ok(self.main_source.clone());
        }
        match id.root() {
            VirtualRoot::Project => self.project.load(id.vpath()),
            VirtualRoot::Package(spec) => {
                let root = self
                    .packages
                    .iter()
                    .find_map(|packages| packages.obtain(spec))
                    .ok_or_else(|| {
                        FileError::Package(PackageError::Other(Some(
                            format!("Paket {spec} ist nicht lokal vorhanden (DevMark lädt keine Pakete herunter; einmal mit dem Typst-CLI kompilieren, um es zu cachen)").into(),
                        )))
                    })?;
                root.load(id.vpath())
            }
        }
    }
}

impl World for DevmarkWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        font_store().book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        font_store().font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;
        let offset_secs = offset.map(|d| d.seconds() as i64).unwrap_or(0);
        let days = (now + offset_secs).div_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Datetime::from_ymd(year, month, day)
    }
}

/// Tage seit 1970-01-01 → (Jahr, Monat, Tag), Algorithmus nach Howard Hinnant.
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_740), (2026, 10, 14));
    }

    #[test]
    fn compiles_simple_document() {
        let compiled =
            compile_pdf("= Hallo\n\nWelt $x^2$", Path::new("/tmp/devmark-test.typ")).unwrap();
        assert!(compiled.pdf.starts_with(b"%PDF"));
        assert_eq!(compiled.pages, 1);
    }

    #[test]
    fn renders_pages_as_png() {
        let rendered = render_pages(
            "= Hallo\n#pagebreak()\nZwei",
            Path::new("/tmp/devmark-test.typ"),
            1.0,
            10,
        )
        .unwrap();
        assert_eq!(rendered.pages.len(), 2);
        assert!(rendered.pages[0].png.starts_with(b"\x89PNG"));
        assert_eq!(rendered.pages[0].width, 595);
    }

    #[test]
    fn reports_errors_with_line() {
        let Err(CompileError::Typst(errors)) =
            compile_pdf("Hallo\n#unbekannt", Path::new("/tmp/devmark-test.typ"))
        else {
            panic!("expected error");
        };
        assert!(errors[0].contains("Zeile 2"), "{errors:?}");
    }
}
