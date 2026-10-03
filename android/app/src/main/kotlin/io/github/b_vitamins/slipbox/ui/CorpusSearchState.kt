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
import io.github.b_vitamins.slipbox.engine.CorpusSearchEntity
import io.github.b_vitamins.slipbox.engine.CorpusSearchField
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SearchCorpusResult
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface CorpusSearchPhase {

    data object Dormant : CorpusSearchPhase

    data class Searching(val query: String) : CorpusSearchPhase

    data class Empty(val query: String) : CorpusSearchPhase

    data class Ready(
        val query: String,
        val hits: List<CorpusSearchHit>,
        val total: Long,
        val queryBound: Long,
        val queryTruncated: Boolean,
        val hasMore: Boolean,
        val nextPosition: String?,
        val loadingMore: Boolean = false,
        val continuationFailed: Boolean = false,
    ) : CorpusSearchPhase

    data class Failed(val query: String) : CorpusSearchPhase
}

internal interface CorpusSearchSource : AutoCloseable {

    fun search(query: String, after: String?, limit: Int): SearchCorpusResult
}

internal fun interface CorpusSearchSourceFactory {

    fun open(ready: ReadySource): CorpusSearchSource
}

private object NativeCorpusSearchSourceFactory : CorpusSearchSourceFactory {

    override fun open(ready: ReadySource): CorpusSearchSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : CorpusSearchSource {
                override fun search(
                    query: String,
                    after: String?,
                    limit: Int,
                ): SearchCorpusResult {
                    val answer =
                        session.answer(ReadOperation.SearchCorpus(query, limit, after)).await()
                    return (answer as EngineAnswer.SearchCorpus).result
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

/** Debounced, source-generation-bound search whose ranking remains entirely in Rust. */
@Stable
internal class CorpusSearchState(
    private val ready: ReadySource,
    initialQuery: String = "",
    private val factory: CorpusSearchSourceFactory = NativeCorpusSearchSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
    private val debounceMillis: Long = DEFAULT_DEBOUNCE_MILLIS,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val requests = AtomicLong()
    private val source = AtomicReference<CorpusSearchSource?>()

    var input: TextFieldValue by
        mutableStateOf(TextFieldValue(initialQuery, TextRange(initialQuery.length)))
        private set

    var phase: CorpusSearchPhase by mutableStateOf(CorpusSearchPhase.Dormant)
        private set

    var selectedNodeKey: String? by mutableStateOf(null)
        private set

    init {
        require(pageSize > 0)
        require(debounceMillis >= 0)
        if (initialQuery.isNotBlank()) submit(initialQuery, wait = true)
    }

    fun restore(query: String) {
        if (query == input.text) return
        input = TextFieldValue(query, TextRange(query.length))
        selectedNodeKey = null
        submit(query, wait = true)
    }

    fun update(value: TextFieldValue) {
        val changed = value.text != input.text
        input = value
        if (!changed) return
        selectedNodeKey = null
        submit(value.text, wait = true)
    }

    fun select(hit: CorpusSearchHit) {
        selectedNodeKey = hit.node.nodeKey
    }

    fun loadMore() {
        val current = phase as? CorpusSearchPhase.Ready ?: return
        if (!current.hasMore || current.loadingMore || current.nextPosition == null) return
        phase = current.copy(loadingMore = true, continuationFailed = false)
        request(current.query, current.nextPosition, restart = false, wait = false)
    }

    fun retry() {
        when (val current = phase) {
            is CorpusSearchPhase.Failed -> submit(current.query, wait = false)
            is CorpusSearchPhase.Ready ->
                if (current.continuationFailed && current.nextPosition != null) {
                    phase = current.copy(loadingMore = true, continuationFailed = false)
                    request(current.query, current.nextPosition, restart = false, wait = false)
                }
            else -> Unit
        }
    }

    private fun submit(raw: String, wait: Boolean) {
        val query = raw.trim()
        if (query.isEmpty()) {
            requests.incrementAndGet()
            phase = CorpusSearchPhase.Dormant
            return
        }
        phase = CorpusSearchPhase.Searching(query)
        request(query, after = null, restart = true, wait = wait)
    }

    private fun request(query: String, after: String?, restart: Boolean, wait: Boolean) {
        if (!live.get()) return
        val serial = requests.incrementAndGet()
        Thread(
                {
                    if (wait && debounceMillis > 0) Thread.sleep(debounceMillis)
                    if (!live.get() || requests.get() != serial) return@Thread
                    val outcome = runCatching {
                        val searches = source.get() ?: openSource()
                        searches.search(query, after, pageSize).also {
                            validate(query, after, it)
                        }
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

    private fun openSource(): CorpusSearchSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the corpus search was closed while opening")
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
            throw IllegalStateException("the corpus search was closed while opening")
        }
        return retained
    }

    private fun validate(query: String, before: String?, page: SearchCorpusResult) {
        require(query.isNotBlank())
        require(page.hits.size <= pageSize)
        require(page.total >= page.hits.size)
        require(page.queryBound > 0)
        require(page.total <= page.queryBound)
        require(!page.queryTruncated || page.total == page.queryBound)
        require(page.hasMore == (page.nextPosition != null))
        require(!page.hasMore || page.hits.isNotEmpty())
        require(page.nextPosition == null || page.nextPosition != before)
        require(page.hits.map { it.node.nodeKey }.distinct().size == page.hits.size)
        page.hits.forEach(::validateHit)
    }

    private fun validateHit(hit: CorpusSearchHit) {
        require((hit.entity == CorpusSearchEntity.GLOSSARY) == hit.node.glossary)
        require(hit.title.segments.joinToString("") { it.text } == hit.node.title)
        require(hit.aliases.segments.joinToString("") { it.text } == hit.node.aliases.joinToString(" "))
        val title = hit.title.segments.any { it.matched }
        val alias = hit.aliases.segments.any { it.matched }
        val content = hit.excerpt.segments.any { it.matched }
        require(title || alias || content)
        require(
            when (hit.matchedField) {
                CorpusSearchField.TITLE -> title
                CorpusSearchField.ALIAS -> !title && alias
                CorpusSearchField.CONTENT -> !title && !alias && content
            },
        )
    }

    private fun accept(query: String, restart: Boolean, page: SearchCorpusResult) {
        if (restart) {
            phase =
                if (page.hits.isEmpty()) {
                    CorpusSearchPhase.Empty(query)
                } else {
                    CorpusSearchPhase.Ready(
                        query = query,
                        hits = page.hits,
                        total = page.total,
                        queryBound = page.queryBound,
                        queryTruncated = page.queryTruncated,
                        hasMore = page.hasMore,
                        nextPosition = page.nextPosition,
                    )
                }
            return
        }

        val current = phase as? CorpusSearchPhase.Ready ?: return
        require(current.query == query)
        require(page.total == current.total)
        require(page.queryBound == current.queryBound)
        require(page.queryTruncated == current.queryTruncated)
        val known = current.hits.mapTo(mutableSetOf()) { it.node.nodeKey }
        require(page.hits.none { !known.add(it.node.nodeKey) })
        val hits = current.hits + page.hits
        require(page.total >= hits.size)
        phase =
            current.copy(
                hits = hits,
                hasMore = page.hasMore,
                nextPosition = page.nextPosition,
                loadingMore = false,
            )
    }

    private fun fail(query: String, restart: Boolean) {
        phase =
            if (restart) {
                closeSource()
                CorpusSearchPhase.Failed(query)
            } else {
                val current = phase as? CorpusSearchPhase.Ready
                current?.copy(loadingMore = false, continuationFailed = true)
                    ?: CorpusSearchPhase.Failed(query)
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
        const val DEFAULT_PAGE_SIZE = 30
        const val DEFAULT_DEBOUNCE_MILLIS = 275L
        const val WORKER_NAME = "slipbox-corpus-search"
    }
}

@Composable
internal fun rememberCorpusSearchState(ready: ReadySource): CorpusSearchState {
    val state =
        remember(ready.binding, ready.contentRoot, ready.database) {
            CorpusSearchState(ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
