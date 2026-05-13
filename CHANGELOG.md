# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog, and this project follows SemVer.

## [Unreleased]

## [0.12.0] - 2026-05-14

### Added
- Added Rust-owned structural rewrite reports and daemon/CLI flows for
  `edit refile-subtree`, `edit refile-region`, `edit extract-subtree`,
  `edit promote-file`, and `edit demote-file`, including changed/removed file
  reporting and refreshed-index guarantees.
- Added safe remediation apply for supported dangling-link review findings,
  guarded by daemon-owned previews, stale-file checks, restored-target checks,
  explicit confirmation, and refreshed affected-file reporting.
- Added `slipbox link rewrite-slipbox` preview/apply commands to replace
  resolvable `slipbox:` Org links with stable `id:` links through the daemon,
  assigning target IDs where needed.
- Added maintenance diagnostics for files, nodes, and index drift through the
  `diagnose` CLI family.

### Changed
- Defined the `0.12.x` structural editing and stabilization line after the
  `0.11.0` CLI parity cut, including the CLI/Emacs parity audit buckets,
  affected-file/write-preview expectations, maintenance diagnostics, and the
  boundary against premature MCP, agent-adapter, plugin-runtime, scheduler, or
  broad automated mutation work.
- Documented the final `0.12.0` public surface with a CLI/Emacs parity matrix,
  compatibility and deprecation policy for JSON/durable records, and an
  explicit readiness assessment for whether the next release should be `1.0.0`.
- Broadened benchmark gates for high-risk write paths, covering structural
  edits, remediation apply, and `slipbox:` link rewrite preview/apply over
  non-empty server-backed fixtures.

### Fixed
- Hardened structural, remediation, link rewrite, diagnostics, durable-state,
  and rebuild-survival contract coverage across CLI, daemon, store, and
  benchmark surfaces.

## [0.11.0] - 2026-05-13

### Added
- Added first-class everyday CLI families for ordinary slipbox work over the
  canonical daemon boundary: `sync`, `file`, `node`, `ref`, `tag`, `search`,
  `agenda`, `graph`, `note`, `capture`, and `daily`.
- Added CLI write surfaces for file-note creation, explicit file-note ensure,
  heading append, outline append, capture-template execution and preview,
  daily note ensure/append, node identity assignment, and alias/ref/tag
  metadata updates through Rust-owned mutation paths.
- Added daemon-client coverage for everyday read and write operations so the
  CLI reuses typed canonical operations instead of hand-rolled transport calls.

### Changed
- Documented `0.11.x` as everyday CLI parity: Emacs and CLI are now two
  first-class surfaces over the same Org source of truth, derived index, and
  daemon-owned read/write model.
- Broadened benchmark gates for everyday engine paths, covering file sync,
  node lookup/search, occurrence search, agenda ranges, graph DOT generation,
  capture/create, daily append, and metadata update over non-empty fixtures.

### Fixed
- Hardened everyday CLI JSON contracts and read-your-writes integration
  coverage across sync, file, node, ref, tag, search, agenda, graph, note,
  capture, daily, identity, and metadata command families.

## [0.10.0] - 2026-05-12

### Added
- Added explicit workflow spec compatibility metadata with legacy v1 defaulting,
  future-version rejection, and distinct discovery issue reporting for
  unsupported workflow JSON.
- Added core report profile specs for bounded review, routine, audit,
  workflow, and diff output presets with status filters, diff buckets,
  summary/detail mode, and JSONL line-kind selections.
- Added core review routine specs for declarative recurring audit/workflow
  review loops, including typed workflow inputs, save-review policy,
  latest-compatible comparison policy, and report profile references.
- Added core workbench pack manifests for bundling workflows, review routines,
  report profiles, entrypoint routine references, summaries, and validation
  issues as declarative portable assets.
- Added durable workbench pack persistence outside the derived SQLite index,
  with overwrite/no-overwrite save modes, validated loads, and non-polluting
  pack identity.
- Added typed daemon/RPC workbench pack operations for import, show, validate,
  export, list, and delete over the durable pack store.
- Merged imported workbench pack workflows, review routines, and report
  profiles into deterministic server catalogs with explicit shadowing and
  invalid-entry issues.
- Added daemon-owned review routine execution over canonical audit, workflow,
  save-review, diff, and report-profile semantics.
- Added task-shaped `slipbox pack` commands for list, show, validate, import,
  export, and delete over the canonical daemon pack boundary.
- Added built-in review routines plus task-shaped `slipbox routine` list,
  show, and run commands over daemon-owned routine execution.

### Changed
- Defined the `0.10.x` release band as declarative workbench extension through
  workflow compatibility, review routines, report profiles, and packs, while
  keeping plugin runtime, MCP, agent adapters, raw-RPC sprawl, broad mutation,
  notes, review runs, and saved exploration artifacts out of that asset model.
- Broadened benchmark gates for declarative extension paths, covering imported
  pack list, validation, import, routine execution, and report-profile
  rendering over non-empty server-backed fixtures.
- Recalibrated the CI workflow and report-profile benchmark thresholds to
  match the current declarative workbench corpus while preserving release
  profile headroom.
- Documented the declarative workbench extension surface, including pack
  authoring, validation, import/export, routine execution, report profiles,
  compatibility behavior, overwrite policy, and the boundary against plugin
  runtimes, MCP, agent adapters, and raw-RPC wrapper sprawl.

### Fixed
- Hardened pack and routine CLI JSON contracts for wrapper shapes, import/export
  round trips, persisted routine review output, and structured failure behavior.

## [0.9.0] - 2026-05-06

### Added
- Added operational built-in review workflows for periodic review and weak
  integration review, expanding the workflow catalog with routines suited for
  durable review-run capture without adding new workflow step kinds.
- Added read-only remediation preview types and daemon operation for supported
  audit review findings, starting with dangling links and duplicate titles.
- Added `--save-review` flows to audit and workflow run commands so live
  operational results can be persisted as durable review runs from the CLI.

### Changed
- Broadened benchmark gates for operational review paths, covering persisted
  review list/show/diff/mark flows, save-review execution, and remediation
  previews over canonical server-backed fixtures.
- Documented the operational review loop, including `--save-review` examples,
  review list/show/diff/mark/delete commands, and the boundary between review
  records, notes, saved exploration artifacts, and read-only remediation
  previews.

### Fixed
- Hardened review CLI JSON contract coverage for save-review, show, diff, mark,
  and delete flows over the binary/daemon boundary.

## [0.8.0] - 2026-05-05

### Added
- Added named workflow execution over the canonical daemon boundary, including
  built-in workflow list/show/run commands, configured workflow-directory
  discovery, and deterministic catalog issues for invalid or colliding
  discovered workflow specs.
- Added corpus-health audit query and CLI surfaces for dangling links,
  duplicate titles, orphan notes, and weakly integrated notes.
- Added workflow and audit report outputs, including JSONL streams and
  file-output acknowledgements for machine-readable review flows.

### Changed
- Broadened benchmark corpora and regression gates so workflow discovery,
  discovered workflow execution, and corpus-health audit paths are measured as
  part of the larger workbench surface.
- Documented `0.8.x` as a composed research workbench line built from named
  workflows, audits, bounded discovery, report outputs, and stricter scale
  guarantees without claiming plugin-runtime, MCP, or agent-adapter maturity.

### Fixed
- Hardened workflow and audit CLI JSON contracts so wrapper shapes, report
  outputs, discovery issues, cross-command workflow spec round-trips, and
  structured failure behavior are covered at the binary surface.

## [0.7.0] - 2026-05-03

### Added
- Added the first usable headless workbench surface over the canonical daemon
  boundary: a typed Rust daemon client, shared CLI runtime/output scaffolding,
  task-shaped `slipbox` commands for `status`, `resolve-node`, `explore`, and
  `compare`, plus durable artifact lifecycle commands for `list`, `show`,
  `run`, `export`, `import`, and `delete`.

### Changed
- Added live save flows so `slipbox explore --save` and `slipbox compare
  --save` persist durable artifacts through the same engine-owned artifact
  semantics already proven in the cockpit.
- Documented `0.7.x` as the first release band where the workbench is
  genuinely operable outside Emacs, while keeping broader extension, MCP, and
  agent-facing platform claims explicitly deferred.

### Fixed
- Hardened the headless CLI JSON contract suite so daemon-failure, live-save,
  export/import, and saved-versus-executed artifact distinctions are covered
  directly at the shipped binary surface.

## [0.6.1] - 2026-05-03

### Fixed
- Recalibrated the CI backlinks benchmark gate to `32 ms` so the canonical
  GitHub runner threshold matches the current noise envelope of the shipped
  backlinks path instead of failing clean release cuts by less than `1 ms`.

## [0.6.0] - 2026-05-03

### Added
- Added durable exploration artifacts for saved lens views, comparisons, full
  trails, and detached trail slices, with Rust-owned persistence outside the
  derived SQLite index and narrow machine-facing operations to save, inspect,
  list, execute, and delete them.

### Changed
- Reused the settled cockpit exploration semantics when saving, replaying, and
  reloading durable artifacts, so dedicated-buffer load flows restore query
  limits, structure uniqueness, comparison context, and detached trail state
  instead of reconstructing weaker approximations in Emacs Lisp.
- Tightened the durable product docs around the first workbench-foundation
  surface so `0.6.x` claims durable artifacts and narrow artifact operations,
  while broader CLI, extension, and agent-facing platform work remains later.

### Fixed
- Hardened durable-artifact verification so persisted comparison and trail
  artifacts are replayed through the real saved-artifact RPC path even after a
  fresh server reopen.

## [0.5.0] - 2026-05-01

### Added
- Added richer task and time exploration semantics, including explicit
  planning-date relations in dedicated-buffer lenses and first-class
  comparison sections for shared planning dates, contrasting task states, and
  planning tensions.
- Added dedicated benchmark coverage for a guaranteed non-structure
  exploration fixture so cockpit performance checks measure the real
  unresolved-lens path instead of silently falling back to the cheap
  structure view.

### Changed
- Strengthened non-obvious exploration ranking and explanation payloads so
  bridge, dormant, unresolved, and weakly integrated results are ordered by
  explicit supporting evidence and preserve fuller rationale in the cockpit.
- Reworked dedicated-buffer rendering around explicit lens-local and
  comparison-group plans, with clearer explanation blocks, coherent trail
  labels, and more legible attached-versus-detached trail state.
- Tightened durable product docs around the settled `0.5.x` cockpit model so
  the dedicated buffer is documented as the exploratory house while saved
  views, broader headless workflows, and workbench extraction remain later
  work.

### Fixed
- Preferred newer Elisp sources during batch test runs so verification does
  not accidentally pick stale compiled artifacts over the current source tree.
- Fixed the dedicated exploration benchmark contract so it now fails loudly if
  the generated corpus does not provide the intended unresolved-lens fixture.

## [0.4.0] - 2026-04-30

### Added
- Added a dedicated-buffer exploratory cockpit with declared lenses,
  structured explanation payloads, pivotable navigation, note comparison, and
  explicit trails built on shared Rust query semantics rather than ad hoc
  buffer-only state.
- Added non-obvious exploration surfaces for bridge candidates,
  dormant-but-relevant notes, unresolved task-linked material, and weakly
  integrated notes, all with explicit reasons for why a result surfaced.
- Added a push-triggered GitHub Actions verification workflow covering
  formatting, Rust tests, clippy, Emacs batch tests, and the benchmark
  regression gate.

### Changed
- Reworked the dedicated buffer into a stateful exploration surface with
  explicit session, history, frozen context, comparison, and trail state while
  preserving the persistent buffer as the cheap point-tracking path.
- Clarified the durable product docs around the cockpit-versus-workbench split
  so `0.4.x` remains focused on exploratory cockpit maturity rather than
  prematurely freezing a broader headless platform surface.
- Expanded benchmark and regression coverage for the cockpit model, including
  dedicated verification for persistent and dedicated buffer rendering paths.

### Fixed
- Streamlined backlink lookup so the common path honors limits earlier and
  caches per-file owner resolution instead of recomputing note ownership for
  every backlink row.
- Optimized `node_at_point` with lean ownership resolution and a supporting
  composite node index, preserving current semantics while restoring the query
  to comfortably sub-threshold benchmark performance.

## [0.3.0] - 2026-04-17

### Changed
- Narrowed the public note model to canonical notes only, so
  `org-slipbox-node-find`, `org-slipbox-node-insert`, exact title-or-alias
  lookup, random-node selection, backlinks, forward links, graph export, and
  other user-facing note surfaces no longer expose anonymous heading anchors.
- Split canonical note records from structural anchor records across the Rust
  store, RPC layer, and Emacs client so anonymous headings remain available for
  agenda, occurrence ownership, subtree rewrite, and other anchor-oriented
  operations without leaking into public note semantics.

### Fixed
- Required the standard `crm` library in metadata commands so tag completion
  byte-compiles cleanly under lexical binding during release verification.

## [0.2.0] - 2026-03-14

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
- Exported public Emacs helpers for node/ref completion candidates,
  completion annotations, node visiting, and direct link insertion so
  downstream frontend packages no longer need double-hyphen internals for
  integration.
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
