---
title: Installation
description: DevMark wird aus dem Quellcode gebaut. Unter macOS entsteht daraus eine normale App im Programme-Ordner.
order: 1
---

DevMark ist in Rust geschrieben und läuft unter macOS, Windows und Linux. Vorgefertigte Downloads gibt es noch nicht; gebaut wird aus dem Repository.

## Voraussetzungen

- Eine aktuelle stabile Rust-Toolchain (das Projekt nutzt Edition 2024), am einfachsten über `rustup`.
- Git, um das Repository zu holen.

## Bauen und starten

```sh
git clone https://github.com/casoon/devmark.git
cd devmark
cargo run -p devmark -- ~/Projekte/mein-text
```

Der erste Build dauert einige Minuten. Das Argument ist ein Ordner oder eine Datei; mehr dazu unter [Erste Schritte](../erste-schritte/).

## macOS: als App installieren

```sh
scripts/bundle-macos.sh --install
```

Das Skript baut eine Release-Version, packt sie als `DevMark.app` und kopiert sie nach `/Applications`. Ohne `--install` bleibt die App unter `target/release/bundle/`. DevMark meldet sich dabei als Editor für Markdown-Dateien an.
