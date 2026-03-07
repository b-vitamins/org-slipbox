# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog, and this project will follow SemVer once it starts making releases.

## [Unreleased]

### Added
- Initialized the repository as a combined Rust and Emacs Lisp project.
- Added architectural guardrails, release policy, and verification conventions.
- Added a JSON-RPC daemon scaffold and an Emacs client scaffold.
- Added transactional SQLite indexing for Org files, node search, and backlink queries.
- Added interactive Elisp commands for indexing, node lookup, and backlink inspection.
- Added note capture and lazy ID assignment through the Rust write pipeline.
- Added Emacs commands for note capture and `id:` link insertion.
- Added a file-level sync RPC and an Emacs autosync mode driven by buffer saves.
- Added indexed alias and tag metadata for nodes, including search over those fields.
- Added daily-note commands backed by generic file-node and heading-append RPCs.
- Added configurable capture path and title templates backed by unique file-target capture.
