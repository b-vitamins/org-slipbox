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
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.GlossaryDueResult
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface GlossaryReviewPhase {

    data object Dormant : GlossaryReviewPhase

    data class Loading(val query: String) : GlossaryReviewPhase

    data class Empty(
        val query: String,
        val referenceDate: String,
    ) : GlossaryReviewPhase

    data class Ready(
        val query: String,
        val referenceDate: String,
        val terms: List<NodeRecord>,
        val total: Long,
        val hasMore: Boolean,
        val nextPosition: String?,
        val loadingMore: Boolean = false,
        val continuationFailed: Boolean = false,
    ) : GlossaryReviewPhase

    data class Failed(val query: String) : GlossaryReviewPhase
}

internal interface GlossaryReviewSource : AutoCloseable {

    fun due(
        referenceDate: String?,
        query: String?,
        after: String?,
        limit: Int,
    ): GlossaryDueResult
}

internal fun interface GlossaryReviewSourceFactory {

    fun open(ready: ReadySource): GlossaryReviewSource
}

private object NativeGlossaryReviewSourceFactory : GlossaryReviewSourceFactory {

    override fun open(ready: ReadySource): GlossaryReviewSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : GlossaryReviewSource {
                override fun due(
                    referenceDate: String?,
                    query: String?,
                    after: String?,
                    limit: Int,
                ): GlossaryDueResult {
                    val operation = ReadOperation.GlossaryDue(referenceDate, query, limit, after)
                    return (session.answer(operation).await() as EngineAnswer.GlossaryDue).result
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

/** One source-generation-bound, read-only view of the canonical due queue. */
@Stable
internal class GlossaryReviewState(
    private val ready: ReadySource,
    initialQuery: String = "",
    private val factory: GlossaryReviewSourceFactory = NativeGlossaryReviewSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
    private val debounceMillis: Long = DEFAULT_DEBOUNCE_MILLIS,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val requests = AtomicLong()
    private val source = AtomicReference<GlossaryReviewSource?>()
    private var referenceDate: String? = null

    var input: TextFieldValue by
        mutableStateOf(TextFieldValue(initialQuery, TextRange(initialQuery.length)))
        private set

    var phase: GlossaryReviewPhase by mutableStateOf(GlossaryReviewPhase.Dormant)
        private set

    init {
        require(pageSize > 0)
        require(debounceMillis >= 0)
    }

    fun activate(query: String) {
        if (!live.get()) return
        if (input.text != query) input = TextFieldValue(query, TextRange(query.length))
        referenceDate = null
        submit(query.trim(), wait = false)
    }

    fun update(value: TextFieldValue) {
        val changed = value.text != input.text
        input = value
        if (changed) submit(value.text.trim(), wait = true)
    }

    fun loadMore() {
        val current = phase as? GlossaryReviewPhase.Ready ?: return
        if (!current.hasMore || current.loadingMore || current.nextPosition == null) return
        phase = current.copy(loadingMore = true, continuationFailed = false)
        request(current.query, current.nextPosition, restart = false, wait = false)
    }

    fun retry() {
        when (val current = phase) {
            is GlossaryReviewPhase.Failed -> submit(current.query, wait = false)
            is GlossaryReviewPhase.Ready ->
                if (current.continuationFailed && current.nextPosition != null) {
                    phase = current.copy(loadingMore = true, continuationFailed = false)
                    request(current.query, current.nextPosition, restart = false, wait = false)
                }
            else -> Unit
        }
    }

    private fun submit(query: String, wait: Boolean) {
        phase = GlossaryReviewPhase.Loading(query)
        request(query, after = null, restart = true, wait = wait)
    }

    private fun request(query: String, after: String?, restart: Boolean, wait: Boolean) {
        if (!live.get()) return
        val serial = requests.incrementAndGet()
        val asOf = referenceDate
        Thread(
                {
                    if (wait && debounceMillis > 0) Thread.sleep(debounceMillis)
                    if (!live.get() || requests.get() != serial) return@Thread
                    val outcome = runCatching {
                        val pages = source.get() ?: openSource()
                        pages
                            .due(asOf, query.ifBlank { null }, after, pageSize)
                            .also { validate(asOf, after, it) }
                    }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        outcome.fold(
                            onSuccess = { page ->
                                runCatching { accept(query, restart, page) }
                                    .onFailure { fail(query, restart) }
                            },
                            onFailure = { fail(query, restart) },
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

    private fun openSource(): GlossaryReviewSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the glossary review view was closed while opening")
        }
        val retained =
            if (source.compareAndSet(null, opened)) {
                opened
            } else {
                opened.close()
                checkNotNull(source.get())
            }
        if (!live.get()) {
            if (source.compareAndSet(retained, null)) retained.close()
            throw IllegalStateException("the glossary review view was closed while opening")
        }
        return retained
    }

    private fun validate(asOf: String?, before: String?, page: GlossaryDueResult) {
        require(DATE.matches(page.referenceDate))
        require(asOf == null || page.referenceDate == asOf)
        require(page.terms.size <= pageSize)
        require(page.total >= page.terms.size)
        require(page.hasMore == (page.nextPosition != null))
        require(!page.hasMore || page.terms.isNotEmpty())
        require(page.nextPosition == null || page.nextPosition != before)
        require(page.terms.all(NodeRecord::glossary))
        require(page.terms.map(NodeRecord::nodeKey).distinct().size == page.terms.size)
    }

    private fun accept(query: String, restart: Boolean, page: GlossaryDueResult) {
        val pinned = referenceDate
        require(pinned == null || pinned == page.referenceDate)
        referenceDate = page.referenceDate
        if (restart) {
            phase =
                if (page.terms.isEmpty()) {
                    GlossaryReviewPhase.Empty(query, page.referenceDate)
                } else {
                    GlossaryReviewPhase.Ready(
                        query = query,
                        referenceDate = page.referenceDate,
                        terms = page.terms,
                        total = page.total,
                        hasMore = page.hasMore,
                        nextPosition = page.nextPosition,
                    )
                }
            return
        }

        val current = phase as? GlossaryReviewPhase.Ready ?: return
        require(current.query == query)
        require(current.referenceDate == page.referenceDate)
        require(current.total == page.total)
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

    private fun fail(query: String, restart: Boolean) {
        phase =
            if (restart) {
                closeSource()
                GlossaryReviewPhase.Failed(query)
            } else {
                val current = phase as? GlossaryReviewPhase.Ready
                current?.copy(loadingMore = false, continuationFailed = true)
                    ?: GlossaryReviewPhase.Failed(query)
            }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        closeSource()
    }

    private fun closeSource() {
        source.getAndSet(null)?.close()
    }

    private companion object {
        val DATE = Regex("\\d{4}-\\d{2}-\\d{2}")
        const val DEFAULT_PAGE_SIZE = 50
        const val DEFAULT_DEBOUNCE_MILLIS = 275L
        const val WORKER_NAME = "slipbox-glossary-review"
    }
}

@Composable
internal fun rememberGlossaryReviewState(ready: ReadySource): GlossaryReviewState {
    val state =
        remember(ready.binding, ready.contentRoot, ready.database) {
            GlossaryReviewState(ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
