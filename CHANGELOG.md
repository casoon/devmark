# Changelog

All notable changes to DevMark are listed here. DevMark follows
[Semantic Versioning](https://semver.org/): before 1.0, the interface and the data format may still change.

## [Unreleased]

### Changed

- **Pages:** the project site uses the current shared theme (0.12.10).


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
