---
title: devmark-typst
description: Convert Markdown, MDX and Typst to Typst or PDF on the command line – the same conversion as in the editor.
order: 2
---

`devmark-typst` is the editor's conversion as a program of its own, for scripts or a build.

```sh
cargo run -p devmark-writing --bin devmark-typst -- text.md --pdf
```

## Usage

```text
devmark-typst <input.md|.mdx|.typ> [-o output.typ] [--pdf [output.pdf]]
              [--lang de] [--paper a4] [--body-only] [--stdout]
```

| Option | Effect |
|---|---|
| `-o`, `--output <path>` | Target of the Typst file (default: the input with the extension `.typ`) |
| `--pdf [path]` | Also typeset a PDF, optionally to the given path |
| `--lang <code>` | Language, if the frontmatter names none |
| `--paper <size>` | Paper size, if the frontmatter names none |
| `--body-only` | Output only the body, without the document preamble |
| `--stdout` | Write Typst to standard output |
| `-h`, `--help` | Show usage |

Which Markdown features are supported and which frontmatter fields apply is described under [Print preview and PDF](../../guides/print-and-pdf/). Everything runs offline; Typst packages are loaded from the local cache only.
