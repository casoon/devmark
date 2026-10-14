//! Export aller Dokumente als Dateien (Ordnerstruktur bleibt erhalten) – kein Lock-in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Filter, Folder, Notes};

/// Schreibt alle aktiven Dokumente nach `dir` (Markdown mit Frontmatter bzw. Typst mit
/// Kommentarkopf). Gibt die Anzahl der Dateien zurück.
pub fn export_markdown(notes: &Notes, dir: &Path) -> std::io::Result<usize> {
    let io = |err: rusqlite::Error| std::io::Error::other(err.to_string());
    let folders = notes.folders().map_err(io)?;
    let documents = notes.list(&Filter::default()).map_err(io)?;
    std::fs::create_dir_all(dir)?;

    let mut used: HashMap<PathBuf, usize> = HashMap::new();
    for document in &documents {
        let folder_dir = folder_path(&folders, document.folder_id.as_deref());
        let target_dir = dir.join(folder_dir);
        std::fs::create_dir_all(&target_dir)?;
        let stem = file_stem(&document.display_title());
        let ext = document.format.extension();
        let count = used.entry(target_dir.join(&stem)).or_insert(0);
        *count += 1;
        let name = if *count == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{count}.{ext}") };

        let tags = notes.tags_of(&document.id).map_err(io)?;
        let header = match document.format {
            crate::Format::Markdown => {
                let mut lines = vec![
                    "---".to_string(),
                    format!("id: {}", document.id),
                    format!("title: \"{}\"", document.display_title().replace('"', "\\\"")),
                ];
                if !tags.is_empty() {
                    lines.push(format!("tags: [{}]", tags.join(", ")));
                }
                lines.push(format!("created: {}", document.created_at));
                lines.push(format!("updated: {}", document.updated_at));
                lines.push("---\n\n".into());
                lines.join("\n")
            }
            crate::Format::Typst => format!(
                "// id: {}\n// tags: {}\n\n",
                document.id,
                tags.join(", ")
            ),
        };
        std::fs::write(target_dir.join(name), format!("{header}{}", document.content))?;
    }
    Ok(documents.len())
}

fn folder_path(folders: &[Folder], id: Option<&str>) -> PathBuf {
    let mut parts = Vec::new();
    let mut current = id;
    // Zyklen (fehlerhafte Daten) dürfen nicht hängen.
    for _ in 0..64 {
        let Some(folder) = current.and_then(|id| folders.iter().find(|f| f.id == id)) else {
            break;
        };
        parts.push(file_stem(&folder.name));
        current = folder.parent_id.as_deref();
    }
    parts.iter().rev().collect()
}

fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() { "Notiz".into() } else { cleaned.chars().take(100).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Format, Kind};

    #[test]
    fn exports_folder_structure() {
        let mut notes = Notes::open_in_memory().unwrap();
        let folder = notes.create_folder("Arbeit/Privat", None).unwrap();
        let doc = notes.create(Kind::Note, Format::Markdown, "Plan", "Inhalt", Some(&folder.id)).unwrap();
        notes.add_tag(&doc.id, "x").unwrap();
        notes.create(Kind::Note, Format::Markdown, "Plan", "Zweiter", Some(&folder.id)).unwrap();
        notes.create(Kind::Article, Format::Typst, "", "= Typst", None).unwrap();
        let dir = std::env::temp_dir().join(format!("devmark-export-{}", devmark_storage::new_id()));
        assert_eq!(export_markdown(&notes, &dir).unwrap(), 3);
        // Gleichnamige Dokumente bekommen eindeutige Dateinamen.
        let plans: Vec<String> = ["Plan.md", "Plan-2.md"]
            .iter()
            .map(|name| std::fs::read_to_string(dir.join("Arbeit-Privat").join(name)).unwrap())
            .collect();
        assert_eq!(plans.iter().filter(|p| p.contains("tags: [x]") && p.ends_with("Inhalt")).count(), 1);
        assert!(dir.join("Typst.typ").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
