---
title: Notes
description: Notes are Markdown files in a folder of their own – with search, tags, links, favorites, history and trash.
order: 3
---

## Notes are files

Every note is a plain Markdown file in the notes folder, by default `~/Documents/DevMark Notizen`.

- **Folders** in the notes folder are real subfolders and form the hierarchy.
- **The file name** is the first line of the note and follows it when it changes.
- **Write right away:** `Cmd/Ctrl-Shift-N`, no file name, no saving – autosave takes care of it.

Since they are just files, any other program can open, version or [sync](../sync/) them.

## Find

- **Full-text search** across all notes with `Cmd/Ctrl-Shift-F`.
- **Tags** as `#tag` in the text or as `tags:` in the frontmatter. A note's tags are shown above the text; click one to filter the list.
- **Favorites** are marked in the list; the “Favoriten“ view shows only those.

## Link

`[[` opens a picker of existing notes. What gets inserted is a plain relative Markdown link – it works on GitHub and in any other Markdown program too. When a note is renamed or moved, DevMark updates the links. Backlinks are shown below the note.

## History, trash, backup

- **Version history:** earlier versions of a note are kept and can be restored.
- **Trash:** deleted notes stay in `.trash/` in the notes folder for 30 days.
- **Backup:** the index is backed up daily.

## Save a note to the workspace

Notizen → “Notiz im Workspace speichern …“ (save note to workspace) stores the active note as a Markdown file in the workspace – for example when a note grows into a document of the project. In the same menu, “Notizordner im Finder zeigen“ opens the folder and “Notizordner wählen …“ switches to another one.
