/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.engine.DocumentLinkResolution
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.navigation.ReadingAnchor
import io.github.b_vitamins.slipbox.navigation.ReadingReturnAvailability
import io.github.b_vitamins.slipbox.navigation.ReadingReturnsState
import io.github.b_vitamins.slipbox.navigation.SlipboxDestinations
import io.github.b_vitamins.slipbox.navigation.SlipboxNavigation
import io.github.b_vitamins.slipbox.navigation.SlipboxRoute
import io.github.b_vitamins.slipbox.navigation.SlipboxSurface
import io.github.b_vitamins.slipbox.navigation.rememberReadingTrailSession
import io.github.b_vitamins.slipbox.navigation.rememberReadingReturnsState
import io.github.b_vitamins.slipbox.navigation.readingRoute
import io.github.b_vitamins.slipbox.navigation.slipboxDestinations
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sources.SourceLibraryState
import io.github.b_vitamins.slipbox.sources.rememberSourceLibraryState
import io.github.b_vitamins.slipbox.ui.AboutScreen
import io.github.b_vitamins.slipbox.ui.ConnectionScreen
import io.github.b_vitamins.slipbox.ui.CorpusSearchPhase
import io.github.b_vitamins.slipbox.ui.CorpusSearchState
import io.github.b_vitamins.slipbox.ui.DirectedRelationsPhase
import io.github.b_vitamins.slipbox.ui.DocumentReaderPhase
import io.github.b_vitamins.slipbox.ui.GlossaryInventoryPhase
import io.github.b_vitamins.slipbox.ui.GlossaryInventoryState
import io.github.b_vitamins.slipbox.ui.GlossaryReviewPhase
import io.github.b_vitamins.slipbox.ui.GlossaryReviewState
import io.github.b_vitamins.slipbox.ui.GlossaryScreen
import io.github.b_vitamins.slipbox.ui.LibraryScreen
import io.github.b_vitamins.slipbox.ui.MentionDiscoveryPhase
import io.github.b_vitamins.slipbox.ui.ReaderExplorationPhase
import io.github.b_vitamins.slipbox.ui.ReaderScreen
import io.github.b_vitamins.slipbox.ui.ReaderSurfaceKind
import io.github.b_vitamins.slipbox.ui.RandomNotePhase
import io.github.b_vitamins.slipbox.ui.RandomNoteState
import io.github.b_vitamins.slipbox.ui.RelatedDiscoveryPhase
import io.github.b_vitamins.slipbox.ui.SourceSettingsScreen
import io.github.b_vitamins.slipbox.ui.rememberDocumentReaderState
import io.github.b_vitamins.slipbox.ui.rememberCorpusSearchState
import io.github.b_vitamins.slipbox.ui.rememberDirectedRelationsState
import io.github.b_vitamins.slipbox.ui.rememberGlossaryInventoryState
import io.github.b_vitamins.slipbox.ui.rememberGlossaryReviewState
import io.github.b_vitamins.slipbox.ui.rememberGlossaryTermReaderState
import io.github.b_vitamins.slipbox.ui.rememberReaderDiscoveryState
import io.github.b_vitamins.slipbox.ui.rememberReaderExplorationState
import io.github.b_vitamins.slipbox.ui.rememberRandomNoteState
import io.github.b_vitamins.slipbox.ui.content.DocumentGesture
import io.github.b_vitamins.slipbox.ui.content.DocumentIntent
import io.github.b_vitamins.slipbox.ui.content.DocumentPosition
import io.github.b_vitamins.slipbox.ui.content.RepositoryAssets
import io.github.b_vitamins.slipbox.ui.content.SystemExternalLinkHandoff
import io.github.b_vitamins.slipbox.ui.content.isRepositoryAssetTarget
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.settings.rememberReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

@Composable
fun SlipboxApp() {
    SlipboxApp(rememberSourceLibraryState())
}

@Composable
internal fun SlipboxApp(library: SourceLibraryState) {
    val settings = rememberReadingSettings()
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    SlipboxTheme(appearance = settings.preferences.appearance) {
        when (val phase = library.phase) {
            SourceLibraryPhase.Loading ->
                LibraryScreen(
                    phase = phase,
                    hasSources = library.catalog?.sources?.isNotEmpty() == true,
                    appearance = settings.preferences.appearance,
                    reduceMotion = settings.preferences.reduceMotion,
                    onSelectAppearance = settings::select,
                    onSelectReduceMotion = settings::selectReduceMotion,
                    onOpenAbout = {},
                )
            is SourceLibraryPhase.Ready -> {
                val trail = rememberReadingTrailSession(phase.source.binding)
                val readingReturns = rememberReadingReturnsState(phase.source)
                val glossary = rememberGlossaryInventoryState(phase.source)
                val glossaryReview = rememberGlossaryReviewState(phase.source)
                val search = rememberCorpusSearchState(phase.source)
                val random = rememberRandomNoteState(phase.source)
                val destinations =
                    remember(
                        settings,
                        library,
                        readingReturns,
                        glossary,
                        glossaryReview,
                        search,
                        random,
                    ) {
                        productionDestinations(
                            settings,
                            library,
                            readingReturns,
                            glossary,
                            glossaryReview,
                            search,
                            random,
                        )
                    }
                key(phase.source.binding) {
                    if (trail == null || readingReturns == null) {
                        LibraryScreen(
                            phase = SourceLibraryPhase.Loading,
                            hasSources = true,
                            appearance = settings.preferences.appearance,
                            reduceMotion = settings.preferences.reduceMotion,
                            onSelectAppearance = settings::select,
                            onSelectReduceMotion = settings::selectReduceMotion,
                            onOpenAbout = {},
                        )
                    } else {
                        SlipboxNavigation(
                            destinations = destinations,
                            motion = motion,
                            generations = library,
                            restored = trail.restored,
                            trails = trail.sink,
                        )
                    }
                }
            }
            else -> {
                val destinations =
                    remember(settings, library) { productionDestinations(settings, library) }
                SlipboxNavigation(destinations = destinations, motion = motion, generations = library)
            }
        }
    }
}

private fun productionDestinations(
    settings: ReadingSettings,
    library: SourceLibraryState,
    readingReturns: ReadingReturnsState? = null,
    glossary: GlossaryInventoryState? = null,
    glossaryReview: GlossaryReviewState? = null,
    search: CorpusSearchState? = null,
    random: RandomNoteState? = null,
): SlipboxDestinations =
    slipboxDestinations {
        surface(SlipboxSurface.Library) { route, backStack ->
            val libraryRoute = route as SlipboxRoute.Library
            val catalog = library.catalog
            val ready = (library.phase as? SourceLibraryPhase.Ready)?.source
            LaunchedEffect(search, libraryRoute.query) { search?.restore(libraryRoute.query) }
            LibraryScreen(
                phase = library.phase,
                hasSources = catalog?.sources?.isNotEmpty() == true,
                onConnect = { backStack.open(SlipboxRoute.Connection()) },
                onManageSources = {
                    val source = catalog?.activeSource ?: catalog?.sources?.firstOrNull()
                    source?.let { backStack.open(SlipboxRoute.SourceSettings(it.id)) }
                },
                onRetry = library::reload,
                onOpenGlossary = {
                    ready?.let { backStack.open(SlipboxRoute.Glossary(it.binding)) }
                },
                searchInput = search?.input ?: androidx.compose.ui.text.input.TextFieldValue(),
                searchPhase = search?.phase ?: CorpusSearchPhase.Dormant,
                onSearchChange = { input ->
                    if (io.github.b_vitamins.slipbox.navigation.isSavedText(input.text)) {
                        search?.update(input)
                        backStack.rememberLibrarySearch(libraryRoute, input.text)
                    }
                },
                onRetrySearch = { search?.retry() },
                onOpenSearchHit = { hit ->
                    ready?.let { source ->
                        search?.select(hit)
                        backStack.open(hit.readingRoute(source.binding))
                    }
                },
                readingReturns = readingReturns?.snapshot,
                appearance = settings.preferences.appearance,
                reduceMotion = settings.preferences.reduceMotion,
                onSelectAppearance = settings::select,
                onSelectReduceMotion = settings::selectReduceMotion,
                randomPhase = random?.phase ?: RandomNotePhase.Idle,
                onSurpriseMe = {
                    ready?.let { source ->
                        random?.choose { node ->
                            val destination =
                                if (node.glossary) {
                                    SlipboxRoute.Glossary(source.binding, term = node.nodeKey)
                                } else {
                                    SlipboxRoute.Reader(
                                        BoundNote(
                                            binding = source.binding,
                                            nodeKey = node.nodeKey,
                                            explicitId = node.explicitId,
                                            filePath = node.filePath,
                                        ),
                                    )
                                }
                            backStack.open(destination)
                        }
                    }
                },
                onOpenReadingReturn = { entry ->
                    if (entry.availability == ReadingReturnAvailability.Available) {
                        backStack.open(SlipboxRoute.Reader(entry.note, entry.anchor))
                    }
                },
                onRemoveBookmark = { readingReturns?.removeBookmark(it) },
                onRemoveRecent = { readingReturns?.removeRecent(it) },
                onClearBookmarks = { readingReturns?.clearBookmarks() },
                onClearRecents = { readingReturns?.clearRecents() },
                onOpenAbout = { backStack.open(SlipboxRoute.About) },
            )
        }
        surface(SlipboxSurface.Reader) { route, backStack ->
            val readerRoute = route as SlipboxRoute.Reader
            val ready =
                (library.phase as? SourceLibraryPhase.Ready)
                    ?.source
                    ?.takeIf { it.binding == readerRoute.note.binding }
            if (ready == null) {
                ReaderScreen(
                    phase = DocumentReaderPhase.SourceUnavailable,
                    settings = settings,
                    onBack = { backStack.back() },
                    onRetry = {},
                )
            } else {
                val reader = rememberDocumentReaderState(readerRoute.note, ready)
                val document = (reader.phase as? DocumentReaderPhase.Ready)?.document
                val resolved = document?.anchor
                val addressed = document?.addressedAnchor
                val resolvedBound = resolved?.let(readerRoute.note::resolvedBy)
                val resolvedNote = resolvedBound ?: readerRoute.note
                val relations = rememberDirectedRelationsState(resolvedBound, ready)
                val discoveries = rememberReaderDiscoveryState(resolvedBound, ready)
                val exploration =
                    rememberReaderExplorationState(resolvedBound, resolved, ready)
                LaunchedEffect(
                    addressed?.nodeKey,
                    addressed?.explicitId,
                    resolved?.nodeKey,
                    resolved?.explicitId,
                ) {
                    addressed?.let { backStack.reconcileReadingNote(readerRoute, it) }
                    resolved?.let { readingReturns?.recordRecent(it, readerRoute.anchor) }
                }
                val context = LocalContext.current
                val external = remember(context) { SystemExternalLinkHandoff(context) }
                val assets =
                    remember(context, ready.binding, ready.contentRoot) {
                        RepositoryAssets.packaged(context, ready.binding, ready.contentRoot)
                    }
                val openExternal: (DocumentLinkResolution.External, DocumentPosition) -> Boolean =
                    { resolution, position ->
                        val anchor = position.toReadingAnchor()
                        val remembered = backStack.rememberReadingPlace(readerRoute, anchor)
                        if (remembered) readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                        remembered && external.open(resolution.url)
                    }
                val follow: (String, DocumentPosition) -> Unit = { target, position ->
                    reader.follow(target) { resolution ->
                        when (resolution) {
                            is DocumentLinkResolution.Note -> {
                                readingReturns?.rememberReadingPlace(
                                    resolvedNote,
                                    position.toReadingAnchor(),
                                )
                                backStack.follow(
                                    origin = readerRoute,
                                    targetNodeKey = resolution.nodeKey,
                                    originAnchor = position.toReadingAnchor(),
                                )
                            }
                            is DocumentLinkResolution.External -> {
                                if (!openExternal(resolution, position)) {
                                    reader.externalUnavailable()
                                }
                            }
                            DocumentLinkResolution.Missing,
                            DocumentLinkResolution.Unsupported,
                            -> Unit
                        }
                    }
                }
                ReaderScreen(
                    phase = reader.phase,
                    settings = settings,
                    onBack = { backStack.back() },
                    onRetry = reader::retry,
                    bookmarked = readingReturns?.isBookmarked(resolvedNote) == true,
                    onToggleBookmark = { resolved?.let { readingReturns?.toggleBookmark(it) } },
                    onOpenFilingNeighbor = { neighbor ->
                        val anchor = readerRoute.anchor
                        readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                        backStack.follow(
                            origin = readerRoute,
                            targetNodeKey = neighbor.nodeKey,
                            originAnchor = anchor,
                        )
                    },
                    resolveAsset = assets,
                    linkPhase = reader.linkPhase,
                    previewPhase = reader.previewPhase,
                    focusRequest = reader.focusRequest,
                    initialAnchor = readerRoute.anchor,
                    onIntent = { intent ->
                        when (intent) {
                            is DocumentIntent.Glance ->
                                if (intent.gesture == DocumentGesture.Touch) {
                                    if (isRepositoryAssetTarget(intent.link.target)) {
                                        reader.openAttachment(intent.link.target, assets)
                                    } else {
                                        follow(
                                            intent.link.target,
                                            intent.position
                                                ?: DocumentPosition(progress = intent.progress),
                                        )
                                    }
                                }
                            is DocumentIntent.Pin ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(
                                        intent.link.target,
                                        intent.position
                                            ?: DocumentPosition(progress = intent.progress),
                                    )
                                }
                            is DocumentIntent.Go ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(
                                        intent.link.target,
                                        intent.position
                                            ?: DocumentPosition(progress = intent.progress),
                                    )
                                }
                            is DocumentIntent.Position -> {
                                val anchor = intent.position.toReadingAnchor()
                                if (backStack.rememberReadingPlace(
                                    readerRoute,
                                    anchor,
                                )) {
                                    readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                                }
                            }
                            DocumentIntent.Dismiss -> reader.dismissPreview()
                        }
                    },
                    onDismissPreview = reader::dismissPreview,
                    onDismissLinkNotice = reader::dismissLinkNotice,
                    onOpenPreview = {
                        reader.openPreview { preview ->
                            val anchor =
                                preview.request.originPosition?.toReadingAnchor()
                                    ?: ReadingAnchor(progress = preview.request.originProgress)
                            readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                            backStack.follow(
                                origin = readerRoute,
                                targetNodeKey = preview.anchor.nodeKey,
                                originAnchor = anchor,
                            )
                        }
                    },
                    relationsPhase = relations?.phase ?: DirectedRelationsPhase.Idle,
                    onOpenRelations = { relations?.activate() },
                    onRetryRelations = { relations?.retry() },
                    onLoadMoreRelations = { relations?.loadMore() },
                    onPreviewRelation = { relation ->
                        val anchor = readerRoute.anchor
                        reader.previewNode(
                            nodeKey = relation.note.nodeKey,
                            gesture = DocumentGesture.Touch,
                            originProgress = anchor.progress,
                            origin = "relation:${relation.note.nodeKey}",
                            originPosition =
                                DocumentPosition(
                                    mark = anchor.mark,
                                    progress = anchor.progress,
                                    offset = anchor.offset,
                                ),
                        )
                    },
                    onOpenRelation = { relation ->
                        val anchor = readerRoute.anchor
                        readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                        backStack.follow(
                            origin = readerRoute,
                            targetNodeKey = relation.note.nodeKey,
                            originAnchor = anchor,
                        )
                    },
                    relatedPhase = discoveries?.related ?: RelatedDiscoveryPhase.Idle,
                    mentionPhase = discoveries?.mentions ?: MentionDiscoveryPhase.Idle,
                    onRevealRelated = { discoveries?.revealRelated() },
                    onRefreshRelated = { discoveries?.refreshRelated() },
                    onShowAllRelated = { discoveries?.showAllRelated() },
                    onRevealMentions = { discoveries?.revealMentions() },
                    onRefreshMentions = { discoveries?.refreshMentions() },
                    onShowAllMentions = { discoveries?.showAllMentions() },
                    onPreviewDiscovered = { note ->
                        val anchor = readerRoute.anchor
                        reader.previewNode(
                            nodeKey = note.nodeKey,
                            gesture = DocumentGesture.Touch,
                            originProgress = anchor.progress,
                            origin = "relation:${note.nodeKey}",
                            originPosition =
                                DocumentPosition(
                                    mark = anchor.mark,
                                    progress = anchor.progress,
                                    offset = anchor.offset,
                                ),
                        )
                    },
                    onOpenDiscovered = { note ->
                        val anchor = readerRoute.anchor
                        readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                        backStack.follow(
                            origin = readerRoute,
                            targetNodeKey = note.nodeKey,
                            originAnchor = anchor,
                        )
                    },
                    explorationPhase =
                        exploration?.phase ?: ReaderExplorationPhase.AwaitingLens,
                    onSelectExplorationLens = { lens -> exploration?.select(lens) },
                    onRefreshExploration = { exploration?.refresh() },
                    onPreviewExploration = { note ->
                        val anchor = readerRoute.anchor
                        reader.previewNode(
                            nodeKey = note.nodeKey,
                            gesture = DocumentGesture.Touch,
                            originProgress = anchor.progress,
                            origin = "explore:${note.nodeKey}",
                            originPosition =
                                DocumentPosition(
                                    mark = anchor.mark,
                                    progress = anchor.progress,
                                    offset = anchor.offset,
                                ),
                        )
                    },
                    onOpenExploration = { note ->
                        val anchor = readerRoute.anchor
                        readingReturns?.rememberReadingPlace(resolvedNote, anchor)
                        backStack.follow(
                            origin = readerRoute,
                            targetNodeKey = note.nodeKey,
                            originAnchor = anchor,
                        )
                    },
                )
            }
        }
        surface(SlipboxSurface.Glossary) { route, backStack ->
            val glossaryRoute = route as SlipboxRoute.Glossary
            val ready =
                (library.phase as? SourceLibraryPhase.Ready)
                    ?.source
                    ?.takeIf { it.binding == glossaryRoute.binding }
            val termKey = glossaryRoute.term
            if (ready == null) {
                ReaderScreen(
                    phase = DocumentReaderPhase.SourceUnavailable,
                    settings = settings,
                    kind = ReaderSurfaceKind.GlossaryTerm,
                    onBack = { backStack.back() },
                    onRetry = {},
                )
            } else if (termKey == null) {
                GlossaryScreen(
                    phase = glossary?.phase ?: GlossaryInventoryPhase.Dormant,
                    onBack = { backStack.back() },
                    onShowNotes = {
                        while (backStack.current is SlipboxRoute.Glossary) backStack.back()
                    },
                    onManageSources = {
                        val catalog = library.catalog
                        val source = catalog?.activeSource ?: catalog?.sources?.firstOrNull()
                        source?.let { backStack.open(SlipboxRoute.SourceSettings(it.id)) }
                    },
                    onOpenAbout = { backStack.open(SlipboxRoute.About) },
                    appearance = settings.preferences.appearance,
                    reduceMotion = settings.preferences.reduceMotion,
                    onSelectAppearance = settings::select,
                    onSelectReduceMotion = settings::selectReduceMotion,
                    onActivate = { glossary?.activate() },
                    onLoadMore = { glossary?.loadMore() },
                    onRetry = { glossary?.retry() },
                    selectedTermNodeKey =
                        (backStack.current as? SlipboxRoute.Glossary)
                            ?.takeIf { it.binding == glossaryRoute.binding }
                            ?.term,
                    onOpenTerm = { term ->
                        backStack.open(glossaryRoute.copy(term = term.nodeKey))
                    },
                    review = glossaryRoute.review,
                    reviewInput =
                        glossaryReview?.input ?: androidx.compose.ui.text.input.TextFieldValue(),
                    reviewPhase = glossaryReview?.phase ?: GlossaryReviewPhase.Dormant,
                    onShowAll = {
                        val query = glossaryReview?.input?.text ?: glossaryRoute.query
                        backStack.rememberGlossaryView(glossaryRoute, review = false, query = query)
                    },
                    onShowDue = {
                        val query = glossaryReview?.input?.text ?: glossaryRoute.query
                        backStack.rememberGlossaryView(glossaryRoute, review = true, query = query)
                    },
                    onActivateReview = { glossaryReview?.activate(glossaryRoute.query) },
                    onReviewQueryChange = { input ->
                        if (io.github.b_vitamins.slipbox.navigation.isSavedText(input.text)) {
                            glossaryReview?.update(input)
                            backStack.rememberGlossaryView(
                                glossaryRoute,
                                review = true,
                                query = input.text,
                            )
                        }
                    },
                    onLoadMoreReview = { glossaryReview?.loadMore() },
                    onRetryReview = { glossaryReview?.retry() },
                )
            } else {
                val reader =
                    rememberGlossaryTermReaderState(
                        BoundNote(binding = glossaryRoute.binding, nodeKey = termKey),
                        ready,
                    )
                val context = LocalContext.current
                val external = remember(context) { SystemExternalLinkHandoff(context) }
                val assets =
                    remember(context, ready.binding, ready.contentRoot) {
                        RepositoryAssets.packaged(context, ready.binding, ready.contentRoot)
                    }
                val openExternal: (DocumentLinkResolution.External) -> Boolean = { resolution ->
                    external.open(resolution.url)
                }
                val follow: (String) -> Unit = { target ->
                    reader.follow(target) { resolution ->
                        when (resolution) {
                            is DocumentLinkResolution.Note ->
                                backStack.open(
                                    SlipboxRoute.Reader(
                                        BoundNote(glossaryRoute.binding, resolution.nodeKey),
                                    ),
                                )
                            is DocumentLinkResolution.External ->
                                if (!openExternal(resolution)) reader.externalUnavailable()
                            DocumentLinkResolution.Missing,
                            DocumentLinkResolution.Unsupported,
                            -> Unit
                        }
                    }
                }
                ReaderScreen(
                    phase = reader.phase,
                    settings = settings,
                    kind = ReaderSurfaceKind.GlossaryTerm,
                    onBack = { backStack.back() },
                    onRetry = reader::retry,
                    resolveAsset = assets,
                    linkPhase = reader.linkPhase,
                    previewPhase = reader.previewPhase,
                    focusRequest = reader.focusRequest,
                    onIntent = { intent ->
                        when (intent) {
                            is DocumentIntent.Glance ->
                                if (intent.gesture == DocumentGesture.Touch) {
                                    if (isRepositoryAssetTarget(intent.link.target)) {
                                        reader.openAttachment(intent.link.target, assets)
                                    } else {
                                        follow(intent.link.target)
                                    }
                                }
                            is DocumentIntent.Pin ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(intent.link.target)
                                }
                            is DocumentIntent.Go ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(intent.link.target)
                                }
                            is DocumentIntent.Position -> Unit
                            DocumentIntent.Dismiss -> reader.dismissPreview()
                        }
                    },
                    onDismissPreview = reader::dismissPreview,
                    onDismissLinkNotice = reader::dismissLinkNotice,
                    onOpenPreview = {
                        reader.openPreview { preview ->
                            if (preview.anchor.glossary) {
                                backStack.open(glossaryRoute.copy(term = preview.anchor.nodeKey))
                            } else {
                                backStack.open(
                                    SlipboxRoute.Reader(
                                        BoundNote(
                                            binding = glossaryRoute.binding,
                                            nodeKey = preview.anchor.nodeKey,
                                            explicitId = preview.anchor.explicitId,
                                            filePath = preview.anchor.filePath,
                                        ),
                                    ),
                                )
                            }
                        }
                    },
                )
            }
        }
        surface(SlipboxSurface.Connection) { _, backStack ->
            val revision = library.catalogRevision()
            if (revision == null) {
                LibraryScreen(
                    phase = library.phase,
                    hasSources = library.catalog?.sources?.isNotEmpty() == true,
                    onConnect = {},
                    onRetry = library::reload,
                    appearance = settings.preferences.appearance,
                    reduceMotion = settings.preferences.reduceMotion,
                    onSelectAppearance = settings::select,
                    onSelectReduceMotion = settings::selectReduceMotion,
                    onOpenAbout = { backStack.open(SlipboxRoute.About) },
                )
            } else {
                ConnectionScreen(
                    catalogRevision = revision,
                    onBack = { backStack.back() },
                    onReady = { source, catalogRevision ->
                        library.activate(source, catalogRevision)
                        backStack.switchSource(source.binding)
                        backStack.back()
                    },
                )
            }
        }
        surface(SlipboxSurface.SourceSettings) { route, backStack ->
            val sourceRoute = route as SlipboxRoute.SourceSettings
            val catalog = library.catalog
            if (catalog == null) {
                LibraryScreen(
                    phase = library.phase,
                    hasSources = false,
                    onRetry = library::reload,
                    appearance = settings.preferences.appearance,
                    reduceMotion = settings.preferences.reduceMotion,
                    onSelectAppearance = settings::select,
                    onSelectReduceMotion = settings::selectReduceMotion,
                    onOpenAbout = { backStack.open(SlipboxRoute.About) },
                )
            } else {
                SourceSettingsScreen(
                    sourceId = sourceRoute.source,
                    catalog = catalog,
                    ready = (library.phase as? SourceLibraryPhase.Ready)?.source,
                    onBack = { backStack.back() },
                    onOpenSource = { source ->
                        backStack.open(SlipboxRoute.SourceSettings(source.id))
                    },
                    onSelect = { source, completed ->
                        library.select(source) { selected ->
                            if (selected is SourceCatalogResult.Active) {
                                backStack.switchSource(selected.ready.binding)
                                while (backStack.current is SlipboxRoute.SourceSettings) {
                                    backStack.back()
                                }
                                completed(true)
                            } else {
                                completed(false)
                            }
                        }
                    },
                    onReady = { source, revision ->
                        library.configured(source, revision)
                    },
                    onCacheRemoved = { source ->
                        val wasActive = library.catalog?.activeSource?.id == source.id
                        if (readingReturns?.source == source.id) readingReturns.clearAll()
                        library.cacheRemoved(source)
                        backStack.removeSource(source.id, clearSearch = wasActive)
                        while (backStack.current is SlipboxRoute.SourceSettings) {
                            backStack.back()
                        }
                    },
                    onSourceRemoved = { listing, source, cleanupComplete ->
                        val wasActive = library.catalog?.activeSource?.id == source.id
                        if (readingReturns?.source == source.id) readingReturns.clearAll()
                        library.removed(listing)
                        backStack.removeSource(
                            source.id,
                            keepSettings = !cleanupComplete,
                            clearSearch = wasActive,
                        )
                        if (cleanupComplete) {
                            while (backStack.current is SlipboxRoute.SourceSettings) {
                                backStack.back()
                            }
                        }
                    },
                )
            }
        }
        surface(SlipboxSurface.About) { _, backStack ->
            AboutScreen(onBack = { backStack.back() }, settings = settings)
        }
    }

private fun DocumentPosition.toReadingAnchor(): ReadingAnchor =
    ReadingAnchor(mark = mark, progress = progress, offset = offset)
