# Changelog

Alle nennenswerten Änderungen an DevMark stehen hier. DevMark folgt
[Semantic Versioning](https://semver.org/lang/de/); vor 1.0 können sich Bedienung und Datenformat noch ändern.

## [Unreleased]

### Added

- **Schreiben:** Über dem Text stehen Speicherstand und Wortzahl, bei Notizen zusätzlich die `#tags` (Klick filtert die Notizliste).
- **Schreiben:** Gliederung am rechten Rand – ein Strich je Überschrift, Klick springt hin.
- **Notizen:** Navigation „Favoriten“ zeigt nur markierte Notizen.
- **Workspace:** Kopf über dem Dateibaum mit Ordnername, zuletzt geöffneten Workspaces und „Ordner öffnen …“; Menü Datei → „Workspace öffnen …“.

### Changed

- **Workspace:** Der Dateibaum bleibt auf den gewählten Hauptordner begrenzt. Dateien von außerhalb öffnen sich als einzelner Tab (mit Hinweis in Tab und Statuszeile), statt die Wurzel zu verschieben.
- **Workspace:** Ohne Ordner-Argument startet DevMark im zuletzt genutzten Workspace oder ohne Workspace, nicht mehr im Home-Ordner. Neue Dokumente und Exporte landen im Workspace.
- **Schreiben:** Markdown, Typst und Notizen erscheinen in einer Serifenschrift in einer zentrierten Textspalte; Code bleibt in Monospace.
- **Notizen:** Die Liste zeigt Titel, Zeit und zwei Zeilen Vorschau; Verschieben, Favorit und Papierkorb erscheinen erst beim Überfahren.
- **Oberfläche:** Eine Navigation (Dateien, Notizen, Favoriten, Papierkorb) ersetzt den Umschalter oben in der Seitenleiste; die Grautöne sind etwas wärmer.

### Fixed

- **Oberfläche:** Die Icons für „Dateien“ und „Notizen“ in der Navigation werden wieder angezeigt.
