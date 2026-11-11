---
title: Data and folders
description: Where DevMark stores notes, index and backups, and how to override those locations.
order: 3
---

## Notes folder

Notes are Markdown files in `~/Documents/DevMark Notizen`; choose another folder with Notizen → “Notizordner wählen …“ (choose notes folder). The trash lives in `.trash/` inside it.

## Data folder

Next to it, DevMark keeps an index for search, tags, favorites and version history:

| System | Location |
|---|---|
| macOS | `~/Library/Application Support/DevMark` |
| Linux | `~/.local/share/devmark` |
| Windows | `%APPDATA%\DevMark` |

Backups of the index are stored there under `Backups/`.

## Environment variables

| Variable | Overrides |
|---|---|
| `DEVMARK_VAULT_DIR` | the notes folder |
| `DEVMARK_DATA_DIR` | the data folder |
| `DEVMARK_BACKUP_DIR` | the backup folder |

## Moving from older versions

Notes from an earlier `devmark.sqlite` are moved into the notes folder once, on first start.
