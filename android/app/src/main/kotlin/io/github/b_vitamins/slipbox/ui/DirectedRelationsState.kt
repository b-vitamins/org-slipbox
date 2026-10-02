/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.github.b_vitamins.slipbox.engine.ADMITTED_LIMITS
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.DirectedRelationsResult
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface DirectedRelationsPhase {

    data object Idle : DirectedRelationsPhase

    data object Loading : DirectedRelationsPhase

    data object Empty : DirectedRelationsPhase

    data class Ready(
        val relations: List<DirectedRelationRecord>,
        val total: Long,
        val incomingTotal: Long,
        val outgoingTotal: Long,
        val hasMore: Boolean,
        val nextPosition: String?,
        val loadingMore: Boolean = false,
        val continuationFailed: Boolean = false,
    ) : DirectedRelationsPhase

    data object Failed : DirectedRelationsPhase
}

internal interface DirectedRelationPageSource : AutoCloseable {

    fun list(nodeKey: String, after: String?, limit: Int): DirectedRelationsResult
}

internal fun interface DirectedRelationPageSourceFactory {

    fun open(ready: ReadySource): DirectedRelationPageSource
}

private object NativeDirectedRelationPageSourceFactory : DirectedRelationPageSourceFactory {

    override fun open(ready: ReadySource): DirectedRelationPageSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : DirectedRelationPageSource {
                override fun list(
                    nodeKey: String,
                    after: String?,
                    limit: Int,
                ): DirectedRelationsResult {
                    val operation = ReadOperation.DirectedRelations(nodeKey, limit, after)
                    return (session.answer(operation).await() as EngineAnswer.DirectedRelations).result
                }

                override fun close() {
                    host.close()
                }
            }
        } catch (failure: Throwable) {
            host.close()
            throw failure
        }
    }
}

/** Owns an on-demand, source-generation-bound relation page stream for one note. */
@Stable
internal class DirectedRelationsState(
    private val note: BoundNote,
    private val ready: ReadySource,
    private val factory: DirectedRelationPageSourceFactory = NativeDirectedRelationPageSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val loading = AtomicBoolean()
    private val requests = AtomicLong()
    private val source = AtomicReference<DirectedRelationPageSource?>()

    var phase: DirectedRelationsPhase by mutableStateOf(DirectedRelationsPhase.Idle)
        private set

    init {
        require(note.binding == ready.binding)
        require(pageSize in 1..ADMITTED_LIMITS.maxRelationEntries)
    }

    fun activate() {
        if (phase != DirectedRelationsPhase.Idle || !live.get()) return
        phase = DirectedRelationsPhase.Loading
        request(restart = true)
    }

    fun loadMore() {
        val current = phase as? DirectedRelationsPhase.Ready ?: return
        if (!current.hasMore || current.loadingMore || current.nextPosition == null) return
        phase = current.copy(loadingMore = true, continuationFailed = false)
        request(restart = false)
    }

    fun retry() {
        when (val current = phase) {
            DirectedRelationsPhase.Failed -> {
                phase = DirectedRelationsPhase.Loading
                request(restart = true)
            }
            is DirectedRelationsPhase.Ready ->
                if (current.continuationFailed) {
                    phase = current.copy(loadingMore = true, continuationFailed = false)
                    request(restart = false)
                }
            else -> Unit
        }
    }

    private fun request(restart: Boolean) {
        if (!live.get() || !loading.compareAndSet(false, true)) return
        val serial = requests.incrementAndGet()
        val current = if (restart) null else phase as DirectedRelationsPhase.Ready
        val before = current?.nextPosition
        val loaded = current?.relations?.size ?: 0
        Thread(
                {
                    val outcome = runCatching {
                        val pages = source.get() ?: openSource()
                        pages.list(note.nodeKey, before, pageSize).also {
                            validate(before, loaded, it)
                        }
                    }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        loading.set(false)
                        outcome.fold(
                            onSuccess = { page ->
                                runCatching { accept(restart, page) }
                                    .onFailure { fail(restart) }
                            },
                            onFailure = { fail(restart) },
                        )
                    }
                },
                WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun openSource(): DirectedRelationPageSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the relation inventory was closed while opening")
        }
        if (!source.compareAndSet(null, opened)) opened.close()
        return checkNotNull(source.get())
    }

    private fun validate(
        before: String?,
        loaded: Int,
        page: DirectedRelationsResult,
    ) {
        require(page.relations.size <= pageSize)
        require(page.total >= 0 && page.incomingTotal >= 0 && page.outgoingTotal >= 0)
        require(page.total >= page.incomingTotal && page.total >= page.outgoingTotal)
        require(page.total <= page.incomingTotal + page.outgoingTotal)
        require(page.total >= loaded.toLong() + page.relations.size)
        require(before != null || page.relations.isNotEmpty() || page.total == 0L)
        require(page.hasMore == (page.nextPosition != null))
        require(page.hasMore == (loaded.toLong() + page.relations.size < page.total))
        require(!page.hasMore || page.relations.isNotEmpty())
        require(page.nextPosition == null || page.nextPosition != before)
        require(page.relations.map { it.note.nodeKey }.distinct().size == page.relations.size)
    }

    private fun accept(restart: Boolean, page: DirectedRelationsResult) {
        if (restart) {
            phase =
                if (page.relations.isEmpty()) {
                    DirectedRelationsPhase.Empty
                } else {
                    page.ready()
                }
            return
        }
        val current = phase as? DirectedRelationsPhase.Ready ?: return
        require(page.total == current.total)
        require(page.incomingTotal == current.incomingTotal)
        require(page.outgoingTotal == current.outgoingTotal)
        val known = current.relations.mapTo(mutableSetOf()) { it.note.nodeKey }
        require(page.relations.none { !known.add(it.note.nodeKey) })
        val relations = current.relations + page.relations
        require(page.total >= relations.size)
        phase =
            current.copy(
                relations = relations,
                hasMore = page.hasMore,
                nextPosition = page.nextPosition,
                loadingMore = false,
            )
    }

    private fun DirectedRelationsResult.ready(): DirectedRelationsPhase.Ready =
        DirectedRelationsPhase.Ready(
            relations = relations,
            total = total,
            incomingTotal = incomingTotal,
            outgoingTotal = outgoingTotal,
            hasMore = hasMore,
            nextPosition = nextPosition,
        )

    private fun fail(restart: Boolean) {
        phase =
            if (restart) {
                source.getAndSet(null)?.close()
                DirectedRelationsPhase.Failed
            } else {
                val current = phase as? DirectedRelationsPhase.Ready
                current?.copy(loadingMore = false, continuationFailed = true)
                    ?: DirectedRelationsPhase.Failed
            }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        source.getAndSet(null)?.close()
    }

    private companion object {
        const val DEFAULT_PAGE_SIZE = 24
        const val WORKER_NAME = "slipbox-directed-relations"
    }
}

@Composable
internal fun rememberDirectedRelationsState(
    note: BoundNote?,
    ready: ReadySource,
): DirectedRelationsState? {
    val state =
        note?.let { bound ->
            remember(bound, ready.binding, ready.contentRoot, ready.database) {
                DirectedRelationsState(bound, ready)
            }
        }
    DisposableEffect(state) { onDispose { state?.close() } }
    return state
}
