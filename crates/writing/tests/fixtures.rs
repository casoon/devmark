//! Beispieldokumente müssen fehlerfrei zu PDF kompilieren.

use std::path::Path;

use devmark_writing::{Options, Warning, compile_pdf, markdown_to_typst};

fn convert_and_compile(name: &str) -> (String, Vec<Warning>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let source = std::fs::read_to_string(&path).unwrap();
    let conversion = markdown_to_typst(&source, &Options::for_file(&path));
    let typ_path = path.with_extension("typ");
    if let Err(err) = compile_pdf(&conversion.typst, &typ_path) {
        panic!("{name}: {err}\n\n{}", conversion.typst);
    }
    (conversion.typst, conversion.warnings)
}

#[test]
fn markdown_fixture_compiles() {
    let (typst, warnings) = convert_and_compile("beispiel.md");
    assert!(typst.contains("#outline()"));
    assert!(typst.contains("image(\"img/verlauf.png\""));
    assert_eq!(warnings.len(), 3, "{warnings:#?}");
}

#[test]
fn mdx_fixture_compiles() {
    let (typst, warnings) = convert_and_compile("beispiel.mdx");
    assert!(typst.contains("#set text(lang: \"en\""));
    assert!(typst.contains("#callout(kind: \"warning\", title: [Heads up])"));
    assert!(!typst.contains("import {") && !typst.contains("export "));
    assert!(!warnings.is_empty());
}
