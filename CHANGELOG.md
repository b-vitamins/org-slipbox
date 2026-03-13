# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog, and this project will follow SemVer once it starts making releases.

## [Unreleased]

### Added
- Added durable product documents in `doc/` for the project's vision,
  capability milestones, and release-band roadmap so future work can stay
  anchored to the slip-box-as-conversation-partner thesis without relying on
  transient planning notes.
- Added a first-class `org-slipbox-node-insert-immediate` command so
  insert-link capture flows can commit newly created nodes directly without
  downstream template rebinding tricks.
- Added a public `org-slipbox-dailies-map` so downstream configs can bind the
  documented dailies workflow through one stable prefix keymap instead of
  recreating the command surface locally.
- Added a first-class `slipbox/forwardLinks` query and thin Emacs consumer so
  outgoing links can be queried as structured indexed records rather than
  reconstructed from graph output or other incidental surfaces.
- Added a dedicated `searchFiles` query plus thin Emacs helpers for indexed
  file records, including indexed file title, mtime, and node-count metadata
  for future frontend consumers.
- Added a first-class indexed `slipbox/searchOccurrences` query plus a thin
  Emacs helper so frontend text-search surfaces can resolve structured
  occurrence hits without shelling out from Emacs, with indexed literal
  matching for queries of three or more characters.
- Added a first-class `slipbox/reflinks` query and dedicated-buffer adoption so
  ref occurrences are resolved in Rust as structured source-node hits instead
  of shelling out to `rg`.
- Added a first-class `slipbox/unlinkedReferences` query and dedicated-buffer
  adoption so title and alias mention discovery now runs as a structured Rust
  query with explicit subtree and linked-occurrence exclusion rules.

### Changed
- Clarified `AGENTS.md` and `README.md` so durable strategy documents belong in
  `doc/*.org`, while temporary planning remains out of the repository.
- Clarified the current-node buffer docs so the persistent point-tracking
  buffer and the dedicated fuller inspection buffer are described as distinct
  entry points with different discovery-cost expectations.
- Extended indexed node query payloads to include file modification time plus
  backlink and forward-link counts, so frontend consumers can rely on
  engine-backed metadata instead of filesystem stats or local graph counting.
- Expanded the benchmark and regression gates for the `0.2.0` read/query
  surfaces, including explicit sorted-node-search benchmark coverage alongside
  the new daemon-backed graph, file, discovery, and occurrence query paths.
- Added daemon-owned `ROAM_EXCLUDE` compatibility semantics for file and
  heading nodes, including inherited exclusion plus explicit `nil` clearing,
  while keeping file-level discovery and `org-id` fallback orthogonal to node
  membership.
- Extended the node chooser template and annotation surface so candidate
  formatting can use indexed file modification time plus backlink and
  forward-link counts without local filesystem stats.
- Switched the dedicated buffer to the daemon-backed forward-links, reflinks,
  and unlinked-reference surfaces, and render indexed file mtime plus graph
  counts in the node summary without local filesystem stats.
- Split capture-template preview payloads away from indexed `NodeRecord`
  semantics by returning an explicit `preview_node` shape for unsaved preview
  materialization.

### Fixed
- Fixed dailies capture so interactive commands select daily templates before
  prompting for `Daily entry:`, and fixed-content templates no longer require a
  meaningless heading when they do not consume title-derived placeholders.

### Removed
- Removed the legacy `file-atime` node chooser sort from `org-slipbox-node-read`
  so supported named sorts now align with the daemon-backed `searchNodes` sort
  contract.

## [0.1.0] - 2026-03-08

### Changed
- Tightened the release-candidate framing in the README and package metadata so
  `org-slipbox` describes itself directly as personal knowledge management with
  interconnected Org notes, without translation trivia or developer-oriented
  jargon.
- Expanded the README into a more manual-like guide for capture templates, `org-protocol`, dailies, export, graph usage, benchmark-based performance guidance, and the remaining adoption-relevant FAQ entries.
- Expanded the README into a more manual-like guide for the current-node buffer, metadata and ref workflows, CAPF-based completion, and encrypted/discovery expectations, including explicit notes on intentional divergences from org-roam.
- Expanded the README's org-roam substitution section into a concrete setup-and-command rewiring map, so switching no longer depends on inferring variable renames or optional mode ownership.
- Reworked the installation story around a clean split between the Emacs package and the `slipbox` daemon, with binary-first and source-build paths described explicitly and without assuming a checkout-local daemon path.
- Changed the default Rust build to use bundled SQLite while keeping an explicit `system-sqlite` feature for packagers and source builds that want system linkage.
- Reworked the README around source installation, explicit setup paths, first-run workflows, and common org-roam command mapping so adoption no longer depends on tribal knowledge.
- Split the Emacs client into focused command modules for nodes, links, capture, metadata, and structural editing.
- Centralized all JSON-RPC method names in `org-slipbox-rpc.el` and routed client calls through named RPC helpers.
- Moved metadata edits, subtree refile and extract, and single-file incremental reindexing behind Rust RPCs so the Emacs client only coordinates sync and buffer refresh.
- Codified runtime guardrails for load-time hooks, persistent-buffer discovery costs, and incremental file sync semantics.
- Separated discovery policy from the JSON-RPC transport so daemon startup, file eligibility, and maintenance diagnostics now share one dedicated policy surface.
- Split node completion, visit/buffer coordination, and insertion glue into focused Elisp modules so `org-slipbox-node.el` remains the public facade instead of the next client monolith.
- Moved persistent context-buffer hook ownership into an explicit mode so the global redisplay lifecycle is mode-controlled rather than command-managed.
- Split `slipbox-store` schema/migration and index sync/delete flows into dedicated Rust modules so the store facade no longer mixes query surfaces with mutation and pruning logic.
- Split `slipbox-store` query families into focused Rust modules for nodes, refs, backlinks, agenda, and admin surfaces so new read paths no longer enlarge one store monolith.
- Split the Rust Org rewrite engine into explicit document submodules for outline traversal, property and keyword mutation, and block/render helpers so structural editing no longer accumulates in one internal file.
- Centralized daemon post-write reconciliation and preview-node recovery in `ServerState` so write handlers no longer sequence index sync, deleted-path removal, or rendered preview rescans themselves.

### Added
- Added an optional `manifest.scm` plus `make guix-*` convenience targets so contributors can enter a complete Guix development shell, including packaged `emacs-org-roam` for reproducible comparison runs, without changing the normal source-build or release-binary paths.
- Added actionable daemon startup diagnostics in `org-slipbox-rpc.el` so missing or non-executable `slipbox` binaries fail with direct installation guidance.
- Added `make build` and `make build-system-sqlite` targets for the two supported source-build paths.
- Added a GitHub Actions release workflow that builds platform `slipbox` binaries, archives them with the GPL license text, and publishes release assets plus checksums.
- Added `org-slipbox-mode` as an explicit single-mode integration surface that owns autosync, the `org-id` bridge, and buffer-local completion in eligible Org files.
- Initialized the repository as a combined Rust and Emacs Lisp project.
- Added architectural guardrails, release policy, and verification conventions.
- Added a JSON-RPC daemon scaffold and an Emacs client scaffold.
- Added transactional SQLite indexing for Org files, node search, and backlink queries.
- Added interactive Elisp commands for indexing, node lookup, and backlink inspection.
- Added note capture and lazy ID assignment through the Rust write pipeline.
- Added Emacs commands for note capture and `id:` link insertion.
- Added a file-level sync RPC and an explicit Emacs autosync mode that keeps indexed state correct across saves, renames, deletes, and VC deletes.
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
- Added whole-buffer file-node promote and demote commands through Rust-backed rewrite RPCs.
- Added an opt-in HTML export module so Org ID targets round-trip correctly during export.
- Added capture finalizers, jump-to-captured support, and insert-link lifecycle handling for org-roam-style templates.
- Added transient capture-session drafts with finalize and abort flows, while keeping all target writes behind the Rust capture RPC.
- Added live target-buffer coordination for capture so modified note buffers are saved and reindexed before Rust-backed capture writes, then refreshed afterward.
- Added `:kill-buffer` capture parity so capture-opened target buffers are cleaned up after finalization without touching buffers that were already open.
- Added `:unnarrowed`, `:clock-in`, `:clock-resume`, and `:clock-keep` capture parity on top of the draft-based capture lifecycle.
- Added honest `:no-save` capture parity through Rust-backed preview materialization, dirty live-target coordination, preview-node ID handling for insert-link flows, and upstream-compatible `:kill-buffer` save precedence.
- Added target-preparation parity for capture drafts across file, outline, datetree, and node targets, including explicit `table-line` placement semantics and clear errors for unsupported target options.
- Added immediate-finish capture, ordered finalize handlers, and explicit lifecycle validation so capture templates now either run real finalize/abort behavior or fail clearly.
- Added a shared file-discovery policy with configurable extensions, exclude regexps, encrypted Org suffix handling, and public `org-slipbox-file-p` / `org-slipbox-list-files` helpers.
- Added an explicit `org-id` bridge mode plus `org-slipbox-update-org-id-locations`, so indexed IDs override stale `org-id-locations` while valid excluded targets remain compatible with `org-id`.
- Added Rust-backed active-region refile support with the same indexed sync and source-cleanup guarantees as subtree refile, including empty-source file removal when the moved region consumes the whole note.
- Added a dedicated indexed ref chooser with annotation hooks, minibuffer history, prompt customization, and `org-slipbox-ref-find` integration.
- Added configurable context-buffer section composition with ordered section specs, a postrender hook, section filtering, and real unique-backlink queries.
- Added explicit maintenance commands for full sync/rebuild, current-file sync, node/file diagnostics, file drift inspection, and SQLite database exploration.
- Added an optional Graphviz export backend that generates DOT from indexed links, supports global or neighborhood graphs, shortens long titles, filters indexed link types, and writes DOT or rendered graph files.
- Added viewer-facing graph integration with post-generation hooks and optional `org-protocol` node URLs for rendered Graphviz output.
- Added a deterministic corpus benchmark harness with named profiles, JSON reports, threshold checks, and a batch Emacs benchmark for the persistent context-buffer redisplay path.
- Added real top-level autoloads for the optional export and graph entry points, so source-loaded installations can enable those documented surfaces immediately after `(require 'org-slipbox)`.

### Fixed
- Updated the GitHub Actions release workflow to use a supported Intel macOS
  runner label and unique per-matrix artifact names so tagged release builds
  can publish all configured binary artifacts.
- Fixed JSON-RPC request normalization for list-valued params such as aliases, tags, and refs, so real metadata edits and ref-backed captures no longer fail in fresh user sessions.
- Fixed dedicated-buffer reflink and unlinked-reference discovery so ripgrep commands are executed exactly once and shell stderr does not leak into parsed result rows.
- Fixed default graph export params so empty hidden-link-type settings are sent as an empty sequence instead of JSON null, restoring the optional graph surface in real use.
- Fixed blank-heading `entry` captures so org-roam-style `* %?` dailies templates now fall back to the prompted title and index the captured heading correctly.
- Fixed dailies template path handling so manual-style targets like `%<%Y-%m-%d>.org` are rooted automatically in `org-slipbox-dailies-directory`.
