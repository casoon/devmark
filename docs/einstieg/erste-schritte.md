---
title: Erste Schritte
description: Workspace öffnen, sich in den drei Spalten zurechtfinden und die erste Notiz schreiben.
order: 2
---

## Workspace

```sh
cargo run -p devmark -- <ordner-oder-datei>
```

- **Ein Ordner** wird zum Workspace. Der Dateibaum zeigt nur diesen Ordner, neue Dokumente und Exporte landen dort.
- **Eine Datei** öffnet sich im zuletzt genutzten Workspace, auch wenn sie außerhalb liegt. Sie erscheint dann als einzelner Tab mit einem Hinweis in Tab und Statuszeile.
- **Ohne Argument** startet DevMark im zuletzt genutzten Workspace, oder ohne Workspace, wenn es noch keinen gab.

Den Workspace wechselt man oben in der Navigation: Der Umschalter zeigt den Ordnernamen, die zuletzt geöffneten Workspaces und „Ordner öffnen …“. Dasselbe steht im Menü Datei → „Workspace öffnen …“.

## Die Oberfläche

Das Fenster hat drei Spalten:

1. **Navigation** links: Notizen, Favoriten, Notizordner, Tags, Workspace-Dateien, Suche und Papierkorb.
2. **Liste** in der Mitte: die Notizen der gewählten Ansicht mit Titel, Zeit und zwei Zeilen Vorschau – oder der Dateibaum des Workspace.
3. **Text** rechts: der Editor mit Tabs. `Cmd/Ctrl-\` teilt ihn in zwei Ansichten.

`Cmd/Ctrl-Shift-L` blendet die mittlere Spalte aus, `Cmd/Ctrl-Shift-Enter` schaltet in den Fokusmodus, der nur den Text zeigt.

## Die erste Notiz

`Cmd/Ctrl-Shift-N` legt eine Notiz an. Einen Dateinamen braucht es nicht: Die erste Zeile wird zum Titel und zum Dateinamen, gespeichert wird automatisch. Wie Notizen auf der Platte liegen, steht unter [Notizen](../../anleitungen/notizen/).

## Das erste Dokument

`Cmd/Ctrl-N` öffnet „Neues Dokument“ mit Vorlagen für Artikel, Bericht, Brief, Buchkapitel, README und Notiz – jeweils in Markdown oder Typst. Weiter geht es unter [Schreiben](../../anleitungen/schreiben/).
