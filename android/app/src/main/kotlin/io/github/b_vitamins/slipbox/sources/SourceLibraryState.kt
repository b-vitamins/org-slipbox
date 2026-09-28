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
    private val gateway: SourceCatalogGateway,
    private val startup: (RefreshSource) -> Unit = {},
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : SourceGenerations, AutoCloseable {

    private val live = AtomicBoolean(true)
    private val reloads = AtomicLong()

    var phase: SourceLibraryPhase by mutableStateOf(SourceLibraryPhase.Loading)
        private set

    init {
        reload()
    }

    fun reload() {
        val serial = reloads.incrementAndGet()
        phase = SourceLibraryPhase.Loading
        Thread(
                {
                    val loaded = gateway.load()
                    if (live.get() && reloads.get() == serial && loaded is SourceCatalogResult.Active) {
                        startup(loaded.ready.source)
                    }
                    delivery.post {
                        if (live.get() && reloads.get() == serial) {
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
                },
                "slipbox-source-catalog",
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    fun activate(source: ReadySource, catalogRevision: Long) {
        phase = SourceLibraryPhase.Ready(source, catalogRevision)
    }

    fun catalogRevision(): Long? =
        when (val current = phase) {
            is SourceLibraryPhase.Empty -> current.catalogRevision
            is SourceLibraryPhase.Ready -> current.catalogRevision
            is SourceLibraryPhase.Loading, is SourceLibraryPhase.Failed -> null
        }

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
                startup = { source -> scheduler.startup(listOf(source)) },
            )
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
