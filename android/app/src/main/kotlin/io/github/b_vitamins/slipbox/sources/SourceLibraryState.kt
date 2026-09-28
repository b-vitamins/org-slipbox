/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.navigation.SourceGenerations
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.SourceRefreshScheduler
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

internal sealed interface SourceLibraryPhase {

    data object Loading : SourceLibraryPhase

    data class Empty(val catalogRevision: Long) : SourceLibraryPhase

    data class Ready(val source: ReadySource, val catalogRevision: Long) : SourceLibraryPhase

    data class Failed(val failure: SourceCatalogResult.Failed) : SourceLibraryPhase
}

@Stable
internal class SourceLibraryState(
    private val gateway: SourceLibraryCatalog,
    private val startup: (Iterable<RefreshSource>) -> Unit = {},
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : SourceGenerations, AutoCloseable {

    private val live = AtomicBoolean(true)
    private val reloads = AtomicLong()

    var phase: SourceLibraryPhase by mutableStateOf(SourceLibraryPhase.Loading)
        private set

    var catalog: SourceCatalogListing? by mutableStateOf(null)
        private set

    init {
        reload()
    }

    fun reload() {
        val serial = reloads.incrementAndGet()
        phase = SourceLibraryPhase.Loading
        Thread(
                {
                    val listed = gateway.list()
                    val snapshot = (listed as? SourceCatalogListingResult.Loaded)?.listing
                    val loaded =
                        snapshot?.let(gateway::load)
                            ?: (listed as SourceCatalogListingResult.Failed).failure
                    delivery.post {
                        if (live.get() && reloads.get() == serial) {
                            catalog = snapshot
                            phase =
                                when (loaded) {
                                    is SourceCatalogResult.Empty ->
                                        SourceLibraryPhase.Empty(loaded.revision)
                                    is SourceCatalogResult.Active ->
                                        SourceLibraryPhase.Ready(loaded.ready, loaded.revision)
                                    is SourceCatalogResult.Failed -> SourceLibraryPhase.Failed(loaded)
                                }
                        }
                    }
                    if (live.get() && reloads.get() == serial && snapshot != null) {
                        startup(snapshot.sources)
                    }
                },
                "slipbox-source-catalog",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    fun activate(source: ReadySource, catalogRevision: Long) {
        val current = catalog
        val sources =
            current
                ?.sources
                ?.map { if (it.id == source.source.id) source.source else it }
                ?.toMutableList()
                ?: mutableListOf()
        if (sources.none { it.id == source.source.id }) sources.add(source.source)
        catalog = SourceCatalogListing(catalogRevision, sources, source.source)
        phase = SourceLibraryPhase.Ready(source, catalogRevision)
    }

    /** Publish a replaced configuration without selecting an inactive source locally. */
    fun configured(source: ReadySource, catalogRevision: Long): Boolean {
        val current = catalog ?: return false
        val wasActive = current.activeSource?.id == source.source.id
        val sources =
            current.sources.map { candidate ->
                if (candidate.id == source.source.id) source.source else candidate
            }
        val activeSource = if (wasActive) source.source else current.activeSource
        catalog = SourceCatalogListing(catalogRevision, sources, activeSource)
        val currentPhase = phase
        phase =
            when {
                wasActive -> SourceLibraryPhase.Ready(source, catalogRevision)
                currentPhase is SourceLibraryPhase.Ready ->
                    SourceLibraryPhase.Ready(
                        currentPhase.source,
                        catalogRevision,
                    )
                currentPhase is SourceLibraryPhase.Empty -> SourceLibraryPhase.Empty(catalogRevision)
                else -> currentPhase
            }
        return wasActive
    }

    fun select(source: RefreshSource, recipient: (SourceCatalogResult) -> Unit) {
        val expected = catalog?.revision
            ?: return recipient(SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE))
        Thread(
                {
                    val selected = gateway.activate(expected, source)
                    delivery.post {
                        if (!live.get()) return@post
                        if (selected is SourceCatalogResult.Active) {
                            activate(selected.ready, selected.revision)
                        }
                        recipient(selected)
                    }
                },
                "slipbox-source-selection",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    fun removed(listing: SourceCatalogListing) {
        catalog = listing
        val ready = (phase as? SourceLibraryPhase.Ready)?.source
        phase =
            if (ready != null && listing.activeSource?.id == ready.source.id) {
                SourceLibraryPhase.Ready(ready, listing.revision)
            } else {
                SourceLibraryPhase.Empty(listing.revision)
            }
    }

    fun cacheRemoved(source: RefreshSource) {
        if (catalog?.activeSource?.id == source.id) {
            phase =
                SourceLibraryPhase.Failed(
                    SourceCatalogResult.Failed(
                        failure = SourceCatalogFailure.GENERATION_UNAVAILABLE,
                    ),
                )
        }
    }

    fun catalogRevision(): Long? = catalog?.revision

    override fun readyGeneration(source: String): String? =
        (phase as? SourceLibraryPhase.Ready)
            ?.source
            ?.takeIf { it.binding.source == source }
            ?.binding
            ?.generation

    override fun close() {
        live.set(false)
    }
}

@Composable
internal fun rememberSourceLibraryState(): SourceLibraryState {
    val context = LocalContext.current
    val state =
        remember(context) {
            val scheduler = SourceRefreshScheduler.packaged(context.applicationContext)
            SourceLibraryState(
                gateway = SourceCatalogGateway(context),
                startup = { sources ->
                    scheduler.startup(
                        sources.filter { source ->
                            SourceCredentials.connected(context.applicationContext, source)
                        },
                    )
                },
            )
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
