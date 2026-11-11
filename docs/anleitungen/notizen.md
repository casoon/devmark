---
title: Notizen
description: Notizen sind Markdown-Dateien in einem eigenen Ordner – mit Suche, Tags, Verknüpfungen, Favoriten, Verlauf und Papierkorb.
order: 3
---

## Notizen sind Dateien

Jede Notiz ist eine normale Markdown-Datei im Notizordner, standardmäßig `~/Documents/DevMark Notizen`.

- **Ordner** im Notizordner sind echte Unterordner und bilden die Hierarchie.
- **Dateiname** ist die erste Zeile der Notiz; er folgt ihr, wenn sie sich ändert.
- **Sofort schreiben:** `Cmd/Ctrl-Shift-N`, kein Dateiname, kein Speichern – Autosave übernimmt.

Weil es nur Dateien sind, lassen sie sich mit jedem anderen Programm öffnen, versionieren oder [abgleichen](../sync/).

## Finden

- **Volltextsuche** über alle Notizen mit `Cmd/Ctrl-Shift-F`.
- **Tags** als `#tag` im Text oder als `tags:` im Frontmatter. Die Tags einer Notiz stehen über dem Text; ein Klick filtert die Liste.
- **Favoriten** markiert man in der Liste; die Navigation „Favoriten“ zeigt nur diese.

## Verknüpfen

`[[` öffnet eine Auswahl der vorhandenen Notizen. Eingefügt wird ein normaler relativer Markdown-Link – er funktioniert auch auf GitHub oder in jedem anderen Markdown-Programm. Benennt man eine Notiz um oder verschiebt sie, passt DevMark die Links an. Rückverweise stehen unter der Notiz.

## Verlauf, Papierkorb, Backup

- **Versionsverlauf:** frühere Stände einer Notiz bleiben erhalten und lassen sich zurückholen.
- **Papierkorb:** gelöschte Notizen liegen 30 Tage in `.trash/` im Notizordner.
- **Backup:** Der Index wird täglich gesichert.

## Notiz im Workspace ablegen

Menü Notizen → „Notiz im Workspace speichern …“ legt die aktive Notiz als Markdown-Datei im Workspace ab – etwa wenn aus einer Notiz ein Dokument im Projekt wird. Im selben Menü führt „Notizordner im Finder zeigen“ zum Ordner, „Notizordner wählen …“ wechselt ihn.
