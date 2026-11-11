---
title: Druckvorschau und PDF
description: Live-Vorschau neben dem Text, PDF-Export über den Typst-Satz und Umwandlung von Markdown in Typst.
order: 2
---

DevMark setzt Markdown und Typst mit [Typst](https://typst.app). Alles läuft offline; Typst-Pakete kommen nur aus dem lokalen Cache.

## Druckvorschau

`Cmd/Ctrl-Shift-V` öffnet die Vorschau neben dem Text. Sie zeigt die gesetzten Seiten und folgt beim Tippen. In Notizen stehen Verknüpfungen zu anderen Notizen als Text, nicht als Link.

## PDF exportieren

`Cmd/Ctrl-Shift-E` schreibt das Dokument als PDF in den Workspace. Seitenformat, Inhaltsverzeichnis und Nummerierung steuert das Frontmatter am Anfang der Datei:

```markdown
---
title: "Quartalsbericht"
author: "Maria Muster"
date: "2026-11-11"
lang: de
toc: true
numbering: true
paper: a4
---
```

| Feld | Wirkung |
|---|---|
| `title`, `author`, `date` | Titelangaben des Dokuments |
| `lang` | Sprache für Silbentrennung und Anführungszeichen |
| `toc` | Inhaltsverzeichnis an den Anfang |
| `numbering` | Überschriften nummerieren |
| `paper` | Papierformat, z. B. `a4` |

## Was aus Markdown übernommen wird

CommonMark und GitHub-Markdown mit Tabellen, Fußnoten und Aufgabenlisten, Hinweisboxen (`> [!NOTE]`), LaTeX-Mathe und Frontmatter. MDX-Dateien gehen auch: Imports und Ausdrücke werden entfernt, Hinweis-Komponenten und `:::tip`-Blöcke werden zu Hinweisboxen.

## Markdown → Typst

`Cmd/Ctrl-Shift-T` wandelt das aktuelle Markdown-Dokument in eine Typst-Datei um – für Texte, deren Satz man danach von Hand verfeinern will. Dieselbe Umwandlung gibt es auf der Kommandozeile als [devmark-typst](../../referenz/devmark-typst/).
