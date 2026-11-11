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
- **Daily note:** “Heute“ in the navigation or `Cmd/Ctrl-Shift-J` opens `Journal/<date>.md` for today and creates it with the date as heading if it does not exist yet. The date is UTC.

Since they are just files, any other program can open, version or [sync](../sync/) them.

## Find

- **Full-text search** across all notes with `Cmd/Ctrl-Shift-F`. A hit shows its folder path (for example `Projects / Client X`) so notes with the same title can be told apart. The button next to the search field sorts the hits by “Zuletzt“ (last modified, the default) or “Relevanz“ (best match); DevMark remembers the choice.
- **Tags** as `#tag` in the text or as `tags:` in the frontmatter. A note's tags are shown above the text; click one to filter the list.
- **Favorites** are marked in the list; the “Favoriten“ view shows only those.
- **Tasks:** the “Aufgaben“ view lists the open tasks (`- [ ] …`) of all notes, grouped by note, the most recently changed note first. Tasks in code blocks and in the frontmatter are ignored. The box in front of a task ticks it off – DevMark writes `- [x]` into the file and changes nothing else in it. A click on the text opens the note at that line.

## Link

`[[` opens a picker of existing notes. What gets inserted is a plain relative Markdown link – it works on GitHub and in any other Markdown program too. When a note is renamed or moved, DevMark updates the links. Backlinks are shown below the note.

## Capture from outside

`devmark --note "Buy stamps"` appends the text to `Inbox.md` in the notes folder – without opening a window, so it works from a shell, a shortcut or a launcher. Without text, DevMark reads from standard input (`pbpaste | devmark --note`). Each entry gets a heading with the date and time in UTC; `Inbox.md` is created if it is missing. If the note is open in DevMark, the new entries show up when the window becomes active again.

## History, trash, backup

- **Version history:** earlier versions of a note are kept and can be restored.
- **Trash:** deleted notes stay in `.trash/` in the notes folder for 30 days.
- **Backup:** the index is backed up daily.

## Save a note to the workspace

Notizen → “Notiz im Workspace speichern …“ (save note to workspace) stores the active note as a Markdown file in the workspace – for example when a note grows into a document of the project. In the same menu, “Notizordner im Finder zeigen“ opens the folder and “Notizordner wählen …“ switches to another one.
