# Changelog

All notable changes to DevMark are listed here. DevMark follows
[Semantic Versioning](https://semver.org/): before 1.0, the interface and the data format may still change.

## [Unreleased]

### Added

- **Notes:** “Heute“ (`Cmd/Ctrl-Shift-J`) opens today's note `Journal/<date>.md` and creates it if needed. See [Write](https://casoon.github.io/devmark/docs/guides/notes/).
- **CLI:** `devmark --note "Text"` appends a dated entry to `Inbox.md` in the notes folder without opening a window. See [Capture from outside](https://casoon.github.io/devmark/docs/guides/notes/#capture-from-outside).
- **Notes:** an “Aufgaben“ view lists the open tasks of all notes, grouped by note; ticking one off writes `- [x]` into the file. See [Find](https://casoon.github.io/devmark/docs/guides/notes/#find).
- **Notes:** search hits show the folder path and can be sorted by last modified (default) or by relevance; the choice is remembered. See [Find](https://casoon.github.io/devmark/docs/guides/notes/#find).
- **Export:** HTML pages follow light and dark mode, use the `accent` colour and get a navigation over all pages; with `base_url` in `.devmark/export.toml` the export writes a `sitemap.xml`. See [Export as HTML](https://casoon.github.io/devmark/docs/guides/print-and-pdf/#export-as-html).

### Changed

- **Pages:** the project site uses the current shared theme (0.12.12).
- **Pages:** the home page shows the path of a document from Markdown to preview, PDF and web pages, and names the prebuilt downloads; the site uses the shared theme 0.12.11.

## [0.3.0] - 2026-11-11

### Changed

- **Interface:** the window's title bar holds two groups of switches – list, split, focus and print preview, cheat sheet, note history – and the Typst, HTML and PDF exports; in a narrow window the switches show icons only. The status bar keeps only save state, word count and reading time.
- **Interface:** “Document” and “Note” buttons at the top of the navigation create new entries; search moved to the top; larger icons and buttons throughout, with a teal accent for the main actions, headings in the editor and the dark theme.
- **Interface:** every tab has its own close button.
- **Notes:** the formatting bar now also shows for notes. Note rows show their tags; the selected row holds move, favorite and trash.
- **Print:** the preview header names the PDF and shows the build status as a badge.
- **Pages:** the project site uses the current shared theme (0.12.10).

### Fixed

- **Interface:** column widths stay intact after leaving focus mode or hiding the list, preview or cheat sheet.

## [0.2.0] - 2026-11-11

### Added

- **Download:** first public release, under Apache-2.0 or MIT – a universal app for macOS and an archive for Linux (x86_64), built in GitHub Actions with checksums and signed build provenance. See [Verify a download](https://casoon.github.io/devmark/docs/getting-started/installation/#verify-a-download).
- **Workspace:** one main folder with file tree, syntax highlighting, tabs and split view, and a switcher with recently used workspaces. Files from outside open as a single tab. See [First steps](https://casoon.github.io/devmark/docs/getting-started/first-steps/#workspace).
- **Interface:** three columns – navigation, list or file tree, text – with focus mode (Cmd/Ctrl-Shift-Enter) and a toggle for the list (Cmd/Ctrl-Shift-L).
- **Writing:** Markdown and Typst with formatting bar, insert list (`/` at the start of a line), syntax hints, cheat sheet, templates and an outline in the margin; save state, word count and reading time above the text. See [Writing](https://casoon.github.io/devmark/docs/guides/writing/).
- **Print:** live print preview next to the text with build status and warnings that jump to their line; PDF export through Typst, all offline; `accent` in the frontmatter colours headings, links, callouts and table headers. See [Print preview and PDF](https://casoon.github.io/devmark/docs/guides/print-and-pdf/).
- **Export:** a document or the whole notes folder as static HTML pages with one stylesheet, without JavaScript; links between notes keep working. See [Export as HTML](https://casoon.github.io/devmark/docs/guides/print-and-pdf/#export-as-html).
- **Notes:** plain Markdown files in a notes folder, with full-text search, `#tags`, favorites, `[[` links that stay valid when notes move, backlinks, version history, trash and daily backup. See [Notes](https://casoon.github.io/devmark/docs/guides/notes/).
- **Sync:** put the notes folder in any synced folder or a Git repository; DevMark picks up changes from outside when its window becomes active. See [Sync](https://casoon.github.io/devmark/docs/guides/sync/).
- **CLI:** `devmark-typst` converts Markdown and MDX to Typst or PDF. See [devmark-typst](https://casoon.github.io/devmark/docs/reference/devmark-typst/).
- **Docs:** project page with documentation at [casoon.github.io/devmark](https://casoon.github.io/devmark/).

### Security

- **Database:** `rusqlite` 0.41 bundles SQLite 3.54, which fixes RUSTSEC-2026-0459.
