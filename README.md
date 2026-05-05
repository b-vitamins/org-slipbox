# org-slipbox

## What It Is

`org-slipbox` is an Emacs package and companion daemon for personal knowledge
management with interconnected Org notes.

It keeps a personal slip-box, or Zettelkasten, in plain Org files. Org files
remain the source of truth, while a derived SQLite index supports interactive
search, backlinks, refs, agenda queries, and structural edits. The Rust side
owns indexing, ranking, query execution, and file mutation. The Emacs Lisp
side owns commands, session state, and presentation.

## Status

Current development starts from the released `0.8.0` foundation.

The documented workflow surface is intended to remain complete enough for
day-to-day replacement use while the project deepens its exploratory model.
The released `0.8.x` line broadened the research workbench by composition:
named workflows, corpus-health audits, bounded workflow discovery, report
outputs, and stricter scale guarantees over the shipped headless surface.
The next `0.9.x` line is operational workbench work: durable review runs,
review status, review diffs, and safe remediation previews for recurring
workflow and audit loops. Raw-RPC sprawl, plugin-runtime ambitions, MCP,
agent-adapter claims, and broad automated mutation remain deferred.

## Requirements

- Emacs `29.1` or newer
- a `slipbox` daemon binary, either from a release archive or a local source build
- Graphviz only if you use the optional graph commands
- `org-protocol` only if you use the optional protocol handlers

## Product Docs

The durable product documents live in `doc/`:

- `doc/vision.org` states the north star
- `doc/milestones.org` translates that vision into capability milestones
- `doc/roadmap.org` maps the milestones onto release bands without date promises

## Installation

`org-slipbox` ships as one repository containing:

- the Emacs package
- the Rust daemon and CLI

The two parts stay separable:

- the Emacs package lives in the repository root for straightforward ELPA-style loading and simple Emacs package builds
- the daemon is a normal `slipbox` executable that can live on `PATH` or be pointed to explicitly with `org-slipbox-server-program`

### Install The Daemon

#### Release Binary

Tagged releases publish platform-specific `slipbox` archives through GitHub
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

### Guix Development Shell

The repository includes [manifest.scm](/home/b/projects/org-slipbox/manifest.scm)
for contributors who want a one-command development environment:

```bash
guix shell -m manifest.scm
```

The Makefile also exposes matching convenience targets:

```bash
make guix-build
make guix-build-system-sqlite
make guix-test
make guix-lint-rust
make guix-bench-check
```

These are convenience wrappers only. The normal source-build and binary-first
paths above remain the primary upstream interfaces.

The Guix shell also includes `emacs-org-roam`, so packaged comparison runs can
use the same shell instead of relying on ad hoc local installs.

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
   Use `M-x org-slipbox-node-insert-immediate` to skip the draft buffer for
   newly captured insertions.
5. Open the current-node context buffer with `M-x org-slipbox-buffer-toggle`
   Use `M-x org-slipbox-buffer-display-dedicated` when you want the fuller
   one-node view, including the more expensive discovery sections.

The first full sync builds the database. After that, autosync keeps the index
current incrementally.

## Core Workflow

`org-slipbox` supports the same basic note loop that `org-roam` users expect:

- `org-slipbox-node-find` visits an existing node or starts capture for a new one.
- `org-slipbox-node-insert` inserts a link to an existing node or captures a new one.
- `org-slipbox-node-insert-immediate` inserts a link and commits newly captured nodes directly.
- `org-slipbox-capture` starts the same draft-based capture flow directly.
- `org-slipbox-buffer-toggle` shows the persistent current-node context buffer.
- `org-slipbox-buffer-display-dedicated` opens the dedicated one-node context buffer with the fuller discovery surface.

File nodes and heading nodes are both first-class. Explicit IDs remain the
stable identity surface, but `org-slipbox` also supports lazy ID assignment when
you turn an existing note into a stable link target.

## Current-Node Buffer

`org-slipbox` provides the same two buffer entry points that `org-roam` users
expect:

- `org-slipbox-buffer-toggle` opens a persistent buffer that tracks the node at point
- `org-slipbox-buffer-display-dedicated` opens a dedicated buffer for one node without replacing it as point moves

The persistent buffer keeps the cheap indexed sections on the hot path. By
default, backlinks and forward links render from indexed daemon queries, while
expensive discovery sections such as reflinks and unlinked references render
only in dedicated buffers, where their daemon-backed query cost is explicit.

For migration purposes, the practical rule is simple: use
`org-slipbox-buffer-toggle` for a cheap tracking buffer that follows point, and
use `org-slipbox-buffer-display-dedicated` when you want the fuller
exploratory cockpit without point-driven replacement.

The dedicated buffer currently supports:

- declared lenses for `structure`, `refs`, `time`, `tasks`, `bridges`, `dormant`, and `unresolved`
- explicit pivot history with `[` and `]`, plus frozen-root toggling with `f`
- note comparison with `c` to set a compare target, `C` to clear it, and `g` to switch comparison groups across overlap, divergence, and tension
- explicit trails with `a`, `{`, `}`, and `T`, including replay and detached branching
- durable exploration artifacts with `s` to save the current lens, comparison, or trail state and `o` to reload a saved artifact into the dedicated cockpit
- explanation blocks for non-obvious results such as shared refs, bridge candidates, dormant notes, planning-date relations, task-state matches, and weak integration

This is the settled cockpit line through `0.6.x`: the dedicated buffer remains
the interactive exploratory house, while `0.6.x` adds durable exploration
artifacts and a first narrow machine-facing surface around them. `0.7.x`
builds the first usable headless workbench on top of that same model through
task-shaped `slipbox` commands for node resolution, live exploration,
comparison, and artifact lifecycle over `slipbox serve`. `0.8.x` broadens the
same house through named workflows, corpus-health audits, bounded workflow
discovery, report outputs, and scale gates. `0.9.x` should make those recurring
workbench loops reviewable and operational through durable review records,
status, diffs, and remediation previews, while keeping review state distinct
from notes and saved exploration artifacts. Broader platform maturity,
extension APIs, and agent-adapter work remain later work.

When the current node record includes indexed metadata, the node summary also
renders file modification time plus backlink and forward-link counts without
local filesystem stats.

The buffer surface is configurable:

```emacs-lisp
(setq org-slipbox-buffer-persistent-sections
      '((org-slipbox-buffer-backlinks-section :unique t)
        org-slipbox-buffer-forward-links-section
        org-slipbox-buffer-refs-section))

(setq org-slipbox-buffer-lens-plans
      '((refs
         org-slipbox-buffer-node-section
         org-slipbox-buffer-refs-section
         org-slipbox-buffer-reflinks-section
         org-slipbox-buffer-unlinked-references-section)))

(setq org-slipbox-buffer-expensive-sections 'dedicated)
```

Section functions may also take keyword arguments, and the overall buffer render
can be shaped with:

- `org-slipbox-buffer-section-filter-function`
- `org-slipbox-buffer-postrender-functions`
- `org-slipbox-buffer-expensive-sections`

Entries in the buffer are ordinary buttons that visit the related node or the
exact match location. This is an intentional divergence from `org-roam`:
`org-slipbox` preserves the workflow surface without depending on
`magit-section`, which keeps the persistent path simpler and cheaper.

## Node Metadata, Refs, And Citations

File-node titles come from `#+title`; heading-node titles come from the heading
text. Aliases, tags, refs, and planning metadata are indexed and available to
search, completion, and the context buffer.

- Add or remove aliases with `org-slipbox-alias-add` and `org-slipbox-alias-remove`
- Add or remove tags with `org-slipbox-tag-add` and `org-slipbox-tag-remove`
- Add or remove refs with `org-slipbox-ref-add` and `org-slipbox-ref-remove`
- Find canonical ref-backed nodes with `org-slipbox-ref-find`

`ROAM_REFS` may contain URLs, citation keys, or multiple refs for a single
note. `org-slipbox` normalizes both Org-cite forms like `[cite:@key]` and
org-ref forms like `cite:key` into the same indexed ref surface, so citation
backlinks appear through the same reflink workflows instead of requiring a
separate code path.

`ROAM_EXCLUDE` is recognized during indexing for file nodes and heading nodes.
Any present value excludes that node from the derived index except the literal
value `nil`, which explicitly clears a local or inherited exclusion. File
discovery stays separate from node membership, so excluded files still remain
eligible for file-level surfaces and `org-id` compatibility fallback.

File tags come from standard Org file-tag behavior, including `#+filetags`.
Heading tags are ordinary Org tags.

## Links And Completion

Graph edges are built from `id:` links. `org-slipbox` also supports title-based
`slipbox:` links as a writing convenience.

When `org-slipbox-mode` is enabled, completion is active in eligible Org files.
You can also enable `org-slipbox-completion-mode` directly if you prefer the
granular setup path.

`org-slipbox` installs completion through `completion-at-point`, so the normal
Emacs completion stack applies:

- inside Org bracket links, completion inserts `slipbox:Title` links
- set `org-slipbox-completion-everywhere` to non-nil to complete outside links too
- set `org-slipbox-link-auto-replace` to non-nil if you want `slipbox:` links rewritten to stable `id:` links on save
- completion respects your configured `completion-styles`
- Corfu, Vertico, Company via `company-capf`, and other CAPF front-ends work through the same surface

The node chooser behind `org-slipbox-node-read` supports configurable display
templates, annotation hooks, and sorting/filtering hooks.

## Encrypted Files And Discovery

Encrypted Org files are part of the normal discovery story. Files ending in
`.org.gpg` or `.org.age` are eligible whenever their base extension matches the
configured discovery policy.

Capture templates can target encrypted notes directly:

```emacs-lisp
(setq org-slipbox-capture-templates
      '(("d" "default" plain "${body}"
         :target (file+head "notes/${slug}.org.gpg"
                            "#+title: ${title}\n"))))
```

The discovery policy is shared across indexing, autosync, dailies, and
discovery sections through:

- `org-slipbox-file-extensions`
- `org-slipbox-file-exclude-regexp`

One important expectation is the same as in `org-roam`: the SQLite database
stores indexed metadata in plain text. Encrypt the database separately if that
metadata is sensitive.

## Capture And Templates

`org-slipbox` treats capture as a first-class workflow, not as a thin wrapper
around ad hoc file edits.

- `org-slipbox-capture` starts a transient draft buffer
- `C-c C-c` finalizes the draft through the Rust RPC layer
- `C-c C-k` aborts the draft without mutating the target note
- `org-slipbox-node-find` and `org-slipbox-node-insert` reuse the same capture flow for new notes

Capture templates live in `org-slipbox-capture-templates`. They support the
same workflow shape `org-roam` users expect:

- typed content kinds: `entry`, `plain`, `item`, `checkitem`, `table-line`
- file targets: `file`, `file+head`, `file+olp`, `file+head+olp`, `file+datetree`
- existing-node targets with `(node ...)`
- lifecycle options such as `:immediate-finish`, `:jump-to-captured`, `:no-save`, and finalize hooks

Template expansion uses `${...}` placeholders. Common values include
`${title}`, `${slug}`, `${ref}`, `${body}`, `${annotation}`, and `${link}`.
Unknown placeholders prompt once and can take defaults with
`${name=default}`.

```emacs-lisp
(setq org-slipbox-capture-templates
      '(("d" "default" plain "${body}"
         :target (file+head "%<%Y%m%d%H%M%S>-${slug}.org"
                            "#+title: ${title}\n")
         :unnarrowed t)))
```

Ref-oriented capture uses `org-slipbox-capture-ref-templates`, which follows
the same syntax while adding `${ref}`, `${body}`, `${annotation}`, and
`${link}`.

This is an intentional divergence from `org-roam` only in architecture: the
draft/session layer lives in Emacs, but all target writes and target
preparation still happen behind Rust RPCs.

## org-protocol

`org-slipbox` keeps browser and external capture flows opt-in. Enable them with:

```emacs-lisp
(org-slipbox-protocol-mode 1)
```

This mode registers `roam-node` and `roam-ref` handlers with
`org-protocol`. The surrounding `org-protocol://` system integration still
belongs to your Emacs and operating-system setup; `org-slipbox` only owns the
handler registration.

- `roam-node` visits an indexed node by ID
- `roam-ref` finds or captures the canonical note for a ref

`roam-ref` uses `org-slipbox-capture-ref-templates`, so bookmarklet and browser
flows reuse the same capture semantics as normal ref capture.

## Dailies

Daily notes are configured independently from the main capture templates:

```emacs-lisp
(setq org-slipbox-dailies-directory "daily/")
(setq org-slipbox-dailies-capture-templates
      '(("d" "default" entry
         "* ${title}"
         :target (file+head "%<%Y-%m-%d>.org"
                            "#+title: %<%Y-%m-%d>\n"))))
```

When daily templates are configured, the interactive capture commands select
the template first and only prompt for `Daily entry:` when that template uses
title-derived placeholders such as `${title}` or `${slug}`. Fixed-content
templates can therefore capture directly without a meaningless heading prompt.

The main commands mirror the documented `org-roam-dailies` workflow:

- `org-slipbox-dailies-capture-today`
- `org-slipbox-dailies-goto-today`
- `org-slipbox-dailies-capture-yesterday`
- `org-slipbox-dailies-goto-yesterday`
- `org-slipbox-dailies-capture-tomorrow`
- `org-slipbox-dailies-goto-tomorrow`
- `org-slipbox-dailies-capture-date`
- `org-slipbox-dailies-goto-date`
- `org-slipbox-dailies-goto-previous-note`
- `org-slipbox-dailies-goto-next-note`
- `org-slipbox-dailies-find-directory`

`org-slipbox-dailies-map` provides a public prefix keymap for these commands
using the conventional bindings:

- `d` today
- `y` yesterday
- `t` tomorrow
- `n` capture today
- `f` next note
- `b` previous note
- `c` goto date
- `v` capture date
- `.` dailies directory

Calendar marking remains optional through `org-slipbox-dailies-calendar-mode`,
so dailies discovery does not bleed into startup or the main buffer hot path.

## Export And Graph

Stable HTML export is opt-in:

```emacs-lisp
(org-slipbox-export-mode 1)
```

This keeps Org ID-backed links aligned with exported HTML anchors without
changing ordinary editing or indexing behavior.

Graph generation is also optional and isolated:

- `org-slipbox-graph` renders and opens a graph
- `org-slipbox-graph-write-dot` writes the DOT source
- `org-slipbox-graph-write-file` renders directly to a file

With no prefix argument, `org-slipbox-graph` renders the global graph. With a
plain `C-u`, it renders the connected component around the node at point. With
a numeric prefix, it renders the bounded neighborhood around the node at point.

Useful graph options include:

- `org-slipbox-graph-executable`
- `org-slipbox-graph-viewer`
- `org-slipbox-graph-filetype`
- `org-slipbox-graph-node-url-prefix`
- `org-slipbox-graph-generation-hook`

For local graphs, the default node URL prefix uses `org-protocol`. For
published graphs, set `org-slipbox-graph-node-url-prefix` to a web URL prefix
and use `org-slipbox-graph-generation-hook` to copy the rendered artifact into
your publishing output.

## If You Use org-roam Today

The normal substitution story is local rewiring, not conceptual retraining.
Start by translating the setup you already know:

| org-roam | org-slipbox | Notes |
| --- | --- | --- |
| `org-roam-directory` | `org-slipbox-directory` | Same role: the note root. |
| `org-roam-db-location` | `org-slipbox-database-file` | Same role: SQLite index path. |
| `org-roam-db-autosync-mode` | `org-slipbox-mode` or `org-slipbox-autosync-mode` | `org-slipbox-mode` is the one-step integration path; `org-slipbox-autosync-mode` is the narrower sync-only equivalent. |
| `org-roam-completion-everywhere` | `org-slipbox-completion-everywhere` | Same meaning. |
| `(require 'org-roam-protocol)` | `(org-slipbox-protocol-mode 1)` | Protocol support stays opt-in and mode-owned. |
| `(require 'org-roam-export)` | `(org-slipbox-export-mode 1)` | Export support stays opt-in and mode-owned. |
| `org-roam-dailies-directory` | `org-slipbox-dailies-directory` | Same role. |
| `org-roam-dailies-capture-templates` | `org-slipbox-dailies-capture-templates` | Same workflow surface. |

For a typical setup, the translation looks like this:

```emacs-lisp
(setq org-slipbox-directory (file-truename "~/org"))
(setq org-slipbox-database-file
      (expand-file-name "org-slipbox.sqlite" user-emacs-directory))
(setq org-slipbox-completion-everywhere t)

(setq org-slipbox-dailies-directory "daily/")
(setq org-slipbox-dailies-capture-templates
      '(("d" "default" entry "* %?"
         :target (file+head "%<%Y-%m-%d>.org"
                            "#+title: %<%Y-%m-%d>\n"))))

(org-slipbox-mode 1)
```

Then run `M-x org-slipbox-sync` once and continue with the usual note loop.

These are the most common command-level equivalents:

| org-roam | org-slipbox | Notes |
| --- | --- | --- |
| `org-roam-db-sync` | `org-slipbox-sync` | Full rebuild/sync entry point. |
| `org-roam-node-find` | `org-slipbox-node-find` | Find existing node or start capture for a new one. |
| `org-roam-node-insert` | `org-slipbox-node-insert` | Insert an `id:` link or capture a new node. |
| immediate-insert wrapper | `org-slipbox-node-insert-immediate` | Same insert-link flow, but newly captured nodes commit without opening a draft. |
| `org-roam-capture` | `org-slipbox-capture` | Direct capture entry point. |
| `org-roam-buffer-toggle` | `org-slipbox-buffer-toggle` | Persistent current-node buffer that tracks point and keeps expensive discovery off the hot path. |
| `org-roam-buffer-display-dedicated` | `org-slipbox-buffer-display-dedicated` | Dedicated one-node buffer for the fuller context surface, including expensive discovery sections. |
| `org-roam-node-at-point` | `org-slipbox-node-at-point` | Indexed node lookup for the current location. |
| `org-roam-ref-find` | `org-slipbox-ref-find` | Indexed ref chooser. |
| `org-roam-ref-add` / `org-roam-ref-remove` | `org-slipbox-ref-add` / `org-slipbox-ref-remove` | Ref metadata editing. |
| `org-roam-dailies-*` | `org-slipbox-dailies-*` | Same workflow family. |
| `org-roam-graph` | `org-slipbox-graph` | Optional Graphviz surface. |

The important design differences are:

- `org-slipbox` treats Emacs as the client and Rust as the engine.
- Index freshness is explicit and incremental rather than hidden behind ambient state.
- Completion and query hot paths stay index-backed instead of materializing the corpus in Elisp.
- The context buffer preserves the workflow surface without depending on `magit-section`.
- `org-slipbox` keeps one active root and derived index per Emacs session.

## Optional Surfaces

These stay opt-in and isolated from startup:

- `org-slipbox-protocol-mode` for `roam-node` and `roam-ref`
- `org-slipbox-export-mode` for stable HTML export anchors
- `org-slipbox-graph` for Graphviz generation and viewing
- `org-slipbox-dailies-calendar-mode` for calendar marking

## Current Capabilities

- Scan an Org directory and build a SQLite index.
- Search indexed nodes through the Rust query engine.
- Search indexed raw text occurrences through a structured Rust query surface
  that returns file, location, preview, and owning-node metadata, with indexed
  literal matching for queries of three or more characters.
- Format interactive node candidates with configurable display templates or
  functions, including indexed modification-time and graph-count metadata.
- Read nodes through a single-step chooser with configurable filter, sort, annotation, and insertion-format hooks.
- Search nodes by aliases and tags stored in Org metadata.
- Resolve nodes by exact `ID`, exact title or alias, and current point location.
- Resolve nodes from indexed refs and citekeys through a dedicated ref chooser, and edit alias/ref/tag metadata from Emacs.
- Capture notes from refs without duplicating existing ref-backed nodes.
- Expand capture templates into exact file targets, optional file heads, outline-path targets, datetrees, and existing indexed nodes.
- Start note capture in a transient draft buffer, or commit prepared drafts directly with `:immediate-finish`, while only writing through Rust RPC.
- Support org-roam-style typed capture templates with `entry`, `plain`, `item`, `checkitem`, and `table-line` content.
- Honor capture lifecycle actions such as `:finalize`, `:jump-to-captured`, `:immediate-finish`, `:no-save`, and template finalize handlers.
- Display a persistent or dedicated context buffer with declared lenses, inline result explanations, pivot history, comparison groups, explicit trails, configurable ordered sections, postrender hooks, unique-backlink variants, and dedicated-buffer discovery sections.
- Save durable exploration artifacts for dedicated lens views, comparisons, full trails, and detached trail slices, then reload them back into the dedicated cockpit without reconstructing semantics in Elisp.
- Persist durable exploration artifacts outside the derived SQLite index so they survive schema rebuilds and do not pollute public note, search, ref, or graph surfaces.
- Expose narrow JSON-RPC operations for saved artifacts: `saveExplorationArtifact`, `explorationArtifact`, `listExplorationArtifacts`, `executeExplorationArtifact`, and `deleteExplorationArtifact`.
- Complete and follow title-based org-slipbox links, with optional rewrite to stable `id:` links.
- Refile either the active region or the current subtree between indexed notes, and extract subtrees into new promoted file notes.
- Query indexed agenda entries from scheduled and deadline planning lines.
- Register opt-in `org-protocol` handlers for `roam-node` and `roam-ref` browser capture flows.
- Create and visit daily notes, append daily entries, move between existing daily notes, and opt into calendar marking for existing daily files.
- Enable optional HTML export support so Org ID-backed targets keep stable exported anchors.
- Keep indexed state current across Org file saves, renames, deletes, and VC deletes through explicit modes.
- Apply a shared file-discovery policy across indexing, autosync, dailies, and discovery sections, with configurable extensions, exclude regexps, and `.gpg` / `.age` suffix handling.
- Export optional global or neighborhood Graphviz graphs from indexed `id:` links, with title shortening, link filtering, DOT output, rendered file generation, viewer integration, and optional `org-protocol` node URLs.

## Performance

`org-slipbox` ships with an explicit corpus benchmark harness instead of
relying on anecdotal scale claims.

- `cargo run --bin slipbox-bench -- check --profile ci` generates a deterministic corpus, measures full indexing, single-file incremental indexing, indexed search, backlinks, node-at-point lookup, agenda queries, workflow catalog discovery, discovered workflow execution, corpus-health audits, and batch Emacs benchmarks for the persistent tracking buffer, the dedicated comparison render path, and a guaranteed non-structure dedicated exploration render path rooted in the unresolved lens with trail state.
- `cargo run --bin slipbox-bench -- run --profile release --keep-corpus` runs the larger local profile and keeps the generated corpus under `target/bench/` for inspection.
- Benchmark profiles live in [`benches/profiles/ci.json`](/home/b/projects/org-slipbox/benches/profiles/ci.json) and [`benches/profiles/release.json`](/home/b/projects/org-slipbox/benches/profiles/release.json). Reports are written to `target/bench/`.
- The benchmark corpus includes discovered workflow specs plus explicit audit
  fixtures, so the workflow and audit gates measure real workbench behavior
  rather than empty catalog scans or cheap fallback paths.

This is an intentional divergence from the `org-roam` manual's performance
guidance. `org-slipbox` does not expose GC-tuning knobs for cache builds,
because the heavy parse/index/query path lives in Rust rather than in a large
Elisp caching pass. The replacement answer here is benchmarked behavior, not
more GC tuning variables.

## Durable Exploration Artifacts

`0.6.x` introduces the first workbench-foundation surface without pretending
that the whole programmable platform is done.

- In the dedicated cockpit, `s` saves the current lens view, comparison, full trail, or detached trail slice as a durable artifact.
- `o` asks the daemon to execute a saved artifact and then restores the dedicated session from the executed result.
- The daemon persists those artifacts outside the derived SQLite index, so rebuilds do not erase them and note-facing query surfaces do not start treating artifacts as notes.
- The machine-facing surface is deliberately narrow: `saveExplorationArtifact`, `explorationArtifact`, `listExplorationArtifacts`, `executeExplorationArtifact`, and `deleteExplorationArtifact`.
- `0.7.x` is the shipped step: first usable headless workbench commands over
  the canonical daemon boundary for live explore, compare, resolve, and
  artifact lifecycle.
- `0.8.x` is the shipped broader workbench step: named workflows,
  corpus-health audits,
  bounded workflow discovery from configured directories, report output
  surfaces, and stricter scale guarantees built on that same settled
  exploratory model.
- `0.9.x` is the next operational workbench step: durable review runs for
  repeated workflow and audit loops, explicit review status, review diffs, and
  safe remediation previews. Review records are not notes, and they are not
  saved exploration artifacts.
- Broad CLI families, extension APIs, MCP surfaces, and agent adapters are
  still deferred.

## Headless Workbench

`0.7.x` made the workbench genuinely usable outside Emacs. The current
`0.8.x` line composes that surface into named workflows, corpus-health audits,
bounded workflow discovery, and report outputs. The CLI stays on the same
architectural line as the rest of the project:

- every headless command talks to the daemon over canonical JSON-RPC stdio
- the CLI auto-spawns `slipbox serve` from the current executable unless you
  override it with `--server-program`
- `--json` is first-class for machine use
- the command surface is task-shaped rather than a thin wrapper over every RPC

The shipped headless commands are:

- `slipbox status`
- `slipbox resolve-node`
- `slipbox explore`
- `slipbox compare`
- `slipbox audit dangling-links`
- `slipbox audit duplicate-titles`
- `slipbox audit orphan-notes`
- `slipbox audit weakly-integrated-notes`
- `slipbox workflow list`
- `slipbox workflow show`
- `slipbox workflow run`
- `slipbox artifact list`
- `slipbox artifact show`
- `slipbox artifact run`
- `slipbox artifact export`
- `slipbox artifact import`
- `slipbox artifact delete`

All headless commands share the same scope arguments:

```bash
slipbox <command> \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --json
```

Workflow commands also accept repeatable `--workflow-dir` arguments. Discovery is
deliberately narrow:

- only top-level JSON workflow spec files are considered
- built-in workflows win over discovered ones
- earlier configured workflow directories win over later ones
- invalid or colliding discovered workflows are reported as workflow catalog
  issues without hiding the valid workflows that remain runnable

The built-in workflow IDs are:

- `workflow/builtin/context-sweep`
- `workflow/builtin/unresolved-sweep`
- `workflow/builtin/comparison-tension-review`

Examples:

Resolve an exact note target:

```bash
slipbox resolve-node \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --id left-id \
  --json
```

Run live exploration through the settled declared-lens model:

```bash
slipbox explore \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --title "Project X" \
  --lens dormant \
  --limit 25 \
  --json
```

Compare two notes and keep only the tension group:

```bash
slipbox compare \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --left-id left-id \
  --right-id right-id \
  --group tension \
  --json
```

List, inspect, and run named workflows discovered from configured directories:

```bash
slipbox workflow list \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --workflow-dir ~/.config/org-slipbox/workflows \
  --json

slipbox workflow show workflow/research/unresolved-sweep \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --workflow-dir ~/.config/org-slipbox/workflows \
  --json

slipbox workflow run workflow/research/unresolved-sweep \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --workflow-dir ~/.config/org-slipbox/workflows \
  --input focus=key:file:notes.org::42 \
  --json
```

Inspect a workflow spec JSON file locally without connecting to the daemon:

```bash
slipbox workflow show --spec ~/.config/org-slipbox/workflows/review.json --json
```

Write a workflow report as line-oriented JSON:

```bash
slipbox workflow run workflow/builtin/unresolved-sweep \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --input focus=key:file:notes.org::42 \
  --jsonl \
  --output unresolved-report.jsonl
```

Run corpus-health audits through the daemon:

```bash
slipbox audit dangling-links \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --limit 200 \
  --json

slipbox audit duplicate-titles \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --json

slipbox audit orphan-notes \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --jsonl \
  --output orphan-notes.jsonl

slipbox audit weakly-integrated-notes \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --limit 100 \
  --json
```

Save a live exploration or comparison as a durable artifact:

```bash
slipbox explore \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --key file:context.org::42 \
  --lens time \
  --save \
  --artifact-id artifact/current-time-slice \
  --artifact-title "Current time slice" \
  --artifact-summary "Anchor-scoped planning context" \
  --json
```

```bash
slipbox compare \
  --root ~/notes \
  --db ~/.cache/org-slipbox.sqlite \
  --left-id left-id \
  --right-id right-id \
  --group tension \
  --save \
  --artifact-id artifact/left-vs-right \
  --artifact-title "Left vs right" \
  --json
```

List, inspect, execute, export, import, and delete durable artifacts:

```bash
slipbox artifact list --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox artifact show artifact/left-vs-right --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox artifact run artifact/left-vs-right --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox artifact export artifact/left-vs-right --root ~/notes --db ~/.cache/org-slipbox.sqlite --json > left-vs-right.json
slipbox artifact import --root ~/notes --db ~/.cache/org-slipbox.sqlite --json left-vs-right.json
slipbox artifact delete artifact/left-vs-right --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
```

The JSON contracts are intentionally different where the semantics differ:

- `slipbox compare --save --json` returns `{ "result": ..., "artifact": ... }`
- `slipbox artifact run --json` returns `{ "artifact": ... }`, where that
  payload is the executed artifact shape
- `slipbox artifact export --json` emits raw saved-artifact JSON, and
  `slipbox artifact import --json` consumes that same raw saved-artifact JSON

This is now a broader composed research workbench surface, not the whole
platform. Named workflows and audits compose the settled live
explore/compare/artifact model; they do not introduce a broad CLI for every
RPC, an extension API, MCP, or an agent-adapter layer. Workflow discovery is a
bounded declarative mechanism for JSON specs in configured directories, not a
plugin runtime.

The next workbench line is operational rather than expansive. `0.9.x` should
make recurring workflow and audit runs durable, comparable, and reviewable:
what ran, what changed since the last run, what still needs attention, and
what remediation could be previewed safely before any write occurs. That
review state should live beside the composed workbench model without becoming
note identity, saved exploration-artifact identity, a task-manager clone, or a
general automation platform.

## FAQ

### More Than One Slipbox Root

`org-slipbox` currently assumes one active root and one active derived index per
Emacs session. That is an intentional divergence from `org-roam`'s
directory-local multi-root story: it keeps daemon state, autosync behavior, and
index freshness explicit instead of letting correctness depend on hidden
per-buffer root switching.

The intended layout is one `org-slipbox-directory` with subdirectories under
it. If you truly need separate slipboxes, use separate Emacs sessions or switch
the configured root deliberately before reconnecting and reindexing.

### Create A Note Whose Title Matches A Candidate Prefix

`org-slipbox-node-find` and `org-slipbox-node-insert` both allow fresh input
when the minibuffer text is not resolved to an indexed node. In practice, you
can type the exact new title you want and confirm it directly; `org-slipbox`
turns that fresh title into a normal capture flow instead of relying on
frontend-specific completion hacks.

### Stop Creating IDs Everywhere

`org-slipbox` assigns explicit IDs lazily. Existing notes do not get IDs unless
they become stable link targets, ref-backed notes, or other explicit identity
surfaces. This is the default design, not a workaround.

### Publish Notes With An Internet-Friendly Graph

Use `org-slipbox-graph-node-url-prefix` to emit web-facing node links instead
of local `org-protocol` links, then attach a function to
`org-slipbox-graph-generation-hook` to copy the rendered graph into your
publishing output.

## Development

Use `make test` for the current Rust and Emacs checks.
Use `make build` for the default bundled-SQLite release build.
Use `make build-system-sqlite` when you want to link against a system SQLite installation.
Use `make bench-check PROFILE=ci` for the repeatable corpus regression gate.
Use `make bench PROFILE=release` for the larger local benchmark profile.
