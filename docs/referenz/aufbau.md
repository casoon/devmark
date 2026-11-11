---
title: Aufbau des Projekts
description: Die Crates im Repository und was sie enthalten.
order: 4
---

DevMark ist ein Cargo-Workspace. Die Oberfläche baut auf [GPUI](https://www.gpui.rs) und [gpui-kit](https://github.com/longbridge/gpui-kit).

| Crate | Inhalt |
|---|---|
| `apps/desktop` | GPUI-App |
| `crates/writing` | Markdown/MDX → Typst, PDF und Vorschau, Formatieraktionen, Vorlagen; enthält `devmark-typst` |
| `crates/storage` | SQLite, Migrationen, Änderungsprotokoll, Backups |
| `crates/notes` | Notizordner: Markdown-Dateien, Index, Links, Ordner, Tags, Suche, Revisionen, Papierkorb |

## Prüfen

Dieselben Prüfungen laufen in der CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- --deny warnings
cargo test --workspace --locked
```

Der Quellcode steht unter Apache-2.0 oder MIT, nach Wahl.
