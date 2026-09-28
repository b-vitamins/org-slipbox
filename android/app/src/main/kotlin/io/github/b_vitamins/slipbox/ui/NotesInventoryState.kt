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
import io.github.b_vitamins.slipbox.engine.ListNotesResult
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface NotesInventoryPhase {

    data object Loading : NotesInventoryPhase

    data object Empty : NotesInventoryPhase

    data class Ready(
        val notes: List<NodeRecord>,
        val total: Long,
        val hasMore: Boolean,
        val nextPosition: String?,
        val loadingMore: Boolean = false,
        val continuationFailed: Boolean = false,
    ) : NotesInventoryPhase

    data object Failed : NotesInventoryPhase
}

internal interface NotePageSource : AutoCloseable {

    fun list(after: String?, limit: Int): ListNotesResult
}

internal fun interface NotePageSourceFactory {

    fun open(ready: ReadySource): NotePageSource
}

private object NativeNotePageSourceFactory : NotePageSourceFactory {

    override fun open(ready: ReadySource): NotePageSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : NotePageSource {
                override fun list(after: String?, limit: Int): ListNotesResult {
                    val answer = session.answer(ReadOperation.ListNotes(limit, after)).await()
                    return (answer as EngineAnswer.ListNotes).result
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

/** Owns one serial page stream for exactly one ready source generation. */
@Stable
internal class NotesInventoryState(
    private val ready: ReadySource,
    private val factory: NotePageSourceFactory = NativeNotePageSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val loading = AtomicBoolean()
    private val requests = AtomicLong()
    private val source = AtomicReference<NotePageSource?>()

    var phase: NotesInventoryPhase by mutableStateOf(NotesInventoryPhase.Loading)
        private set

    init {
        require(pageSize > 0)
        request(restart = true)
    }

    fun loadMore() {
        val current = phase as? NotesInventoryPhase.Ready ?: return
        if (!current.hasMore || current.loadingMore || current.nextPosition == null) return
        phase = current.copy(loadingMore = true, continuationFailed = false)
        request(restart = false)
    }

    fun retry() {
        when (val current = phase) {
            NotesInventoryPhase.Failed -> {
                phase = NotesInventoryPhase.Loading
                request(restart = true)
            }
            is NotesInventoryPhase.Ready ->
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
        val before = if (restart) null else (phase as NotesInventoryPhase.Ready).nextPosition
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

    private fun openSource(): NotePageSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the note inventory was closed while opening")
        }
        if (!source.compareAndSet(null, opened)) {
            opened.close()
        }
        return checkNotNull(source.get())
    }

    private fun validate(before: String?, page: ListNotesResult) {
        require(page.notes.size <= pageSize)
        require(page.total >= 0)
        require(page.total >= page.notes.size)
        require(before != null || page.notes.isNotEmpty() || page.total == 0L)
        require(page.hasMore == (page.nextPosition != null))
        require(!page.hasMore || page.notes.isNotEmpty())
        require(page.nextPosition == null || page.nextPosition != before)
        require(page.notes.map(NodeRecord::nodeKey).distinct().size == page.notes.size)
    }

    private fun accept(restart: Boolean, page: ListNotesResult) {
        if (restart) {
            phase =
                if (page.notes.isEmpty()) {
                    NotesInventoryPhase.Empty
                } else {
                    NotesInventoryPhase.Ready(
                        notes = page.notes,
                        total = page.total,
                        hasMore = page.hasMore,
                        nextPosition = page.nextPosition,
                    )
                }
            return
        }

        val current = phase as? NotesInventoryPhase.Ready ?: return
        require(page.total == current.total)
        val known = current.notes.mapTo(mutableSetOf(), NodeRecord::nodeKey)
        require(page.notes.none { !known.add(it.nodeKey) })
        val notes = current.notes + page.notes
        require(page.total >= notes.size)
        phase =
            current.copy(
                notes = notes,
                hasMore = page.hasMore,
                nextPosition = page.nextPosition,
                loadingMore = false,
            )
    }

    private fun fail(restart: Boolean) {
        phase =
            if (restart) {
                source.getAndSet(null)?.close()
                NotesInventoryPhase.Failed
            } else {
                val current = phase as? NotesInventoryPhase.Ready
                current?.copy(loadingMore = false, continuationFailed = true)
                    ?: NotesInventoryPhase.Failed
            }
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        source.getAndSet(null)?.close()
    }

    private companion object {
        const val DEFAULT_PAGE_SIZE = 50
        const val WORKER_NAME = "slipbox-note-inventory"
    }
}

@Composable
internal fun rememberNotesInventoryState(ready: ReadySource): NotesInventoryState {
    val state =
        remember(ready.binding, ready.contentRoot, ready.database) {
            NotesInventoryState(ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
