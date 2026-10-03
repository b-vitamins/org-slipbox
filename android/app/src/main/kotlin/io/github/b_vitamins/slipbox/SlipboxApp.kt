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
import io.github.b_vitamins.slipbox.navigation.slipboxDestinations
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sources.SourceLibraryState
import io.github.b_vitamins.slipbox.sources.rememberSourceLibraryState
import io.github.b_vitamins.slipbox.ui.AboutScreen
import io.github.b_vitamins.slipbox.ui.ConnectionScreen
import io.github.b_vitamins.slipbox.ui.DirectedRelationsPhase
import io.github.b_vitamins.slipbox.ui.DocumentReaderPhase
import io.github.b_vitamins.slipbox.ui.LibraryScreen
import io.github.b_vitamins.slipbox.ui.MentionDiscoveryPhase
import io.github.b_vitamins.slipbox.ui.ReaderExplorationPhase
import io.github.b_vitamins.slipbox.ui.ReaderScreen
import io.github.b_vitamins.slipbox.ui.RelatedDiscoveryPhase
import io.github.b_vitamins.slipbox.ui.SourceSettingsScreen
import io.github.b_vitamins.slipbox.ui.rememberDocumentReaderState
import io.github.b_vitamins.slipbox.ui.rememberDirectedRelationsState
import io.github.b_vitamins.slipbox.ui.rememberNotesInventoryState
import io.github.b_vitamins.slipbox.ui.rememberReaderDiscoveryState
import io.github.b_vitamins.slipbox.ui.rememberReaderExplorationState
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
    val settings = rememberReadingSettings()
    val library = rememberSourceLibraryState()
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    SlipboxTheme(appearance = settings.preferences.appearance) {
        when (val phase = library.phase) {
            SourceLibraryPhase.Loading ->
                LibraryScreen(
                    phase = phase,
                    hasSources = library.catalog?.sources?.isNotEmpty() == true,
                    onOpenAbout = {},
                )
            is SourceLibraryPhase.Ready -> {
                val trail = rememberReadingTrailSession(phase.source.binding)
                val readingReturns = rememberReadingReturnsState(phase.source)
                val destinations =
                    remember(settings, library, readingReturns) {
                        productionDestinations(settings, library, readingReturns)
                    }
                key(phase.source.binding) {
                    if (trail == null || readingReturns == null) {
                        LibraryScreen(
                            phase = SourceLibraryPhase.Loading,
                            hasSources = true,
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
): SlipboxDestinations =
    slipboxDestinations {
        surface(SlipboxSurface.Library) { _, backStack ->
            val catalog = library.catalog
            val ready = (library.phase as? SourceLibraryPhase.Ready)?.source
            val inventory = ready?.let { rememberNotesInventoryState(it) }
            LibraryScreen(
                phase = library.phase,
                hasSources = catalog?.sources?.isNotEmpty() == true,
                onConnect = { backStack.open(SlipboxRoute.Connection()) },
                onManageSources = {
                    val source = catalog?.activeSource ?: catalog?.sources?.firstOrNull()
                    source?.let { backStack.open(SlipboxRoute.SourceSettings(it.id)) }
                },
                onRetry = library::reload,
                inventory = inventory?.phase,
                onLoadMore = { inventory?.loadMore() },
                onRetryInventory = { inventory?.retry() },
                onOpenNote = { note ->
                    ready?.let {
                        backStack.open(
                            SlipboxRoute.Reader(
                                BoundNote(
                                    binding = it.binding,
                                    nodeKey = note.nodeKey,
                                    explicitId = note.explicitId,
                                    filePath = note.filePath,
                                ),
                            ),
                        )
                    }
                },
                readingReturns = readingReturns?.snapshot,
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
                val resolved = (reader.phase as? DocumentReaderPhase.Ready)?.document?.anchor
                val resolvedBound = resolved?.let(readerRoute.note::resolvedBy)
                val resolvedNote = resolvedBound ?: readerRoute.note
                val relations = rememberDirectedRelationsState(resolvedBound, ready)
                val discoveries = rememberReaderDiscoveryState(resolvedBound, ready)
                val exploration =
                    rememberReaderExplorationState(resolvedBound, resolved, ready)
                LaunchedEffect(resolved?.nodeKey, resolved?.explicitId) {
                    resolved?.let {
                        backStack.reconcileReadingNote(readerRoute, it)
                        readingReturns?.recordRecent(it, readerRoute.anchor)
                    }
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
                    resolveAsset = assets,
                    linkPhase = reader.linkPhase,
                    previewPhase = reader.previewPhase,
                    focusRequest = reader.focusRequest,
                    initialAnchor = readerRoute.anchor,
                    onIntent = { intent ->
                        when (intent) {
                            is DocumentIntent.Glance ->
                                if (intent.gesture == DocumentGesture.Touch) {
                                    reader.preview(
                                        target = intent.link.target,
                                        gesture = intent.gesture,
                                        originProgress = intent.progress,
                                        origin = intent.origin,
                                        originPosition = intent.position,
                                        onExternal = { resolution ->
                                            openExternal(
                                                resolution,
                                                intent.position
                                                    ?: DocumentPosition(progress = intent.progress),
                                            )
                                        },
                                    )
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
        surface(SlipboxSurface.Connection) { _, backStack ->
            val revision = library.catalogRevision()
            if (revision == null) {
                LibraryScreen(
                    phase = library.phase,
                    hasSources = library.catalog?.sources?.isNotEmpty() == true,
                    onConnect = {},
                    onRetry = library::reload,
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
