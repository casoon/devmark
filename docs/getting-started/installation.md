---
title: Installation
description: DevMark is built from source. On macOS, a script turns it into a regular app in the Applications folder.
order: 1
---

DevMark is written in Rust and runs on macOS, Windows and Linux. There are no prebuilt downloads yet; you build it from the repository.

## Requirements

- A current stable Rust toolchain (the project uses edition 2024), most easily installed with `rustup`.
- Git, to fetch the repository.

## Build and run

```sh
git clone https://github.com/casoon/devmark.git
cd devmark
cargo run -p devmark -- ~/projects/my-text
```

The first build takes a few minutes. The argument is a folder or a file; see [First steps](../first-steps/).

## macOS: install as an app

```sh
scripts/bundle-macos.sh --install
```

The script builds a release version, packages it as `DevMark.app` and copies it to `/Applications`. Without `--install` the app stays in `target/release/bundle/`. The app registers itself as an editor for Markdown files.
