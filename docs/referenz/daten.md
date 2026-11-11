---
title: Daten und Ordner
description: Wo DevMark Notizen, Index und Backups ablegt und wie man die Orte überschreibt.
order: 3
---

## Notizordner

Notizen liegen als Markdown-Dateien in `~/Documents/DevMark Notizen`, wählbar über Menü Notizen → „Notizordner wählen …“. Der Papierkorb liegt in `.trash/` darin.

## Datenordner

Daneben führt DevMark einen Index für Suche, Tags, Favoriten und Versionsverlauf:

| System | Ort |
|---|---|
| macOS | `~/Library/Application Support/DevMark` |
| Linux | `~/.local/share/devmark` |
| Windows | `%APPDATA%\DevMark` |

Backups des Index liegen dort unter `Backups/`.

## Umgebungsvariablen

| Variable | Überschreibt |
|---|---|
| `DEVMARK_VAULT_DIR` | den Notizordner |
| `DEVMARK_DATA_DIR` | den Datenordner |
| `DEVMARK_BACKUP_DIR` | den Backup-Ordner |

## Umzug aus älteren Versionen

Notizen aus einer früheren `devmark.sqlite` werden beim ersten Start einmalig in den Notizordner übernommen.
