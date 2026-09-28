/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import android.os.Handler
import android.os.Looper
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import io.github.b_vitamins.slipbox.sync.RefreshState
import io.github.b_vitamins.slipbox.sync.RefreshStatus
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.SourceRefreshOutcome
import io.github.b_vitamins.slipbox.sync.SourceRefreshRuntime
import io.github.b_vitamins.slipbox.sync.SourceRefreshStatusOutcome
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

internal sealed interface InitialSourceImportEvent {

    data class Progress(val status: RefreshStatus?) : InitialSourceImportEvent

    data class Ready(val source: ReadySource, val catalogRevision: Long) : InitialSourceImportEvent

    data class Failed(
        val refresh: SourceRefreshOutcome? = null,
        val catalog: SourceCatalogResult.Failed? = null,
    ) : InitialSourceImportEvent

    data object Cancelled : InitialSourceImportEvent
}

internal fun interface ImportDelivery {

    fun post(action: () -> Unit)

    companion object {

        val MainThread = ImportDelivery { action -> Handler(Looper.getMainLooper()).post(action) }
    }
}

/** Owns exactly one initial import and suppresses delivery from withdrawn work. */
internal class InitialSourceImportOwner(
    private val runtime: SourceRefreshRuntime,
    private val catalog: SourceActivation,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable, DefaultLifecycleObserver {

    private val active = AtomicReference<Import?>(null)
    private val closed = AtomicBoolean()

    fun begin(
        source: RefreshSource,
        catalogRevision: Long,
        listener: (InitialSourceImportEvent) -> Unit,
    ): Boolean {
        if (closed.get()) return false
        val import = Import(source)
        if (!active.compareAndSet(null, import)) return false
        emit(import, listener, InitialSourceImportEvent.Progress(null))
        import.start {
            val monitor = monitor(import, listener)
            val outcome = runtime.execute(source, 0) { operation -> import.operation = operation }
            import.running.set(false)
            monitor.interrupt()
            val event = terminal(import, catalogRevision, outcome)
            settle(import, listener, event)
        }
        return true
    }

    fun cancel() {
        val import = active.get() ?: return
        import.cancelling.set(true)
        runtime.cancel(import.source.id)
    }

    override fun close() {
        closed.set(true)
        val import = active.getAndSet(null) ?: return
        import.running.set(false)
        runtime.cancel(import.source.id)
        import.worker.getAndSet(null)?.interrupt()
    }

    override fun onDestroy(owner: LifecycleOwner) = close()

    private fun monitor(
        import: Import,
        listener: (InitialSourceImportEvent) -> Unit,
    ): Thread =
        Thread(
                {
                    var last: RefreshStatus? = null
                    while (import.running.get()) {
                        when (val outcome = runtime.status(import.source.id)) {
                            is SourceRefreshStatusOutcome.Known ->
                                if (outcome.status != last) {
                                    last = outcome.status
                                    emit(
                                        import,
                                        listener,
                                        InitialSourceImportEvent.Progress(outcome.status),
                                    )
                                }
                            else -> Unit
                        }
                        try {
                            Thread.sleep(POLL_MILLIS)
                        } catch (_: InterruptedException) {
                            break
                        }
                    }
                },
                MONITOR_THREAD,
            )
            .apply {
                isDaemon = true
                start()
            }

    private fun terminal(
        import: Import,
        catalogRevision: Long,
        outcome: SourceRefreshOutcome,
    ): InitialSourceImportEvent {
        if (import.cancelling.get()) return InitialSourceImportEvent.Cancelled
        val answered = outcome as? SourceRefreshOutcome.Answered
            ?: return InitialSourceImportEvent.Failed(refresh = outcome)
        if (answered.status.state == RefreshState.CANCELLED) {
            return InitialSourceImportEvent.Cancelled
        }
        val generation = answered.status.readyGeneration
        if (answered.status.state != RefreshState.READY || generation == null) {
            return InitialSourceImportEvent.Failed(refresh = outcome)
        }
        return when (val committed = catalog.commit(catalogRevision, import.source, generation)) {
            is SourceCatalogResult.Active ->
                InitialSourceImportEvent.Ready(committed.ready, committed.revision)
            is SourceCatalogResult.Failed -> InitialSourceImportEvent.Failed(catalog = committed)
            is SourceCatalogResult.Empty ->
                InitialSourceImportEvent.Failed(
                    catalog = SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED),
                )
        }
    }

    private fun settle(
        import: Import,
        listener: (InitialSourceImportEvent) -> Unit,
        event: InitialSourceImportEvent,
    ) {
        delivery.post {
            if (active.compareAndSet(import, null) && !closed.get()) {
                listener(event)
            }
        }
    }

    private fun emit(
        import: Import,
        listener: (InitialSourceImportEvent) -> Unit,
        event: InitialSourceImportEvent,
    ) {
        delivery.post {
            if (active.get() === import && !closed.get()) listener(event)
        }
    }

    private class Import(val source: RefreshSource) {

        val running = AtomicBoolean(true)
        val cancelling = AtomicBoolean()
        val worker = AtomicReference<Thread?>(null)

        @Volatile var operation: Long? = null

        fun start(work: () -> Unit) {
            val thread = Thread(work, IMPORT_THREAD).apply { isDaemon = true }
            worker.set(thread)
            thread.start()
        }
    }

    private companion object {

        const val IMPORT_THREAD = "slipbox-initial-import"
        const val MONITOR_THREAD = "slipbox-import-status"
        const val POLL_MILLIS = 100L
    }
}
