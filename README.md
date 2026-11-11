# DevMark

Universeller Editor für Code, Notizen und längere Texte – Rust + [GPUI](https://www.gpui.rs) über [gpui-kit](https://github.com/longbridge/gpui-kit). Local First, Desktop (macOS, Windows, Linux).

**Status:** Editor (Dateibaum, Tree-sitter, Tabs, Split), Schreibhilfen für Markdown/Typst mit Live-Druckvorschau und PDF-Export, Notizen als Markdown-Dateien in einem eigenen Ordner.
Der frühere Flutter-Stand liegt im Branch `archive/flutter-v0.1`.

## Starten

```sh
cargo run -p devmark -- <ordner-oder-datei>
```

Ein Ordner wird zum Workspace; der Dateibaum zeigt nur ihn. Eine Datei öffnet sich im zuletzt genutzten Workspace, auch wenn sie außerhalb liegt. Ohne Argument startet DevMark im zuletzt genutzten Workspace.

## Funktionen

- **Editor:** Workspace (ein Hauptordner) mit Dateibaum, Syntax-Highlighting, Tabs, geteilte Ansicht, Öffnen-Dialog.
- **Schreiben (Markdown, Typst):** Formatleiste, Menüs „Format“/„Einfügen“, `/` am Zeilenanfang öffnet eine Einfüge-Liste, Erklärung beim Überfahren von Syntax, Spickzettel. „Neues Dokument“ legt Artikel, Bericht, Brief (DIN 5008), Buchkapitel, README oder Notiz an.
- **Druck:** Live-Vorschau neben dem Text mit Bau-Status und Hinweisen; Export als PDF (Typst-Satz) oder Umwandlung Markdown → Typst zum Weiterarbeiten.
- **HTML:** Ein Dokument oder den ganzen Notizordner als statische Seiten exportieren – je Datei eine `.html`-Datei plus ein Stylesheet, offline und ohne JavaScript. Verknüpfungen zwischen Notizen werden zu relativen Links, Ordner bleiben Ordner, Hinweisboxen sehen aus wie im PDF.
- **Notizen:** normale Markdown-Dateien im Notizordner (Ordner = Hierarchie, Dateiname = erste Zeile). Sofort schreiben ohne Dateinamen, Autosave, Volltextsuche, `#tags` im Text oder `tags:` im Frontmatter, Favoriten, Papierkorb (30 Tage), Versionsverlauf, tägliches Backup. Verknüpfungen mit `[[` sind normale relative Markdown-Links und funktionieren auch auf GitHub; Umbenennen und Verschieben passen sie an. „Notiz im Workspace speichern …“ legt eine Notiz als Datei im Workspace ab.
- **Sync:** Den Notizordner in iCloud Drive, Nextcloud, Dropbox oder Syncthing legen (Menü Notizen → „Notizordner wählen …“) oder als Git-Repository führen. Änderungen von außen übernimmt DevMark, sobald das Fenster aktiv wird.

| Taste | Aktion |
|---|---|
| Cmd/Ctrl-N | Neues Dokument aus Vorlage |
| Cmd/Ctrl-Shift-N | Neue Notiz |
| Cmd/Ctrl-Shift-F | Notizen durchsuchen |
| Cmd/Ctrl-O | Öffnen |
| Cmd/Ctrl-S | Speichern |
| Cmd/Ctrl-W | Tab schließen |
| Cmd/Ctrl-\ | Split an/aus |
| Cmd/Ctrl-Shift-L | Liste ein/aus |
| Cmd/Ctrl-Shift-Enter | Fokusmodus |
| Cmd/Ctrl-B / I / K / E | Fett / Kursiv / Link / Code |
| Cmd/Ctrl-Alt-1…3 | Überschrift 1–3 |
| Cmd/Ctrl-Shift-7 / 8 / 9 | Nummerierte Liste / Aufzählung / Aufgabenliste |
| Cmd/Ctrl-Shift-V | Druckvorschau |
| Cmd/Ctrl-Shift-/ | Spickzettel |
| Cmd/Ctrl-Shift-E | PDF exportieren |
| Cmd/Ctrl-Shift-T | Markdown → Typst |

## macOS-App bauen

```sh
scripts/bundle-macos.sh --install   # DevMark.app nach /Applications
```

## Markdown → Typst auf der Kommandozeile

```sh
cargo run -p devmark-writing --bin devmark-typst -- text.md --pdf
```

Unterstützt CommonMark/GFM (Tabellen, Fußnoten, Aufgabenlisten, `> [!NOTE]`), LaTeX-Mathe, Frontmatter (`title`, `author`, `date`, `lang`, `toc`, `numbering`, `paper`, `accent` – Akzentfarbe für Überschriften, Links, Hinweisboxen und Tabellenkopf), MDX (Imports/Ausdrücke werden entfernt, Hinweis-Komponenten und `:::tip`-Blöcke werden zu Hinweisboxen). Alles läuft offline; Typst-Pakete nur aus dem lokalen Cache.

## Daten

Notizen liegen als Markdown-Dateien in `~/Documents/DevMark Notizen` (wählbar, Papierkorb in `.trash/`). Daneben führt DevMark einen Index für Suche, Tags, Favoriten und Versionsverlauf im Datenordner (macOS: `~/Library/Application Support/DevMark`, Linux: `~/.local/share/devmark`, Windows: `%APPDATA%\DevMark`), Backups des Index unter `Backups/`. Notizen aus einer früheren `devmark.sqlite` werden beim ersten Start einmalig in den Notizordner übernommen. Überschreiben mit `DEVMARK_VAULT_DIR`, `DEVMARK_DATA_DIR` bzw. `DEVMARK_BACKUP_DIR`.

## Aufbau

| Crate | Inhalt |
|---|---|
| `apps/desktop` | GPUI-App |
| `crates/writing` | Markdown/MDX → Typst und HTML, PDF/Vorschau, Formatieraktionen, Vorlagen |
| `crates/storage` | SQLite, Migrationen, Änderungsprotokoll, Backups |
| `crates/notes` | Notizordner: Markdown-Dateien, Index, Links, Ordner, Tags, Suche, Revisionen, Papierkorb |

## Lizenz

Nach Wahl [Apache-2.0](LICENSE-APACHE) oder [MIT](LICENSE-MIT).
