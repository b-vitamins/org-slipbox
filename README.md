# org-slipbox

`org-slipbox` is an Emacs package and companion Rust daemon for personal
knowledge management in plain Org files.

Org files are the source of truth. A derived SQLite index makes search,
backlinks, refs, agenda queries, graph export, exploration, review loops, and
daemon-owned writes fast enough for interactive use. Emacs owns editing UI,
session state, and presentation. Rust owns parsing, indexing, ranking, query
execution, and file mutation.

The latest shipped release is `0.12.0`, the structural editing and
stabilization line. Current development is the `0.13.0` consolidation line:
compact docs, clearer command taxonomy, and internal refactors without new
public product nouns.

For the product model, see [doc/model.org](doc/model.org). The short version
is:

- `Notes`: Org files, file nodes, heading anchors, metadata, capture, dailies,
  and structural edits.
- `Relations`: links, refs, tags, backlinks, forward links, occurrence search,
  agenda entries, and graph edges.
- `Explorations`: lenses, comparisons, trails, explanations, and saved
  exploration artifacts.
- `Reviews`: audits, review runs, status, diffs, remediation previews, and
  bounded remediation apply records.
- `Assets`: workflow specs, review routines, report profiles, and workbench
  packs.
- `System`: daemon/runtime state, sync, indexed files, diagnostics,
  compatibility policy, and benchmark profiles.

Boundary statement: daemon-backed work goes through `slipbox serve` over
JSON-RPC stdio; the CLI and Emacs package expose task-shaped operations rather
than raw transport sprawl, plugin runtime, MCP implementation, or agent
adapter surfaces.

## Requirements

- Emacs `29.1` or newer
- a `slipbox` daemon binary, from a release archive or a local source build
- Graphviz only for optional graph rendering
- `org-protocol` only for optional browser/protocol capture handlers

## Product Docs

- [doc/model.org](doc/model.org) defines the compact public model and command
  taxonomy.
- [doc/compatibility.org](doc/compatibility.org) defines compatibility and
  deprecation policy.
- [doc/vision.org](doc/vision.org) states the product direction.
- [doc/milestones.org](doc/milestones.org) describes durable capability
  milestones.
- [doc/roadmap.org](doc/roadmap.org) maps near-term release bands.

## Installation

`org-slipbox` ships as one repository containing the Emacs package and the Rust
daemon/CLI. The two parts stay separable: the root `.el` files can be loaded as
an Emacs package, and the daemon is an ordinary `slipbox` executable on `PATH`
or at `org-slipbox-server-program`.

### Install The Daemon

Tagged releases publish platform-specific `slipbox` archives through GitHub
Actions. Unpack the archive somewhere on `PATH`, or point
`org-slipbox-server-program` at the unpacked binary.

To build from source with bundled SQLite:

```bash
make build
```

This is equivalent to:

```bash
cargo build --release --locked
```

To install the built daemon on `PATH`:

```bash
make install-daemon
```

This is equivalent to:

```bash
cargo install --path . --locked
```

Packagers who want system SQLite can use:

```bash
make build-system-sqlite
make install-daemon-system-sqlite
```

Those targets use:

```bash
cargo build --release --locked --no-default-features --features system-sqlite
cargo install --path . --locked --no-default-features --features system-sqlite
```

The `system-sqlite` path expects a discoverable SQLite development
installation. The default bundled build is the normal path when that toolchain
is not available.

### Development Shell

The repository includes [manifest.scm](manifest.scm) as an optional Guix
development convenience:

```bash
guix shell -m manifest.scm
```

Matching Makefile wrappers are available:

```bash
make guix-build
make guix-build-system-sqlite
make guix-test
make guix-lint-rust
make guix-bench-check
```

These wrappers are contributor conveniences. Release binaries, source builds,
and the installed `slipbox` executable remain the primary user paths.

### Install The Emacs Package

Add the repository root to `load-path` and require `org-slipbox`:

```emacs-lisp
(add-to-list 'load-path "/path/to/org-slipbox")
(require 'org-slipbox)
```

Then configure the note root and database:

```emacs-lisp
(setq org-slipbox-directory (file-truename "~/notes"))
(setq org-slipbox-database-file
      (expand-file-name "org-slipbox.sqlite" user-emacs-directory))
(org-slipbox-mode 1)
```

If `slipbox` is not on `PATH`, also set:

```emacs-lisp
(setq org-slipbox-server-program
      "/path/to/org-slipbox/target/release/slipbox")
```

`org-slipbox-mode` enables:

- `org-slipbox-autosync-mode` for save, rename, delete, and VC-delete index
  updates.
- `org-slipbox-id-mode` so indexed IDs cooperate with `org-id`.
- `org-slipbox-completion-mode` in eligible Org buffers under
  `org-slipbox-directory`.

Loading `org-slipbox` alone does not install hooks or mutate user state. The
integration starts when you enable the mode or one of its narrower component
modes.

## First Run

Build the initial index:

```text
M-x org-slipbox-sync
```

Then run the core note loop:

1. `M-x org-slipbox-node-find`
2. Enter a new title and press `RET`.
3. Finalize the draft with `C-c C-c`, or abort with `C-c C-k`.
4. Insert a link from another note with `M-x org-slipbox-node-insert`.
5. Open context with `M-x org-slipbox-buffer-toggle`.
6. Use `M-x org-slipbox-buffer-display-dedicated` for the fuller exploratory
   cockpit.

The first sync builds the database. After that, autosync keeps changed files
current incrementally.

## Everyday Emacs Use

The main Emacs commands are:

- `org-slipbox-node-find`: visit an indexed node or capture a new note.
- `org-slipbox-node-insert`: insert an `id:` link or capture a new target.
- `org-slipbox-node-insert-immediate`: insert and immediately commit newly
  captured nodes.
- `org-slipbox-capture`: start the capture flow directly.
- `org-slipbox-buffer-toggle`: show the cheap persistent current-node context.
- `org-slipbox-buffer-display-dedicated`: open the richer one-node cockpit.

File nodes and heading nodes are both first-class. Explicit IDs remain the
stable identity surface, and IDs can be assigned lazily when an existing note
becomes a stable link target.

### Context And Exploration

The persistent buffer keeps cheap indexed sections on the hot path. The
dedicated buffer is the richer cockpit for declared lenses, comparisons,
trails, explanations, and saved artifacts.

The dedicated buffer supports:

- lenses: `structure`, `refs`, `time`, `tasks`, `bridges`, `dormant`, and
  `unresolved`
- pivot history with `[` and `]`
- frozen-root toggling with `f`
- comparison with `c`, `C`, and `g`
- trails with `a`, `{`, `}`, and `T`
- artifact save/load with `s` and `o`
- explanation blocks for non-obvious results

Buffer sections are configurable:

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

### Metadata, Refs, Links, And Completion

File-node titles come from `#+title`; heading-node titles come from headings.
Aliases, tags, refs, planning dates, and task states are indexed.

Common metadata commands:

- `org-slipbox-alias-add` / `org-slipbox-alias-remove`
- `org-slipbox-tag-add` / `org-slipbox-tag-remove`
- `org-slipbox-ref-add` / `org-slipbox-ref-remove`
- `org-slipbox-ref-find`

`ROAM_REFS` may contain URLs, citation keys, or multiple refs for one note.
Org-cite forms such as `[cite:@key]` and org-ref forms such as `cite:key`
normalize into the same ref surface.

`ROAM_EXCLUDE` excludes file or heading nodes from the derived index, except
the literal value `nil`, which clears an inherited exclusion.

Completion uses `completion-at-point`:

- inside Org links, completion inserts `slipbox:Title` links
- `org-slipbox-completion-everywhere` enables completion outside links
- `org-slipbox-link-auto-replace` rewrites `slipbox:` links to stable `id:`
  links on save
- ordinary Emacs completion front-ends work through the same CAPF surface

### Capture, Dailies, Protocol, Export, And Graph

Capture templates live in `org-slipbox-capture-templates` and support:

- content kinds: `entry`, `plain`, `item`, `checkitem`, `table-line`
- targets: `file`, `file+head`, `file+olp`, `file+head+olp`, `file+datetree`,
  and existing `(node ...)`
- lifecycle options such as `:immediate-finish`, `:jump-to-captured`,
  `:no-save`, and finalize hooks
- placeholders such as `${title}`, `${slug}`, `${ref}`, `${body}`,
  `${annotation}`, and `${link}`

Example:

```emacs-lisp
(setq org-slipbox-capture-templates
      '(("d" "default" plain "${body}"
         :target (file+head "%<%Y%m%d%H%M%S>-${slug}.org"
                            "#+title: ${title}\n")
         :unnarrowed t)))
```

Daily notes use `org-slipbox-dailies-directory` and
`org-slipbox-dailies-capture-templates`. The main dailies commands are
`org-slipbox-dailies-goto-*`, `org-slipbox-dailies-capture-*`, and
`org-slipbox-dailies-find-directory`.

Optional surfaces stay opt-in:

- `org-slipbox-protocol-mode` registers `roam-node` and `roam-ref`
  `org-protocol` handlers.
- `org-slipbox-export-mode` keeps Org ID-backed links aligned with exported
  HTML anchors.
- `org-slipbox-graph` renders Graphviz graphs.
- `org-slipbox-dailies-calendar-mode` marks existing daily files in the
  calendar.

Encrypted Org files ending in `.org.gpg` or `.org.age` are eligible when their
base extension matches the configured discovery policy. Indexed metadata in
SQLite remains plaintext; encrypt the database separately if needed.

## CLI Surface

The CLI is the scriptable front-end for the same daemon-owned model. Use Emacs
for interactive editing, completion UI, draft capture buffers, live cockpit
navigation, calendar UI, and viewer hooks. Use the CLI for repeatable sync,
lookup, search, graph export, note creation, capture, dailies, reviews,
assets, diagnostics, and structural writes.

Daemon-backed commands share these scope arguments:

```bash
slipbox <command> --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
```

Daemon-backed commands auto-spawn `slipbox serve` from the current executable
unless `--server-program` is provided. Local inspection commands such as
`slipbox workflow show --spec` and `slipbox pack validate` do not require
daemon scope.

The top-level command families follow the public model:

| Bucket | Commands |
| --- | --- |
| Notes | `node`, `note`, `capture`, `daily`, `edit`, `resolve-node` |
| Relations | `ref`, `tag`, `search`, `agenda`, `graph`, `link` |
| Explorations | `explore`, `compare`, `artifact` |
| Reviews | `audit`, `review` |
| Assets | `workflow`, `routine`, `pack` |
| System | `serve`, `status`, `sync`, `file`, `diagnose` |

Use `slipbox --help`, `slipbox <family> --help`, and
`slipbox <family> <command> --help` as the command reference. Help output now
documents target selectors, local-vs-daemon behavior, write safety, report
output modes, and JSON intent. Representative commands:

```bash
slipbox sync root --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox node show --id project-alpha --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox node search planning --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox ref resolve cite:smith2026 --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox agenda today --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox note create --title "Project Alpha" --file notes/project-alpha.org --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox capture preview --file inbox.org --title Inbox --type plain --content "Draft only" --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
```

Writes return read-your-writes results after daemon-owned index refresh. For
structural edits, JSON output is a `StructuralWriteReport` with changed files,
removed files, refreshed-index status, and any resulting node or anchor.

Exploration, review, and asset examples:

```bash
slipbox explore --title "Project X" --lens dormant --limit 25 --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox compare --left-id left-id --right-id right-id --group tension --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox workflow list --workflow-dir ~/.config/org-slipbox/workflows --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox workflow run workflow/builtin/unresolved-sweep --input focus=title:Project --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox audit dangling-links --limit 200 --save-review --review-id review/dangling/current --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox pack validate ~/.config/org-slipbox/packs/research-review.json --json
```

Preview/apply pairs keep mutation explicit:

```bash
slipbox link rewrite-slipbox preview --file notes/project.org --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
slipbox review remediation preview review/dangling/current audit/dangling/source/missing-id --root ~/notes --db ~/.cache/org-slipbox.sqlite --json
```

## Workbench Data

The daemon persists three durable JSON side stores beside the database:

- exploration artifacts: saved lenses, comparisons, trails, and detached trail
  slices
- review runs: saved audit/workflow evidence, status, diffs, and remediation
  evidence
- workbench packs: portable workflows, review routines, and report profiles

These stores survive index rebuilds and do not become notes, search hits, refs,
or graph nodes. See [doc/model.org](doc/model.org) for ownership and
[doc/compatibility.org](doc/compatibility.org) for compatibility policy.

Workflow specs and workbench packs carry compatibility metadata. Supported
version is `1`; future versions are rejected before typed parsing attempts to
interpret future syntax.

`--json` returns stable task-shaped objects. `--jsonl` is available for report
surfaces that stream line-oriented machine output. Command-specific details
belong in `slipbox <command> --help`.

## If You Use org-roam Today

The normal migration path is local rewiring:

| org-roam | org-slipbox |
| --- | --- |
| `org-roam-directory` | `org-slipbox-directory` |
| `org-roam-db-location` | `org-slipbox-database-file` |
| `org-roam-db-autosync-mode` | `org-slipbox-mode` or `org-slipbox-autosync-mode` |
| `org-roam-completion-everywhere` | `org-slipbox-completion-everywhere` |
| `(require 'org-roam-protocol)` | `(org-slipbox-protocol-mode 1)` |
| `(require 'org-roam-export)` | `(org-slipbox-export-mode 1)` |
| `org-roam-dailies-directory` | `org-slipbox-dailies-directory` |
| `org-roam-dailies-capture-templates` | `org-slipbox-dailies-capture-templates` |

Common command equivalents:

| org-roam | org-slipbox |
| --- | --- |
| `org-roam-db-sync` | `org-slipbox-sync` |
| `org-roam-node-find` | `org-slipbox-node-find` |
| `org-roam-node-insert` | `org-slipbox-node-insert` |
| `org-roam-capture` | `org-slipbox-capture` |
| `org-roam-buffer-toggle` | `org-slipbox-buffer-toggle` |
| `org-roam-buffer-display-dedicated` | `org-slipbox-buffer-display-dedicated` |
| `org-roam-ref-find` | `org-slipbox-ref-find` |
| `org-roam-dailies-*` | `org-slipbox-dailies-*` |
| `org-roam-graph` | `org-slipbox-graph` |

The important architectural difference is that the heavy parse/index/query and
write paths live in Rust rather than in Emacs Lisp.

## Performance

`org-slipbox` ships with a corpus benchmark harness instead of relying on
anecdotal scale claims.

```bash
cargo run --bin slipbox-bench -- check --profile ci
cargo run --bin slipbox-bench -- run --profile release --keep-corpus
```

Benchmark profiles live in [benches/profiles/ci.json](benches/profiles/ci.json)
and [benches/profiles/release.json](benches/profiles/release.json). Reports are
written under `target/bench/`.

The generated corpus includes workflow specs, audit fixtures, review fixtures,
imported pack/routine/report-profile fixtures, everyday CLI fixtures,
structural-write fixtures, remediation fixtures, and link-rewrite fixtures, so
the gates measure real paths rather than empty catalog scans.

## Stability And Compatibility

`org-slipbox` is pre-`1.0`, but public CLI JSON, JSON-RPC params/results,
durable records, declarative assets, structural write reports, diagnostics,
link rewrite records, remediation records, and benchmark profiles are
compatibility-sensitive.

The authoritative policy is [doc/compatibility.org](doc/compatibility.org).
The short rule is: evolve public machine contracts additively where possible,
document non-additive changes, and treat the derived SQLite schema as an
internal rebuildable index.

See [CHANGELOG.md](CHANGELOG.md) for shipped history.

## FAQ

### More Than One Slipbox Root

`org-slipbox` assumes one active root and one derived index per Emacs session.
Use subdirectories under one root, or switch root/database deliberately before
reconnecting and reindexing.

### Create A Note Whose Title Matches A Candidate Prefix

`org-slipbox-node-find` and `org-slipbox-node-insert` both allow fresh input
when minibuffer text does not resolve to an indexed node. Type the exact new
title and confirm it directly.

### Stop Creating IDs Everywhere

IDs are assigned lazily. Existing notes do not get IDs unless they become
stable link targets, ref-backed notes, or other explicit identity surfaces.

### Publish Notes With An Internet-Friendly Graph

Set `org-slipbox-graph-node-url-prefix` to emit web-facing node links, then use
`org-slipbox-graph-generation-hook` to copy the rendered graph into your
publishing output.

## Development

Use:

```bash
make test
make build
make build-system-sqlite
make bench-check PROFILE=ci
make bench PROFILE=release
```

Before milestone commits, run the relevant Rust, Elisp, and benchmark checks
for the touched surfaces.
