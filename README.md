# org-slipbox

`org-slipbox` is a local-first Org knowledge engine with an Emacs front-end.

The project is structured to keep parsing, indexing, and query execution outside Emacs Lisp while preserving Org files as the source of truth. The Rust side owns the derived index and JSON-RPC protocol; the Elisp side owns user commands, integration with editing workflows, and presentation.

## Status

The repository is under active development. Work remains under `Unreleased` until the project reaches full replacement status for the workflows it targets.

## Development

Use `make test` for the current Rust and Emacs checks.
The Rust workspace links against a system SQLite installation.

## Guardrails

- Emacs Lisp is the client, not the engine: parsing, indexing, ranking, and structural writes stay behind Rust RPCs.
- Persistent context-buffer redisplay must stay cheap; grep-backed discovery belongs only in dedicated or explicit paths.
- File-level incremental sync must update one file without pruning unrelated indexed notes.
- Loading the package must not install global hooks; optional modes own their hook lifecycles explicitly.

## Current Capabilities

- Scan an Org directory and build a SQLite index.
- Search indexed nodes through the Rust query engine.
- Format interactive node candidates with configurable display templates or functions.
- Read nodes through a single-step chooser with configurable filter, sort, annotation, and insertion-format hooks.
- Search nodes by aliases and tags stored in Org metadata.
- Resolve nodes by exact `ID`, exact title or alias, and current point location.
- Resolve nodes from indexed refs and citekeys through a dedicated ref chooser, and edit alias/ref/tag metadata from Emacs.
- Capture notes from refs without duplicating existing ref-backed nodes.
- Expand capture templates into exact file targets, optional file heads, and outline-path targets.
- Prepare capture targets consistently across exact files, file heads, outline paths, datetrees, and existing indexed nodes.
- Expand capture templates with contextual variables such as refs and protocol-supplied body text.
- Start note capture in a transient draft buffer, or commit prepared drafts directly with `:immediate-finish`, while only writing through Rust RPC.
- Coordinate capture with live target note buffers by saving/reindexing them before Rust-backed writes and refreshing them afterward.
- Support org-roam-style typed capture templates with `entry`, `plain`, `item`, `checkitem`, and `table-line` content, including explicit `:table-line-pos` placement.
- Honor capture lifecycle actions such as `:finalize`, `:jump-to-captured`, `:immediate-finish`, `:no-save`, and template finalize handlers.
- Honor capture buffer-lifecycle behavior such as `:kill-buffer`, without closing note buffers that were already open before capture started.
- Honor capture view-state and clock lifecycle options such as `:unnarrowed`, `:clock-in`, `:clock-resume`, and `:clock-keep`.
- Materialize unsaved capture previews through Rust so dirty live target buffers stay unsaved and insert-link flows can still use explicit IDs.
- Capture into datetree targets and existing indexed nodes through the shared Rust write pipeline.
- Display a persistent or dedicated context buffer for the current node with configurable ordered sections, postrender hooks, unique-backlink variants, and dedicated-buffer reference discovery sections.
- Complete and follow title-based org-slipbox links, with optional rewrite to stable `id:` links.
- Refile either the active region or the current subtree between indexed notes, and extract subtrees into new promoted file notes.
- Promote a single root-heading note into a file node, or demote a file node into a single root heading, through Rust-backed rewrite commands.
- Query indexed agenda entries from scheduled and deadline planning lines.
- Resolve backlinks for nodes with explicit Org `ID` properties.
- Visit a random indexed node without materializing the full graph in Emacs.
- Capture new file notes with explicit IDs and configurable path/title templates.
- Reuse the same target-template system for daily-note capture when configured.
- Register opt-in `org-protocol` handlers for `roam-node` and `roam-ref` browser capture flows.
- Capture new child headings directly under existing indexed nodes.
- Create and visit daily notes, append daily entries, move between existing daily notes, and opt into calendar marking for existing daily files.
- Insert `id:` links after lazily assigning IDs to existing nodes.
- Enable optional HTML export support so Org ID-backed targets keep stable exported anchors.
- Keep indexed state current across Org file saves, renames, deletes, and VC deletes through an explicit autosync mode.
- Apply a shared file-discovery policy across indexing, autosync, dailies, and grep-backed discovery, with configurable extensions, exclude regexps, and `.gpg` / `.age` suffix handling.
- Bridge `org-id` through an explicit compatibility mode so indexed IDs win over stale `org-id-locations`, while valid non-indexed IDs can still be refreshed into `org-id`.
- Inspect and maintain the index through explicit sync/rebuild commands, node/file diagnostics, file drift reports, and an opt-in SQLite explorer.
- Export optional global or neighborhood Graphviz graphs from indexed `id:` links, with title shortening, link filtering, DOT output, rendered file generation, viewer integration, and optional `org-protocol` node URLs.
- Connect Emacs to the local daemon over JSON-RPC on stdio.
