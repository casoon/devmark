# DevMark

Universeller Editor für Code, Notizen und längere Texte – Rust + [GPUI](https://www.gpui.rs) über [gpui-kit](https://github.com/longbridge/gpui-kit). Local First, Desktop (macOS, Windows, Linux).

**Status:** Editor (Dateibaum, Tree-sitter, Tabs, Split), Schreibhilfen für Markdown/Typst mit Live-Druckvorschau und PDF-Export, Notizen mit SQLite.
Der frühere Flutter-Stand liegt im Branch `archive/flutter-v0.1`.

## Starten

```sh
cargo run -p devmark -- <ordner-oder-datei>
```

## Funktionen

- **Editor:** Dateibaum, Syntax-Highlighting, Tabs, geteilte Ansicht, Öffnen-Dialog.
- **Schreiben (Markdown, Typst):** Formatleiste, Menüs „Format“/„Einfügen“, `/` am Zeilenanfang öffnet eine Einfüge-Liste, Erklärung beim Überfahren von Syntax, Spickzettel. „Neues Dokument“ legt Artikel, Bericht, Brief (DIN 5008), Buchkapitel, README oder Notiz an.
- **Druck:** Live-Vorschau neben dem Text; Export als PDF (Typst-Satz) oder Umwandlung Markdown → Typst zum Weiterarbeiten.
- **Notizen:** sofort schreiben ohne Dateinamen, Autosave, Volltextsuche, Favoriten, Papierkorb (30 Tage), Versionsverlauf, Export aller Notizen als Dateien, tägliches Backup.

| Taste | Aktion |
|---|---|
| Cmd/Ctrl-N | Neues Dokument aus Vorlage |
| Cmd/Ctrl-Shift-N | Neue Notiz |
| Cmd/Ctrl-Shift-F | Notizen durchsuchen |
| Cmd/Ctrl-O | Öffnen |
| Cmd/Ctrl-S | Speichern |
| Cmd/Ctrl-W | Tab schließen |
| Cmd/Ctrl-\ | Split an/aus |
| Cmd/Ctrl-B / I / K / E | Fett / Kursiv / Link / Code |
| Cmd/Ctrl-Alt-1…3 | Überschrift 1–3 |
| Cmd/Ctrl-Shift-7 / 8 / 9 | Nummerierte Liste / Aufzählung / Aufgabenliste |
| Cmd/Ctrl-Shift-V | Druckvorschau |
| Cmd/Ctrl-Shift-/ | Spickzettel |
| Cmd/Ctrl-Shift-E | PDF exportieren |
| Cmd/Ctrl-Shift-T | Markdown → Typst |

## Markdown → Typst auf der Kommandozeile

```sh
cargo run -p devmark-writing --bin devmark-typst -- text.md --pdf
```

Unterstützt CommonMark/GFM (Tabellen, Fußnoten, Aufgabenlisten, `> [!NOTE]`), LaTeX-Mathe, Frontmatter (`title`, `author`, `date`, `lang`, `toc`, `numbering`, `paper`), MDX (Imports/Ausdrücke werden entfernt, Hinweis-Komponenten und `:::tip`-Blöcke werden zu Hinweisboxen). Alles läuft offline; Typst-Pakete nur aus dem lokalen Cache.

## Daten

Notizen liegen in einer SQLite-Datenbank (macOS: `~/Library/Application Support/DevMark/devmark.sqlite`, Linux: `~/.local/share/devmark`, Windows: `%APPDATA%\DevMark`), Backups daneben unter `Backups/`. Überschreiben mit `DEVMARK_DATA_DIR` bzw. `DEVMARK_BACKUP_DIR`.

## Aufbau

| Crate | Inhalt |
|---|---|
| `apps/desktop` | GPUI-App |
| `crates/writing` | Markdown/MDX → Typst, PDF/Vorschau, Formatieraktionen, Vorlagen |
| `crates/storage` | SQLite, Migrationen, Änderungsprotokoll, Backups |
| `crates/notes` | Notizen: Dokumente, Ordner, Tags, Suche, Revisionen, Papierkorb, Export |

## Lizenz

GPL-3.0-or-later
