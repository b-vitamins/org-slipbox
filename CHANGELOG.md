# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog, and this project will follow SemVer once it starts making releases.

## [Unreleased]

### Changed
- Split the Emacs client into focused command modules for nodes, links, capture, metadata, and structural editing.
- Centralized all JSON-RPC method names in `org-slipbox-rpc.el` and routed client calls through named RPC helpers.
- Moved metadata edits, subtree refile and extract, and single-file incremental reindexing behind Rust RPCs so the Emacs client only coordinates sync and buffer refresh.
- Codified runtime guardrails for load-time hooks, persistent-buffer discovery costs, and incremental file sync semantics.

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
- Added indexed scheduled, deadline, and closed timestamps plus an agenda query command.
- Added indexed `ROAM_REFS` support, ref lookup commands, and alias/ref metadata editing commands.
- Added indexed tag completion plus file-level and heading-level tag editing commands.
- Added daily-note discovery and next/previous navigation commands.
- Added exact node lookup helpers for IDs, exact title/alias matches, and point-based resolution.
- Added a persistent or dedicated context buffer showing current node metadata, refs, and backlinks.
- Added title-based org-slipbox links with indexed completion and rewrite-to-`id:` workflows.
- Added subtree extraction and refile commands plus file-node/subtree promote-demote helpers.
- Added dedicated-buffer reflink and unlinked-reference discovery sections without putting grep-backed work on the persistent redisplay path.
- Added indexed random-node lookup through a dedicated RPC path.
- Added opt-in daily-note calendar marking without installing calendar hooks at load time.
- Added ref-driven note capture that reuses existing ref nodes and writes refs transactionally for new notes.
- Added node-target capture for appending new child headings under existing indexed nodes.
- Added capture-target expansion for exact files, optional file heads, and outline-path targets.
- Added daily-note template support on top of the shared capture-target pipeline while preserving the legacy entry-level flow by default.
- Added contextual capture-template variables for refs, annotations, links, and protocol-supplied body text.
- Added an opt-in `org-protocol` mode for `roam-node` and `roam-ref` handlers backed by the shared capture pipeline.
- Added configurable node completion display templates and function-based candidate formatters.
- Added a public `org-slipbox-node-read` chooser with filter, sort, annotation, and insertion-format customization.
- Added a generic template-capture RPC and Rust write path for typed capture targets and content placement.
- Added org-roam-style typed capture templates, including datetree and existing-node targets plus `${...}` and `org-capture` body expansion.
- Added exact backlink locations plus preview-rich context-buffer and backlink views backed by indexed link occurrences.
- Added region-aware node insertion so selected text can be preserved through both existing-node and capture-and-insert flows.
