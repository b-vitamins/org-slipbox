# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog, and this project follows SemVer.

## [Unreleased]

### Added
- Ship a tab icon with the reading client. The shell declared none, so every load
  paid a not-found for the browser's automatic `/favicon.ico` probe, and a reader
  holding a session of notes open had nothing but the browser's default page mark
  to tell one tab from another. The mark is one SVG that inverts for a dark tab
  strip. `/favicon.ico` stays a not-found: a declared icon is what stops the
  probe, and answering a missing image with the app shell would render as a broken
  icon rather than a missing one.
- Read the exploration lenses over the reading API. `/api/explore` takes a note
  key, a lens, and an optional limit, and answers with the sections that lens
  defines: structural links, shared references and unlinked mentions, planning and
  task neighbors, bridge candidates, dormant material, unresolved tasks and weakly
  integrated notes. The lens is a parameter rather than a route per lens, an
  unknown one is refused with the accepted set rather than read as a default, and a
  section the lens defines is served empty rather than omitted. The reading client
  gains the matching types and an `explore` call; no surface asks for a lens yet.
- Carry a note's place in the filing order on `/api/note/context`: its ordinal in
  `(file_path, line)` order, the number of notes indexed, and the key and title of
  the note filed on each side. A note at either end of the order has no neighbor
  there, and that side is left out of the payload rather than sent as null. The
  reading client gains the matching types; no surface states a position yet.
- Page both glossary listings past their first 200 terms. `/api/glossary/terms`
  and `/api/glossary/due` answer with the size of the listing behind the page,
  whether more follows, and an opaque `next_position` to pass back as `after`.
  Each listing pages on its own key, so a token the other minted is refused
  rather than resumed at the wrong place. `/api/glossary/search` ranks by
  relevance, which is no stable key, so it serves one page and states the cut.
- Offer ranked related notes in the reading footer: notes the bridges lens ranks
  two hops out that this note does not link to, gathered under the connector they
  are reached through and dropped where the directed inventory already lists them.
  Nothing is asked of the lens until a reader opens the group, so a column still
  costs one request for as long as its footer is only read. The head's cut and the
  lens's own limit are stated separately, since a row count tells them apart from
  neither.
- Rest the reading spine at a snap position, so a free scroll settles with a
  column against the offset it pins at rather than part-cut at an edge. Snapping
  is by proximity, leaving a reader who deliberately holds two half columns
  alone, and the narrow run snaps per note on the axis it scrolls.
- Offer unlinked mentions in the reading footer: notes that write this note's
  title in their prose without linking to it. The links inventory and the bridges
  lens both read link topology, so a note no link touches is answered by neither,
  and the footer now stands for such a note on the mention group alone. A note
  collapses to its first occurrence, the scan's bound is counted in occurrences
  where the head's cut is counted in notes, and the matched run is marked inside
  the line it stands in. `slipbox explore` heads a mention with the note it sits
  in, naming the anchor beside it where that is a heading of its own.
- State what makes a glossary term due. The due listing drew bare headwords in an
  order with no visible principle, since a corpus awaiting its first review carries
  no due dates to sort on. The listing now states its own total and names the order
  it holds the page in, and each row carries its standing: never reviewed, or the
  date it came due. An empty review drawer is a fact rather than a blank pane, so
  the peek says a term has never been reviewed instead of rendering nothing, while
  a drawer holding part of a schedule still shows only the part it holds.
- Browse the glossary past its first page. A reading resource holds one value per
  key, so a second page read through it replaces the first rather than extending
  it; the surface accumulates instead, keeping the first appearance of each node
  key and dropping the lot when the listing changes. Each position is echoed back
  as the listing handed it out, never composed. The term list is its own
  scrollport, so reaching its end asks for the next page, and a control below it
  does the same for a reader who has not scrolled. A search states its cut and its
  total with no offer to continue: a ranking has no stored position to resume from.
- Name the open glossary term in the URL. `?term=` joins `?q=` and `?view=`, read
  on mount to seed the selection and replaced on every later one, so a definition
  on screen has an address a reload or a copy reopens. A key none of the pages read
  holds is resolved through `/api/glossary/term` and peeked off the list, and a key
  the index refuses is named rather than answered with the first row.
- Read a glance preview from the keyboard. A card raised by a cursor and one raised
  by focus were tagged alike, and every card but the tap-raised one was
  `aria-hidden`, so a reader who reached a link by Tab was shown a preview
  announced to nobody. A focus-raised card is now the description of the link that
  raised it, read out after the link and cleared with the card. A cursor-raised one
  stays decorative, since it says what the cursor is already over, and no card
  takes focus: the reader stays on the link.

### Changed
- Draw the `dormant` lens's candidates from link topology rather than from shared
  `:ROAM_REFS:` entries, and order them by age rather than by citations in common.
  The lens answers what older material a note should be read against, which a
  shared citation neither establishes nor is needed for: a focus note carrying no
  refs got nothing, and among the notes that did qualify the oldest could be
  reported below a newer one that happened to share more references. Age now
  leads, then the number of notes a candidate is reached through, then references.
  A `dormant-shared-reference` explanation gains a `via_notes` list and its
  `references` list is empty for a candidate found through links alone;
  explanations stored before this change load with no via notes.
- Draw the weakly integrated notes the `unresolved` lens reports from link
  topology rather than from shared `:ROAM_REFS:` entries, so a link-poor note
  surfaces because something the focus note links to, or is linked from, also
  reaches it. The section names the notes a slipbox has nearly forgotten to
  connect, and requiring a shared citation refused exactly the notes least likely
  to carry one. Either kind of evidence now admits a candidate, ordered by
  structural link count, then by how many notes reach it, then by references in
  common, so the least connected note is still reported first. A
  `weakly-integrated-shared-reference` explanation gains a `via_notes` list and
  its `references` list is empty for a candidate found through links alone;
  explanations stored before this change load with no via notes. A clause naming
  evidence a candidate does not have is now left out of the rendered reason, here
  and in the bridge lens.
- Draw the bridge lens's candidates from link topology rather than from shared
  `:ROAM_REFS:` entries, so a note two hops out surfaces because something the
  focus note links to, or is linked from, also reaches it. A shared reference was
  the lens's entry condition, which made the whole lens answer to a citation
  habit instead of to how the notes are linked: a note carrying no ref of its own
  was refused before any candidate was considered, and a slipbox whose notes
  carry no refs got nothing from the lens however densely it was linked.
  References now rank a candidate rather than admit one, below the number of
  notes a candidate bridges through and above the file-path order that separates
  equals, so a citation in common still lifts the candidate that holds it.
  Candidates a shared reference already reached keep their place, and the
  `references` list on a `bridge-candidate` explanation is empty for a candidate
  found through links alone. A note whose neighbors lead nowhere else still
  returns nothing, because a bridge needs a second hop to exist.
- Open a glossary term in the reader from a control beside its headword rather
  than one below its definition, where a long body carried it off the bottom of
  the pane and a reader had to scroll past the whole definition to find the way
  onward. The control is an anchor to the term's own reading URL, so its
  destination can be copied, opened in a new tab, or middle-clicked the way any
  other link can, and it routes through the same navigation grammar as a link out
  of the definition, leaving one place to decide what opening means on this
  surface. It names the term by its id when the note carries one and by its
  slipbox key when it does not, the rule the relations footer already followed
  and now shares a single constructor with, and it clears the coarse pointer's
  touch-target floor, its label centered in the taller box that floor stretches
  it into.
- List a related note once in the reading footer, marked with the direction its
  links run in, rather than once per direction group. A reciprocal note stood in
  both groups, and a forward link's preview quoted the line of the note being
  read, already on screen above the footer. One listing now holds a row per
  related note, in first-appearance order with forward links first, and a preview
  comes from the backlink alone. Each direction states its own cut against its
  own total.
- Set the reading footer at an index's scale rather than the prose's. Three
  groups at the reading scale cost a screenful of a narrow viewport, a page of
  chrome under every note: a footer for a note with six related notes measured
  345px and now measures 243px. Row pitch, group spacing, and the space above the
  rule come down, and the three group labels share one register instead of a
  letterspaced uppercase run. The rule stays, and under a coarse pointer a row's
  height is still the touch-target token's floor.

### Removed
- Retire the reading surface's hop-bounded neighborhood: the `/api/neighborhood`
  route, its walk, and the ranked distance rings the reading column offered
  behind a disclosure. The rings answered which notes lie near this one, which is
  the question the relations footer exists to answer, and answered it worse: a
  second request per note, a second ranking, a filter box over a list a reader
  had not asked for, and a second grammar for opening a note. A note's links and
  backlinks remain in the footer, where they come out of the reading context the
  column already fetched. What goes is the web surface's own walk: the indexed
  link graph, the store queries over it, and the `slipbox graph` DOT export are
  untouched, as is every other reading route.

### Fixed
- Refuse a reading-surface request whose `Host` is not a loopback authority with
  a 403, before any route runs. The surface binds loopback only, but a browser
  sends the name it was given, so a page under a name that resolves to
  `127.0.0.1` reaches the port as a same-origin caller and binding alone does not
  keep it out. `localhost` and any loopback literal are served with or without a
  port; a name that merely resolves to loopback is refused, as is a head carrying
  two `Host` fields, and an HTTP/1.0 request that names no host is unaffected.
- Follow only an external Org link target whose scheme navigates (`http`,
  `https`, `mailto`), and render every other target as inert text still carrying
  its label. A target reached the DOM as an `href` unfiltered, and a note body is
  not necessarily the reader's own, so a `javascript:` link in an imported,
  clipped, or shared note ran in the reading origin, which holds same-origin read
  access to every note and glossary term the API serves; read-only bars the
  write, not the read, and the corpus is what such a link would be after. The
  scheme is read the way a browser reads one, after the tab, newline, and leading
  control characters a browser strips before parsing a URL, and the href handed
  to the DOM is that normalized target rather than the raw one. A `file:` or
  relative target, which this surface could not follow over HTTP anyway, now
  reads as text rather than as a dead link.
- Follow a link inside a glossary definition. The peek pane draws a definition
  with the same Org renderer the reading column uses, so its `id:` links came out
  as real anchors, but the navigation grammar those anchors route through was
  provided only inside a reading column. Unprovided, it fell back to the inert
  one, where the anchor's own `preventDefault` swallowed the click and no verb
  ran, leaving a link that read as live and behaved worse than inert text: a
  modified click still opened it in a new tab while a plain one did nothing at
  all. The glossary is an entry surface with no spine standing beside the
  definition, so both committing verbs open the target as the reading root, and a
  tap from a hoverless pointer commits rather than glancing at a preview card
  this surface does not mount.
- Reach every backlink a note has in the reading footer. The fetch took the
  context route's default of 25 relations per direction and dropped the rest of a
  well-linked note's links without saying so; it now asks for 200, the route's own
  ceiling. `/api/note/context` carries a total per direction alongside the arrays,
  counting related notes the way the listings do rather than the link rows the
  stored `backlink_count` sums, and a group states "Showing 14 of 16." only when
  the rows it drew fall short of that total - so a note linked to twice from one
  place no longer reads as cut.
- Open a stacked note once. An address naming one reference twice drew a column
  per naming, so a note reached again from further down the stack stood twice with
  both copies scrolling as one. A repeated reference now collapses to the first
  position holding it, and the address is rewritten in place to the stack that is
  drawn, adding no history entry to press Back through. Repeats compare
  spellings, so two different references naming one note stand apart until a
  fetch resolves both.
- Size the app shell, the reading spine, the glossary pane and its term list, and
  the entry surface's own top padding against the visible viewport rather than the
  largest one, so a retracting browser toolbar no longer leaves the bottom of a
  column behind chrome that is no longer there, or pushes the search field down out
  of reach. Each site keeps its `vh` declaration ahead of the `dvh` one, so an
  engine without the dynamic unit holds the old value.
- Contain a swipe carried past the end of the reading spine, a note's body, or a
  glossary scrollport, so it no longer reaches the browser as a history gesture
  and leaves the surface mid-read. A column contains the vertical axis only: the
  spine beneath it is the horizontal scroller. The viewport is untouched, so
  pull-to-refresh stays the browser's.
- Hold the reading measure where the columns stack. A stacked column spans the
  whole frame, so at 720px a line of prose ran to 91 characters against 71 in a
  column. A note now caps at the measure a column gives it and centers in what is
  left over, while the column, its border, and the seam under it keep the full
  width. Below the cap the note still fills the frame less its padding.
- Bound a relation preview to the line it shows. The row clipped its preview to
  one line in paint alone, so the text node still carried the whole paragraph the
  index holds and a screen reader read out 300 characters where a sighted reader
  saw one. The projection now elides at 120 characters, through the helper the
  glance excerpt already uses, and the clip stays as the guard for a width where
  the bounded preview still does not fit.
- State a cut glossary listing on the command line. `glossary list`, `search`,
  and `due` printed the page's own length as the count, so a listing cut at
  `--limit` read as the whole glossary. Each now counts the page against the
  whole listing when the page falls short of it.
- Keep a relation preview readable where its row wraps. The preview shared its
  title's line down to a 22ch reservation, so a long title at a narrow width left
  a fragment too short to tell one linking line from another while still costing
  the row a full line. The measure is stated once at 30ch, about 45 characters, as
  both the flex basis and the preview's floor, so a title that leaves less than
  that sends the preview to a line of its own at full column width. The floor caps
  at the column for a column narrower than the measure.
- Head the reading surface's glossary with a top-level heading. The highest
  heading it drew was the peek's `h2` headword, level with the definition's own
  Org headings, so the outline had no top and no nesting. An `h1` names the
  surface outside every conditional block, and a definition's headings now begin
  one level under the headword: `RenderDocument` takes the heading base as a prop,
  and the reading column keeps today's `h2`.
- Let the glossary's listing controls name what they switch. "All terms" and "Due
  for review" carried the tab role with no tabpanel beside them, announcing a
  relationship the surface never held, and their roving tabindex took the arrow
  keys the term list drives. They are now toggle buttons in a group, each pressed
  when its filter holds and each naming the listbox it filters, so both are
  ordinary tab stops and every arrow key stays with the list. The segmented look
  reads off the pressed state itself.
- Keep the glossary's search field in the due listing. It rendered in browse mode
  only, so a page of due terms could be reached by eye alone. The field stands in
  both modes now, named for the set it is over, and in study mode narrows the due
  rows the surface already holds by headword and synonym: the index answers no
  search behind a due filter, so asking it would list terms that are not due. A
  filter that hides every due term says so instead of explaining how terms come
  due, and the field is the one tab stop into the list in both modes, which drops
  the listbox's own.
- Show where focus rests in either search field. The entry surface's field and the
  glossary's each suppressed the platform ring and reported focus with a 1px border
  tint, the weakest indicator on the surface. Each draws a 2px ring in the link
  color as well, offset clear of the box so nothing moves, at `:focus-visible` so
  the platform decides when focus is worth painting.
- Announce what a search found rather than the masthead above it. `aria-live` sat
  on the whole entry surface, so every keystroke re-announced the masthead and the
  corpus line and never the one fact a searcher is waiting for. A status line holds
  the count alone, placed before there is anything to say, with the result list,
  the hero, and the random control outside it. The capped-list line drops the count
  it restated and keeps the advice, and the surface-level `aria-busy` goes with the
  region it described: a search in flight is reported in words instead, which a
  busy flag would have suppressed.
- Keep a search match inside the excerpt that claims it. The excerpt projection had
  no length bound, so a match far into a snippet drew past the two-line clamp and
  the row asserted a match it never showed. A pass now trims the leading context
  until the first matched run draws inside what the clamp holds, marking the cut
  with the same ellipsis the construct repairs use. It runs after those repairs, so
  it measures prose that will be drawn rather than debris a repair is about to drop,
  and keeps its own cut clear of every construct; an excerpt already inside the
  clamp is untouched.
- Move focus to a note opened from the search results. Committing a result replaced
  the entry surface with the spine and left focus on the body, so the next Tab
  restarted at the header and nothing named the note that had opened. A column's
  article takes a negative tab index, and the spine focuses it whenever it reveals a
  column, deferred until that column leaves the obscured state, which is hidden and
  so unfocusable. The focus is the spine's own work rather than a tab stop, so the
  article paints no ring and its scroll stands.
- Keep the query when the header returns to the entry surface. The way home pushed
  the bare path with a null state, so a reader who searched, opened a result and
  took it landed on an empty field with no cursor, while browser Back restored both.
  The shell holds the address the entry surface was left at and pushes that one, and
  a stack push carries forward the history state it used to discard, which is where
  the result cursor rides. That address holds no reading position, so the way home
  still clears the spine.
- Take a hover selection from the pointer's own move. Both option lists selected a
  row on `mouseenter`, which a row arriving under a resting cursor sends too: a
  keystroke replaces the search results, and a glossary page appended below shifts
  the rows above it. The reader lost the cursor Enter opens, and in the glossary the
  address was rewritten to a term nobody reached. A crossing is read as a hover only
  where the pointer is not already, and a click still selects whatever it presses.
- Title the tab after the column being read. The tab named the stack's last
  reference and nothing revised it, so a scroll back along a trail left it naming a
  column nobody was reading. It now names the first column the ladder has not
  pinned, or the frontmost where the whole spine stands in the frame. Each column
  reports the title its own read already carried, so a scroll costs no request and
  the identity resolve the tab used to make is gone.
- Place a glance card clear of the prose it explains. The card sat just below its
  link, covering the lines the reader was asking about, while the space beside the
  reading columns stood empty. It now takes that space, the trailing side first,
  level with the link so it reads as a note in the margin, and falls back to the
  old placement where neither side holds it. The space is measured against every
  column, not the link's own: room the next column stands in is not free.

## [0.17.0] - 2026-07-25

### Added
- Added ranked note-content search as a third search concept alongside node
  search and occurrence search: `search content` matches a note's body, title,
  and aliases through a dedicated `node_content_fts` index, ranks whole notes
  by relevance, and returns a highlighted excerpt per hit.
- Added the `slipbox/searchNodeContent` RPC method and a `search content` CLI
  command that renders each ranked hit with its highlighted body excerpt,
  exposed across core, store, RPC, server, daemon-client, and the CLI.
- Added a highlighted-snippet contract carried as structured `ContentSnippet`
  segments, each flagged matched or unmatched, so a client renders emphasis
  without re-scanning the body or trusting an in-band delimiter.
- Added an indexed-note count to `status`, reporting the addressable note
  surface (file notes and explicit-ID headings) as a strict subset of the
  indexed node count.
- Added note-content contract and benchmark coverage, including a gate proving
  the content probe stays invisible to metadata search so the two remain
  parallel paths.
- Added a read-only web reading surface as a third front-end over Notes and
  Glossary beside Emacs and the CLI, owning no new public bucket: a `slipbox
  web` command bridges HTTP to a spawned `slipbox serve --read-only`, binds
  localhost only, self-spawns the daemon, and serves a small SolidJS reading
  client beneath a bounded JSON API.
- Added an operation mutation classification hosted in `slipbox-rpc` beside the
  method constants, and a read-only serve guard that rejects any non-read-only
  method at dispatch, so read-only is a machine-checkable allowlist rather than
  a convention and no HTTP path can reach a mutating operation.
- Added the `slipbox-web` crate: a read-only daemon bridge and a bounded HTTP
  API over Notes and Glossary covering node lookup, a note reading context,
  node, content, and glossary search, random node, relations, glossary terms,
  definitions, due selection, and a hop-bounded neighborhood.
- Added a SolidJS reading client with URL-as-state navigation, an Org reading
  column with a near/far spine, glance/pin/go navigation reachable by cursor,
  keyboard, and touch, a search-first entry surface over ranked content search,
  and a glossary dictionary with definition peek and an inert study trail.
- Added an `embed-web-client` build feature that compiles the built client into
  the binary for a self-contained reader, CI that typechecks, tests, and builds
  the client, and user documentation for the reading surface.
- Added a reading-surface color scheme that follows the operating system's light
  or dark preference, with a header control cycling Auto, Light, and Dark for a
  reader who wants the opposite. The palette is one set of `light-dark()` token
  pairs, so both schemes come from a single source, and the choice is stored per
  browser rather than in the URL so a shared link carries a reading position and
  not a theme.
- Added an end-to-end reading-client suite driving a real browser against the
  built client over a stubbed JSON API, with a `test-e2e` target and a CI job,
  covering the behaviors the jsdom unit suite cannot see: the spine holding the
  viewport height while a tall note scrolls internally, a restored multi-note
  trail revealing its frontmost column, real browser-history back and forward,
  native link destinations, and the narrow-layout responsive breakpoint.

### Changed
- Changed note-content search to be a parallel path to node search rather than
  a change to it: `node search`, its `node_fts` metadata index, and the Emacs
  `org-slipbox-node-find` recall path keep their exact matching behavior and
  result shape; content search adds a separate index, contract, and ranked
  snippet-bearing result beside them.
- Bumped the derived schema version so the note-content index builds on open;
  the index is derived from Org bodies and rebuildable, and its relevance
  ordering and tokenizer stay internal.
- Defined the `0.17.x` reading-surface band as the deliberate, bounded
  introduction of a read-only web front-end over Notes and Glossary, bridging
  HTTP to a spawned read-only `slipbox serve` over the same daemon-client
  boundary Emacs and the CLI use, while keeping the surface localhost-only and
  single-user and keeping a second database, separate sync mechanism, SSE or
  websocket push, spaced-repetition grade writeback, global force-directed
  graph, and MCP surface out of that front-end.

### Fixed
- Resolve an absolute file path against the canonical root by matching the
  path's own ancestors, so a root reached through a symlinked parent no
  longer rejects its own children as outside the slipbox. Components below
  the root stay as spelled, so a path leaving the root through a symlink
  inside it is still refused.
- Use tab recipe prefixes in the source-build `Makefile` instead of the
  `.RECIPEPREFIX` directive, so every target, including the release metadata
  gate, runs under the GNU Make 3.81 that ships as `make` on macOS.
- Report source truncation on node source reads and note contexts against the
  window the read asked for, the node's own line range plus any requested
  context, instead of against the whole file, so a heading that holds its
  entire subtree reads as complete rather than truncated on both sides.

## [0.16.0] - 2026-07-22

### Added
- Added a Glossary bucket as the seventh public bucket, built entirely on the
  Org-as-truth and derived-index model: a term is an Org note carrying a
  `#+glossary: t` marker, with the headword as `#+title`, synonyms as
  `ROAM_ALIASES`, the definition as the body, and review state in the property
  drawer, so it participates fully in node search, backlinks, refs, and graph.
- Added `glossary` CLI and RPC surfaces for listing terms, stemmed and
  diacritic-folded term search, term inspection with definition and schedule,
  due-term selection, SM-2 grading, and marking an existing note as a term.
- Added SM-2 spaced-repetition scheduling with the schedule stored in each
  term's `GLOSSARY_STATUS` and `SR_*` drawer keys, so it syncs over git and is
  re-parsed from Org on index rebuild rather than living in a side store.
- Added Emacs glossary capture, definition lookup and peek, a dedicated-cockpit
  `glossary` lens, and a card-at-a-time spaced-repetition review buffer with
  single-key grading for the full in-editor study loop.
- Added glossary contract, rebuild-survival, and benchmark coverage, including
  proof that SM-2 drawer state survives a forced derived-index rebuild and
  benchmark gates over glossary list, search, due, and grade paths.

### Changed
- Changed the `node_fts` search tokenizer to `porter unicode61
  remove_diacritics`, so node search matches across word stems and folds
  diacritics; the derived schema version bumps and the index rebuilds on open,
  and search ranking is treated as internal rather than a public contract.

## [0.15.0] - 2026-07-20

### Added
- Added upgrade and derived-index rebuild guidance, including 0.14.4 to
  0.15.0 upgrade steps and durable side-store recovery boundaries.
- Added a 0.15.0 public contract audit baseline and an RPC operation
  descriptor inventory test for method family, mutation, and freshness drift.

### Changed
- Clarified roadmap and release-bucket discipline around GitHub milestones,
  release metadata, and tracker-owned cut lists.
- Report every benchmark threshold miss in benchmark checks and retune the CI
  and release profile thresholds for observed host variance in workflow,
  report, agenda, graph, and structural-write measurements.
- Prefer a manifest-provided `gcc` when the source-build Makefile probes for a
  C compiler, apply that compiler environment to the system-SQLite build path,
  and document bundled-SQLite compiler prerequisites.

## [0.14.4] - 2026-06-02

### Fixed
- Tolerate partial indexed node metadata in node completion display, local
  sorters, visits, and capture clocking, avoiding nil crashes and `nil:nil`
  display fragments.
- Release capture caller session markers when finalization fails during
  prepare or materialization, preventing stale caller state after failed
  captures.
- Clamp all capture blank-line options to non-negative values, including
  explicit `:empty-lines-before` and `:empty-lines-after` settings.

## [0.14.3] - 2026-06-02

### Fixed
- Keep generated file-level property drawers directly adjacent to following
  file keywords instead of inserting a blank line between `:END:` and
  `#+title`/`#+TITLE`.
- Fold Org drawers after visiting indexed nodes, so newly created or opened
  note files do not expose their file-level property drawer by default.

## [0.14.2] - 2026-06-01

### Fixed
- Insert generated file-level property drawers before file keywords when
  creating or promoting file notes, so `ID` and `ROAM_REFS` stay at the top of
  new Org note files.

## [0.14.1] - 2026-06-01

### Fixed
- Made the CLI agenda fixture use non-colliding dynamic range dates so the
  today query test stays valid on 2026-06-01 and other calendar dates.
- Pruned missing indexed note files from node/ref query results so deleted
  notes stop appearing in completion, ref capture falls through to note
  creation, and stale indexed visits cannot recreate empty files.

## [0.14.0] - 2026-05-24

### Added
- Added bounded source-read contracts and structured read/error payloads across
  the core, RPC, daemon-client, server, and CLI paths, including tests for
  missing, unindexed, invalid, and bounded-read cases.
- Exposed reusable Rust service operations so daemon handlers and future
  headless callers can share the same command semantics without duplicating
  CLI glue.

### Changed
- Split the daemon-client crate and Emacs context-buffer implementation into
  focused internal modules while preserving the public client API, package load
  path, and buffer behavior.
- Split the CLI internals for notes, assets, and shared runtime helpers into
  focused modules, and centralized manual daemon/export paths around cleaner
  command boundaries.
- Split the remaining large CLI modules for explorations, relations, and
  reviews into product-surface modules, and moved live artifact/remediation
  construction semantics into core helpers for reuse by other Rust entry
  points.
- Split server workflow/pack/routine query handlers and write handlers into
  focused modules, keeping RPC behavior stable while making durable command
  boundaries easier to audit.
- Extracted the shared durable JSON file-store used by saved artifacts,
  workbench packs, and review runs so atomic persistence semantics are defined
  in one place.
- Batched relation lookups, index refreshes, graph rendering support, and
  relation-count reads so large-corpus query and write paths stay comfortably
  inside the tightened performance envelopes.
- Materialized backlink and forward-link counts in the derived index, with
  incremental maintenance during file sync and regression coverage for count
  refreshes.
- Applied SQLite write pragmas on every database open so existing derived
  indexes retain the intended WAL and synchronous behavior across fresh
  daemon/service connections.
- Tightened CI and release benchmark thresholds around the current query,
  indexing, write, workflow, review, and graph performance envelopes.
- Updated benchmark targets and documentation so benchmark gates run optimized
  binaries by default.

### Fixed
- Confined root-relative path handling across CLI, server, write, reflink, and
  unlinked-reference paths so repository-root escapes are rejected consistently.
- Staged structural rewrites, capture writes, metadata updates, and link
  rewrites before destructive file replacement, preserving refreshed-index
  guarantees after successful writes.
- Clarified README wording around load-time link registration and startup
  behavior.

## [0.13.2] - 2026-05-14

### Added
- Added `org-slipbox-buffer-load-artifact-by-id` as a public helper for
  restoring a saved exploration artifact into the dedicated cockpit by durable
  artifact id, preserving the existing cockpit restore semantics.

## [0.13.1] - 2026-05-14

### Fixed
- Corrected Elisp package headers so shipped package files report the current
  release version and use `Ayan Das` as the copyright holder.
- Added a release metadata gate covering Cargo, README, and Elisp package
  headers so version and copyright drift is caught in CI.

## [0.13.0] - 2026-05-14

### Added
- Added a compact public model in `doc/model.org`, including the command
  taxonomy used to keep notes, relations, explorations, reviews, assets, and
  system surfaces distinct.
- Added a consolidated compatibility and deprecation policy in
  `doc/compatibility.org`.

### Changed
- Rewrote the README and roadmap documents around the normal user path,
  durable product model, and current command surface instead of release
  archaeology.
- Split core domain types, CLI command families, CLI render/output helpers,
  server query handlers, and the benchmark harness into clearer modules while
  preserving public CLI, RPC, and JSON behavior.
- Made CLI help taxonomy-aligned and detailed enough to serve as the command
  reference for the consolidated surface.
- Consolidated integration-test and benchmark helper code without changing the
  tested behavior.

### Removed
- Removed duplicate CLI spellings `ref show` and `capture node`; use canonical
  `ref resolve` and `note create` instead.
- Removed confirmed dead helper code from Rust and Emacs Lisp sources.

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
