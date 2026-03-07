# org-slipbox

`org-slipbox` is a local-first Org knowledge engine with an Emacs front-end.

The project is structured to keep parsing, indexing, and query execution outside Emacs Lisp while preserving Org files as the source of truth. The Rust side owns the derived index and JSON-RPC protocol; the Elisp side owns user commands, integration with editing workflows, and presentation.

## Status

The repository is under active development. Work remains under `Unreleased` until the project reaches full replacement status for the workflows it targets.

## Development

Use `make test` for the current Rust and Emacs checks.
