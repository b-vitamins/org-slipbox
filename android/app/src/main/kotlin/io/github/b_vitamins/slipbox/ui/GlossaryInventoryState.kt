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
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.ListGlossaryTermsResult
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface GlossaryInventoryPhase {

    data object Dormant : GlossaryInventoryPhase

    data object Loading : GlossaryInventoryPhase

    data object Empty : GlossaryInventoryPhase

    data class Ready(
        val terms: List<NodeRecord>,
        val total: Long,
        val hasMore: Boolean,
        val nextPosition: String?,
        val loadingMore: Boolean = false,
        val continuationFailed: Boolean = false,
    ) : GlossaryInventoryPhase

    data object Failed : GlossaryInventoryPhase
}

internal interface GlossaryPageSource : AutoCloseable {

    fun list(after: String?, limit: Int): ListGlossaryTermsResult
}

internal fun interface GlossaryPageSourceFactory {

    fun open(ready: ReadySource): GlossaryPageSource
}

private object NativeGlossaryPageSourceFactory : GlossaryPageSourceFactory {

    override fun open(ready: ReadySource): GlossaryPageSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : GlossaryPageSource {
                override fun list(after: String?, limit: Int): ListGlossaryTermsResult {
                    val answer =
                        session.answer(ReadOperation.ListGlossaryTerms(limit, after)).await()
                    return (answer as EngineAnswer.ListGlossaryTerms).result
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

/** Owns the lazy, bounded glossary inventory for one ready source generation. */
@Stable
internal class GlossaryInventoryState(
    private val ready: ReadySource,
    private val factory: GlossaryPageSourceFactory = NativeGlossaryPageSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val loading = AtomicBoolean()
    private val requests = AtomicLong()
    private val source = AtomicReference<GlossaryPageSource?>()

    var phase: GlossaryInventoryPhase by mutableStateOf(GlossaryInventoryPhase.Dormant)
        private set

    init {
        require(pageSize > 0)
    }

    fun activate() {
        if (phase != GlossaryInventoryPhase.Dormant || !live.get()) return
        phase = GlossaryInventoryPhase.Loading
        request(restart = true)
    }

    fun loadMore() {
        val current = phase as? GlossaryInventoryPhase.Ready ?: return
        if (!current.hasMore || current.loadingMore || current.nextPosition == null) return
        phase = current.copy(loadingMore = true, continuationFailed = false)
        request(restart = false)
    }

    fun retry() {
        when (val current = phase) {
            GlossaryInventoryPhase.Failed -> {
                phase = GlossaryInventoryPhase.Loading
                request(restart = true)
            }
            is GlossaryInventoryPhase.Ready ->
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
        val before = if (restart) null else (phase as GlossaryInventoryPhase.Ready).nextPosition
        Thread(
                {
                    val outcome = runCatching {
                        val pages = source.get() ?: openSource()
                        pages.list(before, pageSize).also { validate(before, it) }
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

    private fun openSource(): GlossaryPageSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the glossary inventory was closed while opening")
        }
        if (!source.compareAndSet(null, opened)) opened.close()
        return checkNotNull(source.get())
    }

    private fun validate(before: String?, page: ListGlossaryTermsResult) {
        require(page.terms.size <= pageSize)
        require(page.total >= 0)
        require(page.total >= page.terms.size)
        require(before != null || page.terms.isNotEmpty() || page.total == 0L)
        require(page.hasMore == (page.nextPosition != null))
        require(!page.hasMore || page.terms.isNotEmpty())
        require(page.nextPosition == null || page.nextPosition != before)
        require(page.terms.all(NodeRecord::glossary))
        require(page.terms.map(NodeRecord::nodeKey).distinct().size == page.terms.size)
    }

    private fun accept(restart: Boolean, page: ListGlossaryTermsResult) {
        if (restart) {
            phase =
                if (page.terms.isEmpty()) {
                    GlossaryInventoryPhase.Empty
                } else {
                    GlossaryInventoryPhase.Ready(
                        terms = page.terms,
                        total = page.total,
                        hasMore = page.hasMore,
                        nextPosition = page.nextPosition,
                    )
                }
            return
        }

        val current = phase as? GlossaryInventoryPhase.Ready ?: return
        require(page.total == current.total)
        val known = current.terms.mapTo(mutableSetOf(), NodeRecord::nodeKey)
        require(page.terms.none { !known.add(it.nodeKey) })
        val terms = current.terms + page.terms
        require(page.total >= terms.size)
        phase =
            current.copy(
                terms = terms,
                hasMore = page.hasMore,
                nextPosition = page.nextPosition,
                loadingMore = false,
            )
    }

    private fun fail(restart: Boolean) {
        phase =
            if (restart) {
                source.getAndSet(null)?.close()
                GlossaryInventoryPhase.Failed
            } else {
                val current = phase as? GlossaryInventoryPhase.Ready
                current?.copy(loadingMore = false, continuationFailed = true)
                    ?: GlossaryInventoryPhase.Failed
            }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        source.getAndSet(null)?.close()
    }

    private companion object {
        const val DEFAULT_PAGE_SIZE = 50
        const val WORKER_NAME = "slipbox-glossary-inventory"
    }
}

@Composable
internal fun rememberGlossaryInventoryState(ready: ReadySource): GlossaryInventoryState {
    val state =
        remember(ready.binding, ready.contentRoot, ready.database) {
            GlossaryInventoryState(ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
