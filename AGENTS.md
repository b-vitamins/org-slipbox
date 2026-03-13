# AGENTS

## Purpose
- Build `org-slipbox` as a local-first Org knowledge engine with an Emacs front-end.
- Keep the architecture honest: Org files are the source of truth, the index is derived, and interactive paths must stay sublinear in corpus size.
- Use `/tmp/org-roam` as a reference implementation when evaluating replacement status, but never mention that path inside this repository.

## Guardrails
- Keep hot paths out of Emacs Lisp. Parsing, indexing, ranking, and query execution belong in Rust.
- Keep structural and metadata writes out of Emacs Lisp. The client may coordinate buffers, but file mutation belongs behind Rust RPCs.
- Do not let hidden global state decide correctness. The daemon must expose freshness explicitly and writes must have read-your-writes semantics.
- Use dedicated incremental index updates for changed files. Do not route file-level sync through full-root prune logic.
- Do not introduce circular crate or feature dependencies. Domain types stay in `slipbox-core`; transport types stay in `slipbox-rpc`.
- Avoid load-time side effects in Elisp. User-facing commands may start the daemon, but simply loading the package must not mutate user state.
- Keep expensive discovery off the persistent redisplay path. Grep-backed sections belong in dedicated buffers or explicit refresh flows.
- Treat capture, search, backlinks, refs, and agenda as first-class product surfaces. Do not bolt them on through incidental internals.
- Keep comparisons factual, specific, and user-serving. Compatibility notes and migration guidance are fine when they reduce friction, but avoid dismissive or salesy competitive copy.
- Keep documentation tight. Use `README.md`, `CHANGELOG.md`, `doc/*.org`, and code comments; do not create ad hoc planning markdown files.

## Release Policy
- Work under `Unreleased` in `CHANGELOG.md` for ongoing development, and keep released sections authoritative once shipped.
- Treat current tagged releases and version metadata as legitimate. Do not retroactively downplay shipped versions.
- Keep the default branch focused on the next planned minor release.
- Use issue-tracker milestones as the literal release buckets for upcoming versions. Keep broad direction in `doc/*.org`, and keep per-release cut lists out of the repository.
- Allow patch releases for important fixes to the latest shipped line when users should not have to wait for the next minor release.
- Keep patch releases minimal and scoped to shipped behavior. Do not bundle unrelated feature work into a patch release.
- When a patch release is needed during ongoing next-minor work, cut a short-lived maintenance branch from the latest tag, cherry-pick the minimal fix, and keep the equivalent fix on the default branch unless the code has materially diverged.
- Unless explicitly stated otherwise, support only the latest shipped release line.
- Do not claim `1.0.0`, stable-platform finality, or full replacement status until the project genuinely reaches that bar.
- Use Conventional Commits for every commit message.
- Keep history readable: commit at coherent milestones after tests pass.

## Repository Conventions
- Prefer a single repository containing the Rust workspace and the Emacs package.
- Keep Emacs package entry files at the repository root for straightforward ELPA packaging.
- Keep package headers strict: lexical binding, `Package-Requires`, `Version`, commentary, and no false metadata.
- Maintain GPL-3.0-or-later licensing across Rust and Elisp code.
- Keep automation credentials out of version control. For GitHub API work in this repository, use the ignored `/.env.github.local` path when a repo-local token is needed, prefer least-privilege fine-grained tokens, and keep expirations short.
- Favor stable protocol boundaries over in-process integration tricks. JSON-RPC over stdio is the default boundary.
- Keep the daemon and Emacs package separable. The Emacs side should work with a `slipbox` executable on `PATH` or an explicit `org-slipbox-server-program`, so downstream packaging stays simple.
- Keep `manifest.scm` as an optional contributor convenience layer. It may smooth local development and comparison runs, but it must not become the only supported build or install path.
- Keep durable product strategy documents in `doc/` as Org files. Vision, milestones, and roadmap belong there; transient planning belongs outside the repository.

## Verification
- Run `cargo fmt`, `cargo test`, and `cargo clippy --all-targets --all-features` before milestone commits when the codebase supports them.
- Run Emacs batch checks before milestone commits once Elisp commands exist.
- Add regression tests with every bug fix touching parsing, indexing, query semantics, or protocol behavior.
- When a local machine lacks the C or packaging toolchain needed for a clean build, prefer `guix shell -m manifest.scm -- <command>` over repo-specific environment hacks.
