---
title: Installation
description: Prebuilt downloads for macOS and Linux, or a build from source; how to verify a download.
order: 1
---

DevMark is written in Rust and runs on macOS, Windows and Linux. Prebuilt versions for macOS and Linux are attached to the [GitHub releases](https://github.com/casoon/devmark/releases); until a release lists your platform, build it from the repository.

## Download

| File | For |
|---|---|
| `DevMark-<version>-macos-universal.zip` | macOS 13 or newer, Apple silicon and Intel |
| `devmark-<version>-linux-x86_64.tar.gz` | Linux on x86_64 |

On macOS, unpack the zip and move `DevMark.app` to the Applications folder. The app is not notarized by Apple, so the first start needs right-click → Open. On Linux, unpack the archive and start `devmark`; it needs the usual desktop libraries (Vulkan, Wayland or X11, fontconfig).

### Verify a download

Official binaries come only from the GitHub releases of `casoon/devmark`. They are built there by the release workflow from the tagged commit, never on a developer machine. Each file has a `.sha256` next to it, the release a combined `SHA256SUMS`, and every download a signed build provenance attestation:

```sh
shasum -a 256 -c DevMark-<version>-macos-universal.zip.sha256
gh attestation verify DevMark-<version>-macos-universal.zip --repo casoon/devmark
```

The second command needs the [GitHub CLI](https://cli.github.com). It confirms which workflow run and which commit produced the file.

Each download contains `THIRD-PARTY-LICENSES.txt` with the licences of the third-party software and fonts it includes.

## Requirements

- A current stable Rust toolchain (the project uses edition 2024), most easily installed with `rustup`.
- Git, to fetch the repository.

## Build from source

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
