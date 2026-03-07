# org-slipbox

`org-slipbox` is a local-first Org knowledge engine with an Emacs front-end.

The project is structured to keep parsing, indexing, and query execution outside Emacs Lisp while preserving Org files as the source of truth. The Rust side owns the derived index and JSON-RPC protocol; the Elisp side owns user commands, integration with editing workflows, and presentation.

## Status

The repository is under active development. Work remains under `Unreleased` until the project reaches full replacement status for the workflows it targets.

## Development

Use `make test` for the current Rust and Emacs checks.
The Rust workspace links against a system SQLite installation.

## Current Capabilities

- Scan an Org directory and build a SQLite index.
- Search indexed nodes through the Rust query engine.
- Resolve backlinks for nodes with explicit Org `ID` properties.
- Capture new file notes with explicit IDs.
- Insert `id:` links after lazily assigning IDs to existing nodes.
- Sync saved Org buffers into the index through an explicit file-level RPC.
- Connect Emacs to the local daemon over JSON-RPC on stdio.
