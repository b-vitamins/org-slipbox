/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.DirectedRelationDirection
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.DirectedRelationsResult
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DirectedRelationsStateTest {

    @Test
    fun activationIsLazyAndContinuationPreservesCanonicalOrderAndCounts() {
        val opens = AtomicInteger()
        val pages =
            QueuePages(
                result(
                    relation("a", DirectedRelationDirection.INCOMING),
                    relation("b", DirectedRelationDirection.BIDIRECTIONAL),
                    total = 3,
                    incoming = 2,
                    outgoing = 2,
                    next = "after-b",
                ),
                result(
                    relation("c", DirectedRelationDirection.OUTGOING),
                    total = 3,
                    incoming = 2,
                    outgoing = 2,
                ),
            )
        val state =
            state(factory = DirectedRelationPageSourceFactory { opens.incrementAndGet(); pages })

        assertEquals(DirectedRelationsPhase.Idle, state.phase)
        assertEquals(0, opens.get())
        state.activate()
        await { state.phase is DirectedRelationsPhase.Ready }
        state.loadMore()
        await { (state.phase as? DirectedRelationsPhase.Ready)?.relations?.size == 3 }

        val ready = state.phase as DirectedRelationsPhase.Ready
        assertEquals(listOf("a", "b", "c"), ready.relations.map { it.note.title })
        assertEquals(listOf(null, "after-b"), pages.after)
        assertEquals(listOf(NODE_KEY, NODE_KEY), pages.nodeKeys)
        assertEquals(3, ready.total)
        assertEquals(2, ready.incomingTotal)
        assertEquals(2, ready.outgoingTotal)
        assertEquals(false, ready.hasMore)
        assertEquals(1, opens.get())
        state.close()
    }

    @Test
    fun continuationFailureKeepsRowsAndRetriesTheSameCursor() {
        val attempts = AtomicInteger()
        val asked = mutableListOf<String?>()
        val source =
            object : DirectedRelationPageSource {
                override fun list(
                    nodeKey: String,
                    after: String?,
                    limit: Int,
                ): DirectedRelationsResult {
                    asked += after
                    return when (attempts.incrementAndGet()) {
                        1 ->
                            result(
                                relation("a", DirectedRelationDirection.INCOMING),
                                total = 2,
                                incoming = 1,
                                outgoing = 1,
                                next = "after-a",
                            )
                        2 -> error("offline")
                        else ->
                            result(
                                relation("b", DirectedRelationDirection.OUTGOING),
                                total = 2,
                                incoming = 1,
                                outgoing = 1,
                            )
                    }
                }

                override fun close() = Unit
            }
        val state = state(factory = DirectedRelationPageSourceFactory { source })
        state.activate()
        await { state.phase is DirectedRelationsPhase.Ready }

        state.loadMore()
        await { (state.phase as? DirectedRelationsPhase.Ready)?.continuationFailed == true }
        assertEquals(listOf("a"), ready(state).relations.map { it.note.title })

        state.retry()
        await { (state.phase as? DirectedRelationsPhase.Ready)?.relations?.size == 2 }
        assertEquals(listOf(null, "after-a", "after-a"), asked)
        assertEquals(listOf("a", "b"), ready(state).relations.map { it.note.title })
        state.close()
    }

    @Test
    fun duplicateAcrossAPageBoundaryIsRejectedWithoutLosingLoadedRows() {
        val pages =
            QueuePages(
                result(
                    relation("a", DirectedRelationDirection.INCOMING),
                    total = 2,
                    incoming = 2,
                    outgoing = 0,
                    next = "after-a",
                ),
                result(
                    relation("a", DirectedRelationDirection.INCOMING),
                    total = 2,
                    incoming = 2,
                    outgoing = 0,
                ),
            )
        val state = state(factory = DirectedRelationPageSourceFactory { pages })
        state.activate()
        await { state.phase is DirectedRelationsPhase.Ready }

        state.loadMore()

        await { (state.phase as? DirectedRelationsPhase.Ready)?.continuationFailed == true }
        assertEquals(listOf("a"), ready(state).relations.map { it.note.title })
        state.close()
    }

    @Test
    fun anIncompletePageCannotClaimTheInventoryEnded() {
        val page =
            result(
                relation("a", DirectedRelationDirection.INCOMING),
                total = 2,
                incoming = 2,
                outgoing = 0,
            )
        val state = state(factory = DirectedRelationPageSourceFactory { QueuePages(page) })

        state.activate()

        await { state.phase == DirectedRelationsPhase.Failed }
        state.close()
    }

    private fun state(factory: DirectedRelationPageSourceFactory): DirectedRelationsState {
        val ready = ready()
        return DirectedRelationsState(
            note = BoundNote(ready.binding, NODE_KEY),
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
            pageSize = 2,
        )
    }

    private fun ready(state: DirectedRelationsState): DirectedRelationsPhase.Ready =
        state.phase as DirectedRelationsPhase.Ready

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private class QueuePages(vararg pages: DirectedRelationsResult) : DirectedRelationPageSource {

        private val pages = ArrayDeque(pages.toList())
        val after = mutableListOf<String?>()
        val nodeKeys = mutableListOf<String>()

        override fun list(
            nodeKey: String,
            after: String?,
            limit: Int,
        ): DirectedRelationsResult {
            nodeKeys += nodeKey
            this.after += after
            return pages.removeFirst()
        }

        override fun close() = Unit
    }

    private fun result(
        vararg relations: DirectedRelationRecord,
        total: Long,
        incoming: Long,
        outgoing: Long,
        next: String? = null,
    ): DirectedRelationsResult =
        DirectedRelationsResult(
            relations = relations.toList(),
            total = total,
            incomingTotal = incoming,
            outgoingTotal = outgoing,
            hasMore = next != null,
            nextPosition = next,
        )

    private fun relation(
        name: String,
        direction: DirectedRelationDirection,
    ): DirectedRelationRecord = DirectedRelationRecord(note(name), direction, "Context for $name")

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

    private fun ready(): ReadySource =
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
            binding = GenerationBinding(SOURCE, "generation-a"),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(3, 3, 0),
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val NODE_KEY = "file:focus.org"
    }
}
