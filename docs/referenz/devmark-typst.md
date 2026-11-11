---
title: devmark-typst
description: Markdown, MDX und Typst auf der Kommandozeile in Typst oder PDF umwandeln – dieselbe Umwandlung wie im Editor.
order: 2
---

`devmark-typst` ist die Umwandlung aus dem Editor als eigenes Programm, etwa für Skripte oder einen Build.

```sh
cargo run -p devmark-writing --bin devmark-typst -- text.md --pdf
```

## Aufruf

```text
devmark-typst <eingabe.md|.mdx|.typ> [-o ausgabe.typ] [--pdf [ausgabe.pdf]]
              [--lang de] [--paper a4] [--body-only] [--stdout]
```

| Option | Wirkung |
|---|---|
| `-o`, `--output <pfad>` | Ziel der Typst-Datei (Standard: Eingabe mit Endung `.typ`) |
| `--pdf [pfad]` | zusätzlich ein PDF setzen, optional mit Zielpfad |
| `--lang <code>` | Sprache, wenn das Frontmatter keine nennt |
| `--paper <format>` | Papierformat, wenn das Frontmatter keines nennt |
| `--body-only` | nur den Textkörper ohne Dokument-Präambel ausgeben |
| `--stdout` | Typst auf die Standardausgabe schreiben |
| `-h`, `--help` | Aufruf anzeigen |

Was aus Markdown übernommen wird und welche Frontmatter-Felder gelten, steht unter [Druckvorschau und PDF](../../anleitungen/druck-und-pdf/). Alles läuft offline; Typst-Pakete kommen nur aus dem lokalen Cache.
