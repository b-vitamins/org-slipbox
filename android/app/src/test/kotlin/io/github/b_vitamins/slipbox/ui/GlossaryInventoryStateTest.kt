/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.ListGlossaryTermsResult
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
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

class GlossaryInventoryStateTest {

    @Test
    fun inventoryIsLazyAndContinuesThroughEveryCursorPage() {
        val pages =
            QueuePages(
                ListGlossaryTermsResult(listOf(term("a"), term("b")), 3, true, "after-b"),
                ListGlossaryTermsResult(listOf(term("c")), 3, false, null),
            )
        val opens = AtomicInteger()
        val state =
            state(
                factory = GlossaryPageSourceFactory {
                    opens.incrementAndGet()
                    pages
                },
            )

        assertEquals(GlossaryInventoryPhase.Dormant, state.phase)
        assertEquals(0, opens.get())
        state.activate()
        await { state.phase is GlossaryInventoryPhase.Ready }
        state.loadMore()
        await { (state.phase as? GlossaryInventoryPhase.Ready)?.terms?.size == 3 }

        val ready = state.phase as GlossaryInventoryPhase.Ready
        assertEquals(listOf("a", "b", "c"), ready.terms.map(NodeRecord::title))
        assertEquals(listOf(null, "after-b"), pages.after)
        assertEquals(1, opens.get())
        assertEquals(false, ready.hasMore)
        state.close()
    }

    @Test
    fun duplicatePageBoundaryCannotCorruptTheVisibleInventory() {
        val pages =
            QueuePages(
                ListGlossaryTermsResult(listOf(term("a")), 2, true, "after-a"),
                ListGlossaryTermsResult(listOf(term("a")), 2, false, null),
            )
        val state = state(factory = GlossaryPageSourceFactory { pages })
        state.activate()
        await { state.phase is GlossaryInventoryPhase.Ready }
        state.loadMore()
        await { (state.phase as? GlossaryInventoryPhase.Ready)?.continuationFailed == true }

        val ready = state.phase as GlossaryInventoryPhase.Ready
        assertEquals(listOf("a"), ready.terms.map(NodeRecord::title))
        assertTrue(ready.continuationFailed)
        state.close()
    }

    @Test
    fun failedContinuationRetriesTheSameOpaqueCursor() {
        val attempts = AtomicInteger()
        val source =
            object : GlossaryPageSource {
                val after = mutableListOf<String?>()

                override fun list(after: String?, limit: Int): ListGlossaryTermsResult {
                    this.after += after
                    return when (attempts.incrementAndGet()) {
                        1 -> ListGlossaryTermsResult(listOf(term("a")), 2, true, "opaque")
                        2 -> error("offline")
                        else -> ListGlossaryTermsResult(listOf(term("b")), 2, false, null)
                    }
                }

                override fun close() = Unit
            }
        val state = state(factory = GlossaryPageSourceFactory { source })
        state.activate()
        await { state.phase is GlossaryInventoryPhase.Ready }
        state.loadMore()
        await { (state.phase as? GlossaryInventoryPhase.Ready)?.continuationFailed == true }
        state.retry()
        await { (state.phase as? GlossaryInventoryPhase.Ready)?.terms?.size == 2 }

        assertEquals(listOf(null, "opaque", "opaque"), source.after)
        state.close()
    }

    private fun state(factory: GlossaryPageSourceFactory): GlossaryInventoryState =
        GlossaryInventoryState(
            ready = ready(),
            factory = factory,
            delivery = ImportDelivery { it() },
            pageSize = 2,
        )

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private class QueuePages(vararg pages: ListGlossaryTermsResult) : GlossaryPageSource {
        private val pages = ArrayDeque(pages.toList())
        val after = mutableListOf<String?>()

        override fun list(after: String?, limit: Int): ListGlossaryTermsResult {
            this.after += after
            return pages.removeFirst()
        }

        override fun close() = Unit
    }

    private fun ready(): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = SOURCE,
                    displayName = "owner/glossary",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/glossary.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(SOURCE, "generation-a"),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(3, 3, 0),
        )

    private fun term(name: String): NodeRecord =
        NodeRecord(
            nodeKey = "heading:terms.org:${name.codePointAt(0)}",
            explicitId = "id-$name",
            filePath = "terms.org",
            title = name,
            outlinePath = "Glossary/$name",
            aliases = listOf("alias-$name"),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = true,
            glossaryStatus = "confirmed",
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 2,
            line = 1,
            kind = NodeKind.HEADING,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
    }
}
