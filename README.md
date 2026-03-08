# org-slipbox

`org-slipbox` is a local-first Org knowledge engine with an Emacs front-end.

It keeps parsing, indexing, ranking, and structural writes outside Emacs Lisp
while preserving Org files as the source of truth. The Rust side owns the
derived index and JSON-RPC protocol; the Elisp side owns commands, session
state, and presentation.

## Status

The repository is under active development. Work remains under `Unreleased`
until the project is cut as `v0.1.0`.

## Installation

`org-slipbox` ships as one repository containing:

- the Emacs package
- the Rust daemon and CLI

The two parts stay separable:

- the Emacs package lives in the repository root for straightforward ELPA-style loading and simple Emacs package builds
- the daemon is a normal `slipbox` executable that can live on `PATH` or be pointed to explicitly with `org-slipbox-server-program`

### Install The Daemon

#### Release Binary

Release tags publish platform-specific `slipbox` archives through GitHub
Actions. Unpack the archive somewhere on `PATH`, or point
`org-slipbox-server-program` at the unpacked binary.

When `slipbox` is already on `PATH`, the default
`org-slipbox-server-program` value works as-is and no extra daemon path setting
is required.

#### Build From Source

The default source build uses bundled SQLite, so no system SQLite development
package is required:

```bash
make build
```

This is equivalent to:

```bash
cargo build --release --locked
```

The `make` target prefers an available C compiler automatically for the bundled
SQLite build. The raw `cargo` command assumes your environment already exposes
one.

If you want the built daemon on `PATH`, you can install it directly:

```bash
make install-daemon
```

This is equivalent to:

```bash
cargo install --path . --locked
```

If you are packaging against a system SQLite instead of the bundled copy, use:

```bash
make build-system-sqlite
```

This is equivalent to:

```bash
cargo build --release --locked --no-default-features --features system-sqlite
```

To install that variant onto `PATH` directly:

```bash
make install-daemon-system-sqlite
```

The `system-sqlite` path expects a discoverable SQLite development
installation. If your toolchain cannot find it, either expose it through the
usual compiler and `pkg-config` environment for your platform, or use the
default bundled build instead.

### Install The Emacs Package

Add the repository root to your Emacs `load-path` and require `org-slipbox`:

```emacs-lisp
(add-to-list 'load-path "/path/to/org-slipbox")
(require 'org-slipbox)
```

### Emacs Setup

If `slipbox` is on `PATH`, the default setup is:

```emacs-lisp
(setq org-slipbox-directory (file-truename "~/notes"))
(setq org-slipbox-database-file
      (expand-file-name "org-slipbox.sqlite" user-emacs-directory))
(org-slipbox-mode 1)
```

If you built the daemon in a checkout and are not using `PATH`, also set:

```emacs-lisp
(setq org-slipbox-server-program
      "/path/to/org-slipbox/target/release/slipbox")
```

`org-slipbox-mode` is a single top-level integration mode. It enables:

- `org-slipbox-autosync-mode` for save, rename, delete, and VC-delete index updates
- `org-slipbox-id-mode` so indexed IDs cooperate with `org-id`
- `org-slipbox-completion-mode` in eligible Org buffers under `org-slipbox-directory`

Loading `org-slipbox` alone does not install global hooks or mutate user state.
The setup remains explicit and mode-owned.

If you prefer granular control, you can enable the pieces separately:

- `org-slipbox-autosync-mode`
- `org-slipbox-id-mode`
- `org-slipbox-completion-mode`

This upstream layout is intentionally simple for downstream packaging:

- Emacs packaging only needs the root `.el` files.
- The daemon is a separate executable discovered on `PATH` or through `org-slipbox-server-program`.
- The default source build works without a system SQLite development package, while packagers can switch to system SQLite explicitly.

## First Run

After enabling the mode, build the initial index:

```text
M-x org-slipbox-sync
```

Then try the core workflow:

1. `M-x org-slipbox-node-find`
2. Enter a new title and press `RET`
3. Finalize the draft with `C-c C-c`, or abort it with `C-c C-k`
4. Insert a link from another note with `M-x org-slipbox-node-insert`
5. Open the current-node context buffer with `M-x org-slipbox-buffer-toggle`

The first full sync builds the database. After that, autosync keeps the index
current incrementally.

## Core Workflow

`org-slipbox` supports the same basic note loop that `org-roam` users expect:

- `org-slipbox-node-find` visits an existing node or starts capture for a new one.
- `org-slipbox-node-insert` inserts a link to an existing node or captures a new one.
- `org-slipbox-capture` starts the same draft-based capture flow directly.
- `org-slipbox-buffer-toggle` shows the current node's context buffer.
- `org-slipbox-buffer-display-dedicated` opens a dedicated context buffer for one node.

File nodes and heading nodes are both first-class. Explicit IDs remain the
stable identity surface, but `org-slipbox` also supports lazy ID assignment when
you turn an existing note into a stable link target.

## Links And Completion

Graph edges are built from `id:` links. `org-slipbox` also supports title-based
`slipbox:` links as a writing convenience.

When `org-slipbox-mode` is enabled, completion is active in eligible Org files:

- inside Org bracket links, completion inserts `slipbox:Title` links
- set `org-slipbox-completion-everywhere` to non-nil to complete outside links too
- set `org-slipbox-link-auto-replace` to non-nil if you want `slipbox:` links rewritten to stable `id:` links on save

The node chooser behind `org-slipbox-node-read` supports configurable display
templates, annotation hooks, and sorting/filtering hooks.

## If You Use org-roam Today

These are the most common command-level equivalents:

| org-roam | org-slipbox |
| --- | --- |
| `org-roam-db-autosync-mode` | `org-slipbox-mode` or `org-slipbox-autosync-mode` |
| `org-roam-db-sync` | `org-slipbox-sync` |
| `org-roam-node-find` | `org-slipbox-node-find` |
| `org-roam-node-insert` | `org-slipbox-node-insert` |
| `org-roam-capture` | `org-slipbox-capture` |
| `org-roam-buffer-toggle` | `org-slipbox-buffer-toggle` |
| `org-roam-buffer-display-dedicated` | `org-slipbox-buffer-display-dedicated` |
| `org-roam-ref-find` | `org-slipbox-ref-find` |
| `org-roam-dailies-*` | `org-slipbox-dailies-*` |
| `org-roam-graph` | `org-slipbox-graph` |

The important design differences are:

- `org-slipbox` treats Emacs as the client and Rust as the engine.
- Index freshness is explicit and incremental rather than hidden behind ambient state.
- Completion and query hot paths stay index-backed instead of materializing the corpus in Elisp.
- The context buffer preserves the workflow surface without depending on `magit-section`.

## Optional Surfaces

These stay opt-in and isolated from startup:

- `org-slipbox-protocol-mode` for `roam-node` and `roam-ref`
- `org-slipbox-export-mode` for stable HTML export anchors
- `org-slipbox-graph` for Graphviz generation and viewing
- `org-slipbox-dailies-calendar-mode` for calendar marking

## Current Capabilities

- Scan an Org directory and build a SQLite index.
- Search indexed nodes through the Rust query engine.
- Format interactive node candidates with configurable display templates or functions.
- Read nodes through a single-step chooser with configurable filter, sort, annotation, and insertion-format hooks.
- Search nodes by aliases and tags stored in Org metadata.
- Resolve nodes by exact `ID`, exact title or alias, and current point location.
- Resolve nodes from indexed refs and citekeys through a dedicated ref chooser, and edit alias/ref/tag metadata from Emacs.
- Capture notes from refs without duplicating existing ref-backed nodes.
- Expand capture templates into exact file targets, optional file heads, outline-path targets, datetrees, and existing indexed nodes.
- Start note capture in a transient draft buffer, or commit prepared drafts directly with `:immediate-finish`, while only writing through Rust RPC.
- Support org-roam-style typed capture templates with `entry`, `plain`, `item`, `checkitem`, and `table-line` content.
- Honor capture lifecycle actions such as `:finalize`, `:jump-to-captured`, `:immediate-finish`, `:no-save`, and template finalize handlers.
- Display a persistent or dedicated context buffer with configurable ordered sections, postrender hooks, unique-backlink variants, and dedicated-buffer reference discovery sections.
- Complete and follow title-based org-slipbox links, with optional rewrite to stable `id:` links.
- Refile either the active region or the current subtree between indexed notes, and extract subtrees into new promoted file notes.
- Query indexed agenda entries from scheduled and deadline planning lines.
- Register opt-in `org-protocol` handlers for `roam-node` and `roam-ref` browser capture flows.
- Create and visit daily notes, append daily entries, move between existing daily notes, and opt into calendar marking for existing daily files.
- Enable optional HTML export support so Org ID-backed targets keep stable exported anchors.
- Keep indexed state current across Org file saves, renames, deletes, and VC deletes through explicit modes.
- Apply a shared file-discovery policy across indexing, autosync, dailies, and grep-backed discovery, with configurable extensions, exclude regexps, and `.gpg` / `.age` suffix handling.
- Export optional global or neighborhood Graphviz graphs from indexed `id:` links, with title shortening, link filtering, DOT output, rendered file generation, viewer integration, and optional `org-protocol` node URLs.

## Performance

`org-slipbox` ships with an explicit corpus benchmark harness instead of
relying on anecdotal scale claims.

- `cargo run --bin slipbox-bench -- check --profile ci` generates a deterministic corpus, measures full indexing, single-file incremental indexing, indexed search, backlinks, node-at-point lookup, agenda queries, and a batch Emacs benchmark of the persistent context-buffer redisplay path.
- `cargo run --bin slipbox-bench -- run --profile release --keep-corpus` runs the larger local profile and keeps the generated corpus under `target/bench/` for inspection.
- Benchmark profiles live in [`benches/profiles/ci.json`](/home/b/projects/org-slipbox/benches/profiles/ci.json) and [`benches/profiles/release.json`](/home/b/projects/org-slipbox/benches/profiles/release.json). Reports are written to `target/bench/`.

## Development

Use `make test` for the current Rust and Emacs checks.
Use `make build` for the default bundled-SQLite release build.
Use `make build-system-sqlite` when you want to link against a system SQLite installation.
Use `make bench-check PROFILE=ci` for the repeatable corpus regression gate.
Use `make bench PROFILE=release` for the larger local benchmark profile.
