//! Kommandozeile: Markdown/MDX → Typst, optional direkt PDF.
//!
//! ```text
//! devmark-typst <eingabe.md|.mdx|.typ> [-o ausgabe.typ] [--pdf [ausgabe.pdf]]
//!               [--lang de] [--paper a4] [--body-only] [--stdout]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use devmark_writing::{Options, compile_pdf, markdown_to_typst};

const USAGE: &str = "Aufruf: devmark-typst <eingabe.md|.mdx|.typ> [-o ausgabe.typ] [--pdf [ausgabe.pdf]] [--lang de] [--paper a4] [--body-only] [--stdout]";

struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    pdf: Option<Option<PathBuf>>,
    lang: Option<String>,
    paper: Option<String>,
    body_only: bool,
    stdout: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut input = None;
    let mut output = None;
    let mut pdf = None;
    let mut lang = None;
    let mut paper = None;
    let mut body_only = false;
    let mut stdout = false;
    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(USAGE.into()),
            "-o" | "--output" => output = Some(PathBuf::from(args.next().ok_or("-o braucht einen Pfad")?)),
            "--pdf" => {
                let path = args.next_if(|next| !next.starts_with('-')).map(PathBuf::from);
                pdf = Some(path);
            }
            "--lang" => lang = Some(args.next().ok_or("--lang braucht einen Wert")?),
            "--paper" => paper = Some(args.next().ok_or("--paper braucht einen Wert")?),
            "--body-only" => body_only = true,
            "--stdout" => stdout = true,
            other if other.starts_with('-') => return Err(format!("Unbekannte Option {other}\n{USAGE}")),
            other => {
                if input.replace(PathBuf::from(other)).is_some() {
                    return Err(format!("Nur eine Eingabedatei erlaubt\n{USAGE}"));
                }
            }
        }
    }
    Ok(Args {
        input: input.ok_or(USAGE)?,
        output,
        pdf,
        lang,
        paper,
        body_only,
        stdout,
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let source = std::fs::read_to_string(&args.input)
        .map_err(|err| format!("{}: {err}", args.input.display()))?;
    let is_typst = args.input.extension().is_some_and(|ext| ext == "typ");

    let (typst, typ_path) = if is_typst {
        (source, args.input.clone())
    } else {
        let typ_path = args.output.clone().unwrap_or_else(|| args.input.with_extension("typ"));
        let mut options = Options::for_file(&args.input);
        options.output_dir = Some(parent_dir(&typ_path));
        options.standalone = !args.body_only;
        if let Some(lang) = &args.lang {
            options.lang = lang.clone();
        }
        if let Some(paper) = &args.paper {
            options.paper = paper.clone();
        }
        let conversion = markdown_to_typst(&source, &options);
        for warning in &conversion.warnings {
            eprintln!("Hinweis: {warning}");
        }
        if args.stdout {
            print!("{}", conversion.typst);
        } else {
            std::fs::write(&typ_path, &conversion.typst)
                .map_err(|err| format!("{}: {err}", typ_path.display()))?;
            eprintln!("Typst: {}", typ_path.display());
        }
        (conversion.typst, typ_path)
    };

    if let Some(pdf_path) = args.pdf {
        let pdf_path = pdf_path.unwrap_or_else(|| typ_path.with_extension("pdf"));
        let compiled = compile_pdf(&typst, &typ_path).map_err(|err| err.to_string())?;
        for warning in &compiled.warnings {
            eprintln!("{warning}");
        }
        std::fs::write(&pdf_path, &compiled.pdf).map_err(|err| format!("{}: {err}", pdf_path.display()))?;
        eprintln!("PDF: {} ({} Seiten)", pdf_path.display(), compiled.pages);
    } else if is_typst {
        return Err("Für .typ-Eingaben --pdf angeben.".into());
    }
    Ok(())
}

fn parent_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}
