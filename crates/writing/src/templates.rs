//! Dokumentvorlagen („Neues Dokument“): Metadaten rein, lesbarer Quelltext raus.
//!
//! Markdown bekommt YAML-Frontmatter (den der Typst-Konverter versteht), Typst eine kleine
//! Präambel mit `set`-Regeln. Alles bleibt normaler, editierbarer Quelltext.

use crate::authoring::Syntax;
use crate::markdown::escape_string;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Article,
    Report,
    Letter,
    Chapter,
    Readme,
    Note,
}

#[derive(Debug, Clone, Default)]
pub struct DocInfo {
    pub title: String,
    pub author: String,
    /// ISO-Datum `JJJJ-MM-TT`; leer = heute nicht eintragen.
    pub date: String,
    /// Sprachkürzel, Standard `de`.
    pub lang: String,
}

impl DocType {
    pub const ALL: &'static [DocType] = &[
        DocType::Article,
        DocType::Report,
        DocType::Letter,
        DocType::Chapter,
        DocType::Readme,
        DocType::Note,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DocType::Article => "Artikel",
            DocType::Report => "Bericht",
            DocType::Letter => "Brief",
            DocType::Chapter => "Buchkapitel",
            DocType::Readme => "README",
            DocType::Note => "Notiz",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            DocType::Article => {
                "Fachartikel oder Blogbeitrag mit Titel, Einleitung und Abschnitten."
            }
            DocType::Report => {
                "Bericht mit Titelseite, Inhaltsverzeichnis und nummerierten Kapiteln."
            }
            DocType::Letter => {
                "Geschäftsbrief nach DIN 5008 mit Anschrift, Betreff und Grußformel."
            }
            DocType::Chapter => "Kapitel eines Buchs oder Manuskripts.",
            DocType::Readme => "Projektbeschreibung für ein Software-Repository.",
            DocType::Note => "Kurze Notiz ohne Gliederung.",
        }
    }

    /// Vorgeschlagener Dateiname ohne Endung.
    pub fn file_stem(self, info: &DocInfo) -> String {
        if self == DocType::Readme {
            return "README".into();
        }
        let slug: String = info
            .title
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'ä' => "ae".to_string(),
                'ö' => "oe".to_string(),
                'ü' => "ue".to_string(),
                'ß' => "ss".to_string(),
                c if c.is_ascii_alphanumeric() => c.to_string(),
                _ => "-".to_string(),
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if slug.is_empty() {
            self.label().to_lowercase()
        } else {
            slug
        }
    }

    pub fn render(self, syntax: Syntax, info: &DocInfo) -> String {
        let title = if info.title.trim().is_empty() {
            self.label()
        } else {
            info.title.trim()
        };
        let lang = if info.lang.trim().is_empty() {
            "de"
        } else {
            info.lang.trim()
        };
        match syntax {
            Syntax::Markdown => self.markdown(title, lang, info),
            Syntax::Typst => self.typst(title, lang, info),
        }
    }

    fn markdown(self, title: &str, lang: &str, info: &DocInfo) -> String {
        let mut front = vec![format!("title: \"{}\"", title.replace('"', "\\\""))];
        if !info.author.trim().is_empty() {
            front.push(format!(
                "author: \"{}\"",
                info.author.trim().replace('"', "\\\"")
            ));
        }
        if !info.date.trim().is_empty() {
            front.push(format!("date: {}", info.date.trim()));
        }
        front.push(format!("lang: {lang}"));
        match self {
            DocType::Report => {
                front.push("toc: true".into());
                front.push("numbering: true".into());
            }
            DocType::Readme | DocType::Note => front.clear(),
            _ => {}
        }
        let frontmatter = if front.is_empty() {
            String::new()
        } else {
            format!("---\n{}\n---\n\n", front.join("\n"))
        };
        let body = match self {
            DocType::Article => format!(
                "# {title}\n\n_Kurze Zusammenfassung in ein bis zwei Sätzen._\n\n## Einleitung\n\nWorum geht es, und warum ist es wichtig?\n\n## Hauptteil\n\nText …\n\n## Fazit\n\nWas bleibt?\n"
            ),
            DocType::Report => "## Zusammenfassung\n\nDas Wichtigste in Kürze.\n\n## Ausgangslage\n\nText …\n\n## Ergebnisse\n\n| Kennzahl | Wert |\n| --- | --- |\n| Beispiel | 42 |\n\n## Empfehlungen\n\n- Erste Empfehlung\n- Zweite Empfehlung\n".to_string(),
            DocType::Letter => format!(
                "**Absender** · Straße 1 · 12345 Ort\n\nEmpfänger GmbH  \nFrau Beispiel  \nStraße 2  \n12345 Ort\n\n**Betreff: {title}**\n\nSehr geehrte Damen und Herren,\n\nText des Briefs …\n\nMit freundlichen Grüßen\n\n{}\n",
                if info.author.trim().is_empty() { "Name" } else { info.author.trim() }
            ),
            DocType::Chapter => format!("# {title}\n\nErster Absatz des Kapitels …\n\n## Abschnitt\n\nText …\n"),
            DocType::Readme => format!(
                "# {title}\n\nKurzbeschreibung: Was macht das Projekt?\n\n## Installation\n\n```sh\n# Befehl zur Installation\n```\n\n## Verwendung\n\n```sh\n# Beispielaufruf\n```\n\n## Lizenz\n\nLizenzangabe\n"
            ),
            DocType::Note => format!("# {title}\n\n"),
        };
        format!("{frontmatter}{body}")
    }

    fn typst(self, title: &str, lang: &str, info: &DocInfo) -> String {
        let author = info.author.trim();
        let mut document = vec![format!("title: \"{}\"", escape_string(title))];
        if !author.is_empty() {
            document.push(format!("author: \"{}\"", escape_string(author)));
        }
        let mut out = format!("#set document({})\n", document.join(", "));
        let lang_line = format!("#set text(lang: \"{}\", size: 11pt)\n", escape_string(lang));
        let date = display_date(info.date.trim(), lang);
        let date = date.as_str();
        match self {
            DocType::Letter => {
                out.push_str("#set page(paper: \"a4\", margin: (left: 25mm, right: 20mm, top: 27mm, bottom: 25mm))\n");
                out.push_str(&lang_line);
                out.push_str("#set par(justify: true)\n\n");
                out.push_str("// Anschriftfeld nach DIN 5008 (Form B)\n");
                out.push_str("#place(top + left, dy: 18mm, block(width: 85mm, height: 45mm)[\n  #text(size: 8pt)[Absender · Straße 1 · 12345 Ort]\n  #v(4mm)\n  Empfänger GmbH \\\n  Frau Beispiel \\\n  Straße 2 \\\n  12345 Ort\n])\n");
                out.push_str("#v(80mm)\n");
                out.push_str(&format!(
                    "#align(right)[Ort, {}]\n\n",
                    if date.is_empty() {
                        "#datetime.today().display(\"[day].[month].[year]\")".to_string()
                    } else {
                        date.to_string()
                    }
                ));
                out.push_str(&format!("*{}*\n\n", typst_text(title)));
                out.push_str("Sehr geehrte Damen und Herren,\n\nText des Briefs …\n\nMit freundlichen Grüßen\n\n#v(1cm)\n");
                out.push_str(&format!(
                    "{}\n",
                    typst_text(if author.is_empty() { "Name" } else { author })
                ));
            }
            DocType::Report => {
                out.push_str("#set page(paper: \"a4\", margin: 2.5cm, numbering: \"1\")\n");
                out.push_str(&lang_line);
                out.push_str("#set par(justify: true)\n#set heading(numbering: \"1.1\")\n\n");
                out.push_str("// Titelseite\n#page(numbering: none)[\n  #v(30%)\n");
                out.push_str(&format!(
                    "  #text(size: 2.2em, weight: \"bold\")[{}]\n",
                    typst_text(title)
                ));
                if !author.is_empty() {
                    out.push_str(&format!("  #v(1em)\n  {}\n", typst_text(author)));
                }
                if !date.is_empty() {
                    out.push_str(&format!("  #v(0.5em)\n  {}\n", typst_text(date)));
                }
                out.push_str("]\n\n#outline()\n#pagebreak()\n\n");
                out.push_str("= Zusammenfassung\n\nDas Wichtigste in Kürze.\n\n= Ausgangslage\n\nText …\n\n= Ergebnisse\n\n#table(\n  columns: 2,\n  table.header([Kennzahl], [Wert]),\n  [Beispiel], [42],\n)\n\n= Empfehlungen\n\n- Erste Empfehlung\n- Zweite Empfehlung\n");
            }
            DocType::Chapter => {
                out.push_str("#set page(paper: \"a5\", margin: (inside: 2.2cm, outside: 1.6cm, y: 2cm), numbering: \"1\")\n");
                out.push_str(&lang_line.replace("11pt", "10.5pt"));
                out.push_str("#set par(justify: true, first-line-indent: 1em)\n\n");
                out.push_str(&format!(
                    "= {}\n\nErster Absatz des Kapitels …\n\n== Abschnitt\n\nText …\n",
                    typst_text(title)
                ));
            }
            _ => {
                out.push_str("#set page(paper: \"a4\", margin: 2.5cm, numbering: \"1\")\n");
                out.push_str(&lang_line);
                out.push_str("#set par(justify: true)\n\n");
                match self {
                    DocType::Article => {
                        out.push_str(&format!("#align(center, text(size: 1.8em, weight: \"bold\")[{}])\n", typst_text(title)));
                        if !author.is_empty() || !date.is_empty() {
                            let byline: Vec<String> = [author, date].iter().filter(|s| !s.is_empty()).map(|s| typst_text(s)).collect();
                            out.push_str(&format!("#align(center)[{}]\n", byline.join(" · ")));
                        }
                        out.push_str("\n_Kurze Zusammenfassung in ein bis zwei Sätzen._\n\n= Einleitung\n\nWorum geht es, und warum ist es wichtig?\n\n= Hauptteil\n\nText …\n\n= Fazit\n\nWas bleibt?\n");
                    }
                    DocType::Readme => out.push_str(&format!(
                        "= {}\n\nKurzbeschreibung: Was macht das Projekt?\n\n== Installation\n\n```sh\n# Befehl zur Installation\n```\n\n== Verwendung\n\n```sh\n# Beispielaufruf\n```\n",
                        typst_text(title)
                    )),
                    _ => out.push_str(&format!("= {}\n\n", typst_text(title))),
                }
            }
        }
        out
    }
}

/// ISO-Datum für die Anzeige: deutsch `14.10.2026`, sonst unverändert.
fn display_date(date: &str, lang: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    match parts.as_slice() {
        [year, month, day] if lang == "de" && year.len() == 4 => format!("{day}.{month}.{year}"),
        _ => date.to_string(),
    }
}

fn typst_text(text: &str) -> String {
    crate::markdown::escape_markup(text, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn all_typst_templates_compile() {
        let info = DocInfo {
            title: "Test: Größe & #1".into(),
            author: "Ada".into(),
            date: "2026-10-14".into(),
            lang: "de".into(),
        };
        for &doc in DocType::ALL {
            let source = doc.render(Syntax::Typst, &info);
            if let Err(err) = crate::compile_pdf(&source, Path::new("/tmp/vorlage.typ")) {
                panic!("{doc:?}: {err}\n{source}");
            }
        }
    }

    #[test]
    fn all_markdown_templates_convert_and_compile() {
        let info = DocInfo {
            title: "Test \"zitiert\"".into(),
            ..Default::default()
        };
        for &doc in DocType::ALL {
            let source = doc.render(Syntax::Markdown, &info);
            let typst = crate::markdown_to_typst(&source, &crate::Options::default()).typst;
            if let Err(err) = crate::compile_pdf(&typst, Path::new("/tmp/vorlage.typ")) {
                panic!("{doc:?}: {err}\n{typst}");
            }
        }
    }

    #[test]
    fn german_dates() {
        assert_eq!(display_date("2026-10-14", "de"), "14.10.2026");
        assert_eq!(display_date("2026-10-14", "en"), "2026-10-14");
    }

    #[test]
    fn file_names() {
        let info = DocInfo {
            title: "Über Größe!".into(),
            ..Default::default()
        };
        assert_eq!(DocType::Article.file_stem(&info), "ueber-groesse");
        assert_eq!(DocType::Readme.file_stem(&info), "README");
    }
}
