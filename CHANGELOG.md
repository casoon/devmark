# Changelog

Alle nennenswerten Änderungen an DevMark stehen hier. DevMark folgt
[Semantic Versioning](https://semver.org/lang/de/); vor 1.0 können sich Bedienung und Datenformat noch ändern.

## [Unreleased]

### Added

- **Schreiben:** Über dem Text stehen Speicherstand, Wortzahl und Lesezeit, bei Notizen zusätzlich die `#tags` (Klick filtert die Notizliste).
- **Schreiben:** Gliederung am rechten Rand – ein Strich je Überschrift, Klick springt hin.
- **Druck:** Die Kopfzeile der Druckvorschau zeigt Quelle → Ziel und den Bau-Status (Satzzeit bzw. Zahl der Hinweise). Klick listet die Hinweise; solche mit Zeilenangabe springen an die Stelle im Text. Auch `devmark-typst` nennt bei Hinweisen die Zeile.
- **Druck:** `accent: "#0a7f6f"` im Frontmatter färbt Überschriften, Links, Hinweisboxen und Tabellenkopf im PDF; bei zu wenig Kontrast zu Papierweiß erscheint ein Hinweis.
- **Export:** Datei → „Als HTML exportieren …“ und Notizen → „Notizordner als HTML exportieren …“ erzeugen statische Seiten mit einem Stylesheet, offline und ohne JavaScript. Verknüpfungen zwischen Notizen werden zu relativen `.html`-Links, Ordner bleiben Ordner, Hinweisboxen sehen aus wie im PDF; bei mehreren Seiten entsteht eine Übersicht `index.html`.
- **Notizen:** Navigation „Favoriten“ zeigt nur markierte Notizen.
- **Notizen:** Menü Notizen → „Notiz im Workspace speichern …“ legt die aktive Notiz als Markdown-Datei im Workspace ab, „Notizordner im Finder zeigen“ und „Notizordner wählen …“ führen zum bzw. wechseln den Ordner.
- **Workspace:** Umschalter oben in der Navigation mit Ordnername, zuletzt geöffneten Workspaces und „Ordner öffnen …“; Menü Datei → „Workspace öffnen …“.
- **Oberfläche:** Ansicht → „Liste ein/aus“ (Cmd/Ctrl-Shift-L) blendet die mittlere Spalte aus, „Fokusmodus“ (Cmd/Ctrl-Shift-Enter) zeigt nur den Text.

### Changed

- **Lizenz:** DevMark steht nach Wahl unter Apache-2.0 oder MIT (bisher GPL-3.0-or-later).
- **Notizen:** Notizen sind Markdown-Dateien in einem eigenen Ordner (Standard `~/Documents/DevMark Notizen`); Ordner sind echte Unterordner, der Dateiname folgt der ersten Zeile, der Papierkorb liegt in `.trash/`. Notizen aus der bisherigen Datenbank werden beim ersten Start übernommen.
- **Notizen:** Verknüpfungen sind normale relative Markdown-Links (Eingabe weiter mit `[[`) und werden beim Umbenennen und Verschieben angepasst. Tags kommen zusätzlich aus `tags:` im Frontmatter.
- **Workspace:** Der Dateibaum bleibt auf den gewählten Hauptordner begrenzt. Dateien von außerhalb öffnen sich als einzelner Tab (mit Hinweis in Tab und Statuszeile), statt die Wurzel zu verschieben.
- **Workspace:** Ohne Ordner-Argument startet DevMark im zuletzt genutzten Workspace oder ohne Workspace, nicht mehr im Home-Ordner. Neue Dokumente und Exporte landen im Workspace.
- **Schreiben:** Markdown, Typst und Notizen erscheinen in einer Serifenschrift in einer zentrierten Textspalte; Code bleibt in Monospace.
- **Notizen:** Die Liste zeigt Titel, Zeit und zwei Zeilen Vorschau; Verschieben, Favorit und Papierkorb erscheinen erst beim Überfahren.
- **Oberfläche:** Drei Spalten: links die Navigation (Notizen, Favoriten, Notizordner, Tags, Workspace-Dateien, Suche, Papierkorb), in der Mitte die Liste bzw. der Dateibaum, rechts der Text. Rückverweise stehen unter der Notiz, die Formatleiste erscheint nur bei Dateien. Die Grautöne sind etwas wärmer.

### Removed

- **Notizen:** Der eigene Sync-Ordner und „Alle Notizen exportieren …“ entfallen – der Notizordner selbst lässt sich über iCloud Drive, Nextcloud, Syncthing oder Git abgleichen.

### Fixed

- **Notizen:** Im PDF einer Notiz stehen Verknüpfungen zu anderen Notizen als Text statt als Link auf eine `.md`-Datei, die es beim Empfänger nicht gibt – wie schon in der Druckvorschau.
- **Oberfläche:** Die Icons für „Dateien“ und „Notizen“ in der Navigation werden wieder angezeigt.

### Security

- **Datenbank:** `rusqlite` 0.41 bündelt SQLite 3.54 und schließt damit RUSTSEC-2026-0459 (uninitialisierter Speicher bei nicht passendem Datenbank-/WAL-Paar).
