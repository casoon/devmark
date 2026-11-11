---
title: First steps
description: Open a workspace, find your way around the three columns and write a first note.
order: 2
---

The interface is in German for now. Menu items are quoted as they appear in the app, with a translation where it helps.

## Workspace

```sh
cargo run -p devmark -- <folder-or-file>
```

- **A folder** becomes the workspace. The file tree shows only that folder; new documents and exports go there.
- **A file** opens in the most recently used workspace, even if it lives elsewhere. It then appears as a single tab, marked in the tab and the status bar.
- **No argument** starts DevMark in the most recently used workspace, or without one if there was none yet.

To switch workspaces, use the switcher at the top of the navigation: it shows the folder name, the recently opened workspaces and “Ordner öffnen …“ (open folder). The same is available under Datei → “Workspace öffnen …“ (File → Open workspace).

## The window

The window has three columns:

1. **Navigation** on the left: notes, favorites, note folders, tags, workspace files, search and trash.
2. **List** in the middle: the notes of the selected view with title, time and a two-line preview – or the file tree of the workspace.
3. **Text** on the right: the editor with tabs. `Cmd/Ctrl-\` splits it into two views.

The title bar toggles the list, split view, focus mode, print preview and cheat sheet, and exports to Typst, HTML or PDF. `Cmd/Ctrl-Shift-L` hides the middle column, `Cmd/Ctrl-Shift-Enter` switches to focus mode, which shows only the text.

## Your first note

`Cmd/Ctrl-Shift-N` creates a note. No file name needed: the first line becomes the title and the file name, and saving happens automatically. How notes are stored is described under [Notes](../../guides/notes/).

## Your first document

`Cmd/Ctrl-N` opens “Neues Dokument“ (new document) with templates for article, report, letter, book chapter, README and note – each in Markdown or Typst. Continue with [Writing](../../guides/writing/).
