/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.engine.DocumentLinkResolution
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.navigation.ReadingAnchor
import io.github.b_vitamins.slipbox.navigation.SlipboxDestinations
import io.github.b_vitamins.slipbox.navigation.SlipboxNavigation
import io.github.b_vitamins.slipbox.navigation.SlipboxRoute
import io.github.b_vitamins.slipbox.navigation.SlipboxSurface
import io.github.b_vitamins.slipbox.navigation.slipboxDestinations
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sources.SourceLibraryState
import io.github.b_vitamins.slipbox.sources.rememberSourceLibraryState
import io.github.b_vitamins.slipbox.ui.AboutScreen
import io.github.b_vitamins.slipbox.ui.ConnectionScreen
import io.github.b_vitamins.slipbox.ui.DocumentReaderPhase
import io.github.b_vitamins.slipbox.ui.LibraryScreen
import io.github.b_vitamins.slipbox.ui.ReaderScreen
import io.github.b_vitamins.slipbox.ui.SourceSettingsScreen
import io.github.b_vitamins.slipbox.ui.rememberDocumentReaderState
import io.github.b_vitamins.slipbox.ui.rememberNotesInventoryState
import io.github.b_vitamins.slipbox.ui.content.DocumentGesture
import io.github.b_vitamins.slipbox.ui.content.DocumentIntent
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
    val destinations = remember(settings, library) { productionDestinations(settings, library) }
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    SlipboxTheme(appearance = settings.preferences.appearance) {
        SlipboxNavigation(destinations = destinations, motion = motion, generations = library)
    }
}

private fun productionDestinations(
    settings: ReadingSettings,
    library: SourceLibraryState,
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
                        backStack.open(SlipboxRoute.Reader(BoundNote(it.binding, note.nodeKey)))
                    }
                },
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
                val context = LocalContext.current
                val external = remember(context) { SystemExternalLinkHandoff(context) }
                val assets =
                    remember(context, ready.binding, ready.contentRoot) {
                        RepositoryAssets.packaged(context, ready.binding, ready.contentRoot)
                    }
                val openExternal: (DocumentLinkResolution.External, Float) -> Boolean =
                    { resolution, progress ->
                        backStack.rememberReadingPlace(
                            readerRoute,
                            ReadingAnchor(progress = progress),
                        ) && external.open(resolution.url)
                    }
                val follow: (String, Float) -> Unit = { target, progress ->
                    reader.follow(target) { resolution ->
                        when (resolution) {
                            is DocumentLinkResolution.Note ->
                                backStack.follow(
                                    origin = readerRoute,
                                    targetNodeKey = resolution.nodeKey,
                                    originAnchor = ReadingAnchor(progress = progress),
                                )
                            is DocumentLinkResolution.External -> {
                                if (!openExternal(resolution, progress)) {
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
                    resolveAsset = assets,
                    linkPhase = reader.linkPhase,
                    previewPhase = reader.previewPhase,
                    focusRequest = reader.focusRequest,
                    initialProgress = readerRoute.anchor.progress,
                    onIntent = { intent ->
                        when (intent) {
                            is DocumentIntent.Glance ->
                                if (intent.gesture == DocumentGesture.Touch) {
                                    reader.preview(
                                        target = intent.link.target,
                                        gesture = intent.gesture,
                                        originProgress = intent.progress,
                                        origin = intent.origin,
                                        onExternal = { resolution ->
                                            openExternal(resolution, intent.progress)
                                        },
                                    )
                                }
                            is DocumentIntent.Pin ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(intent.link.target, intent.progress)
                                }
                            is DocumentIntent.Go ->
                                if (isRepositoryAssetTarget(intent.link.target)) {
                                    reader.openAttachment(intent.link.target, assets)
                                } else {
                                    follow(intent.link.target, intent.progress)
                                }
                            DocumentIntent.Dismiss -> reader.dismissPreview()
                        }
                    },
                    onDismissPreview = reader::dismissPreview,
                    onOpenPreview = {
                        reader.openPreview { preview ->
                            backStack.follow(
                                origin = readerRoute,
                                targetNodeKey = preview.anchor.nodeKey,
                                originAnchor =
                                    ReadingAnchor(progress = preview.request.originProgress),
                            )
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
                        library.cacheRemoved(source)
                        backStack.removeSource(source.id, clearSearch = wasActive)
                        while (backStack.current is SlipboxRoute.SourceSettings) {
                            backStack.back()
                        }
                    },
                    onSourceRemoved = { listing, source, cleanupComplete ->
                        val wasActive = library.catalog?.activeSource?.id == source.id
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
