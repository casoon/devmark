---
title: Project structure
description: The crates in the repository and what they contain.
order: 4
---

DevMark is a Cargo workspace. The interface is built on [GPUI](https://www.gpui.rs) and [gpui-kit](https://github.com/longbridge/gpui-kit).

| Crate | Contents |
|---|---|
| `apps/desktop` | The GPUI app |
| `crates/writing` | Markdown/MDX → Typst, PDF and preview, formatting actions, templates; contains `devmark-typst` |
| `crates/storage` | SQLite, migrations, change log, backups |
| `crates/notes` | Notes folder: Markdown files, index, links, folders, tags, search, revisions, trash |

## Checks

The same checks run in CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- --deny warnings
cargo test --workspace --locked
```

The source code is licensed under Apache-2.0 or MIT, at your option.
