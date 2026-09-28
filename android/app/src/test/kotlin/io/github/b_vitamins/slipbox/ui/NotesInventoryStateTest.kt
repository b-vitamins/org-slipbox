/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.ListNotesResult
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class NotesInventoryStateTest {

    @Test
    fun initialFailureCanReopenAndRestartFromTheFirstPage() {
        val opens = AtomicInteger()
        val state =
            state(
                factory =
                    NotePageSourceFactory {
                        if (opens.incrementAndGet() == 1) error("unavailable")
                        QueuePages(ListNotesResult(listOf(note("a")), 1, false, null))
                    },
            )
        await { state.phase == NotesInventoryPhase.Failed }

        state.retry()

        await { state.phase is NotesInventoryPhase.Ready }
        assertEquals(listOf("a"), (state.phase as NotesInventoryPhase.Ready).notes.map(NodeRecord::title))
        assertEquals(2, opens.get())
        state.close()
    }

    @Test
    fun continuationAppendsEveryPageUsingTheEngineCursor() {
        val pages =
            QueuePages(
                ListNotesResult(listOf(note("a"), note("b")), 3, true, "after-b"),
                ListNotesResult(listOf(note("c")), 3, false, null),
            )
        val state = state(factory = NotePageSourceFactory { pages })
        await { state.phase is NotesInventoryPhase.Ready }

        state.loadMore()

        await { (state.phase as? NotesInventoryPhase.Ready)?.notes?.size == 3 }
        val ready = state.phase as NotesInventoryPhase.Ready
        assertEquals(listOf("a", "b", "c"), ready.notes.map(NodeRecord::title))
        assertEquals(listOf(null, "after-b"), pages.after)
        assertEquals(3, ready.total)
        assertEquals(false, ready.hasMore)
        state.close()
    }

    @Test
    fun continuationFailureKeepsLoadedRowsAndRetriesTheSameCursor() {
        val attempts = AtomicInteger()
        val source =
            object : NotePageSource {
                override fun list(after: String?, limit: Int): ListNotesResult {
                    return when (attempts.incrementAndGet()) {
                        1 -> ListNotesResult(listOf(note("a")), 2, true, "after-a")
                        2 -> error("offline")
                        else -> ListNotesResult(listOf(note("b")), 2, false, null)
                    }
                }

                override fun close() = Unit
            }
        val state = state(factory = NotePageSourceFactory { source })
        await { state.phase is NotesInventoryPhase.Ready }

        state.loadMore()
        await { (state.phase as? NotesInventoryPhase.Ready)?.continuationFailed == true }
        assertEquals(listOf("a"), (state.phase as NotesInventoryPhase.Ready).notes.map(NodeRecord::title))

        state.retry()
        await { (state.phase as? NotesInventoryPhase.Ready)?.notes?.size == 2 }
        assertEquals(listOf("a", "b"), (state.phase as NotesInventoryPhase.Ready).notes.map(NodeRecord::title))
        state.close()
    }

    @Test
    fun closingAWithdrawnGenerationSuppressesItsLatePage() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val source =
            object : NotePageSource {
                override fun list(after: String?, limit: Int): ListNotesResult {
                    entered.countDown()
                    release.await(5, TimeUnit.SECONDS)
                    return ListNotesResult(listOf(note("old")), 1, false, null)
                }

                override fun close() = Unit
            }
        val old = state(factory = NotePageSourceFactory { source })
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        old.close()
        release.countDown()

        val current =
            state(
                ready = ready("generation-b"),
                factory =
                    NotePageSourceFactory {
                        QueuePages(ListNotesResult(listOf(note("new")), 1, false, null))
                    },
            )
        await { current.phase is NotesInventoryPhase.Ready }
        assertEquals("new", (current.phase as NotesInventoryPhase.Ready).notes.single().title)
        assertEquals(NotesInventoryPhase.Loading, old.phase)
        current.close()
    }

    @Test
    fun malformedDuplicateBoundaryIsARecoverableContinuationFailure() {
        val pages =
            QueuePages(
                ListNotesResult(listOf(note("a")), 2, true, "after-a"),
                ListNotesResult(listOf(note("a")), 2, false, null),
            )
        val state = state(factory = NotePageSourceFactory { pages })
        await { state.phase is NotesInventoryPhase.Ready }

        state.loadMore()

        await { (state.phase as? NotesInventoryPhase.Ready)?.continuationFailed == true }
        assertEquals(listOf("a"), (state.phase as NotesInventoryPhase.Ready).notes.map(NodeRecord::title))
        state.close()
    }

    private fun state(
        ready: ReadySource = ready("generation-a"),
        factory: NotePageSourceFactory,
    ): NotesInventoryState =
        NotesInventoryState(
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
            pageSize = 2,
        )

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private class QueuePages(vararg pages: ListNotesResult) : NotePageSource {

        private val pages = ArrayDeque(pages.toList())
        val after = mutableListOf<String?>()

        override fun list(after: String?, limit: Int): ListNotesResult {
            this.after += after
            return pages.removeFirst()
        }

        override fun close() = Unit
    }

    private fun ready(generation: String): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = SOURCE,
                    displayName = "Notes",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/notes.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(SOURCE, generation),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(3, 3, 0),
        )

    private fun note(name: String): NodeRecord =
        NodeRecord(
            nodeKey = "file:$name.org",
            explicitId = null,
            filePath = "$name.org",
            title = name,
            outlinePath = "",
            aliases = emptyList(),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = false,
            glossaryStatus = null,
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 0,
            line = 1,
            kind = NodeKind.FILE,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
    }
}
