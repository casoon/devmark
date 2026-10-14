# DevMark

Universeller Editor für Code, Notizen und längere Texte – Rust + [GPUI](https://www.gpui.rs) über [gpui-kit](https://github.com/longbridge/gpui-kit). Local First, Desktop (macOS, Windows, Linux).

**Status:** Phase-1-Spike (Dateibaum, Editor mit Tree-sitter-Highlighting, Tabs, Split, Speichern).
Der frühere Flutter-Stand liegt im Branch `archive/flutter-v0.1`.

## Starten

```sh
cargo run -p devmark -- <ordner>
```

| Taste | Aktion |
|---|---|
| Cmd/Ctrl-S | Speichern |
| Cmd/Ctrl-W | Tab schließen |
| Cmd/Ctrl-\ | Split an/aus |

## Lizenz

GPL-3.0-or-later
