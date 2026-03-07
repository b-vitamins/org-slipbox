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
- Search nodes by aliases and tags stored in Org metadata.
- Resolve nodes by exact `ID`, exact title or alias, and current point location.
- Resolve nodes from indexed refs and citekeys, and edit alias/ref/tag metadata from Emacs.
- Capture notes from refs without duplicating existing ref-backed nodes.
- Display a persistent or dedicated context buffer for the current node with refs, backlinks, and dedicated-buffer reference discovery sections.
- Complete and follow title-based org-slipbox links, with optional rewrite to stable `id:` links.
- Refile subtrees between indexed notes and extract subtrees into new promoted file notes.
- Query indexed agenda entries from scheduled and deadline planning lines.
- Resolve backlinks for nodes with explicit Org `ID` properties.
- Visit a random indexed node without materializing the full graph in Emacs.
- Capture new file notes with explicit IDs and configurable path/title templates.
- Capture new child headings directly under existing indexed nodes.
- Create and visit daily notes, append daily entries, move between existing daily notes, and opt into calendar marking for existing daily files.
- Insert `id:` links after lazily assigning IDs to existing nodes.
- Sync saved Org buffers into the index through an explicit file-level RPC.
- Connect Emacs to the local daemon over JSON-RPC on stdio.
