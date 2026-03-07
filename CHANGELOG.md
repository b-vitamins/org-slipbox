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
