/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.ui.text.input.TextFieldValue
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.GlossaryDueResult
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
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class GlossaryReviewStateTest {

    @Test
    fun engineReferenceDateIsPinnedAcrossPagingAndSearch() {
        val calls = mutableListOf<Call>()
        val source =
            object : GlossaryReviewSource {
                override fun due(
                    referenceDate: String?,
                    query: String?,
                    after: String?,
                    limit: Int,
                ): GlossaryDueResult {
                    synchronized(calls) { calls += Call(referenceDate, query, after) }
                    return when {
                        query == "beta" -> page(listOf(term("beta")), 1, false, null)
                        after == null -> page(listOf(term("alpha")), 2, true, "after-alpha")
                        else -> page(listOf(term("beta")), 2, false, null)
                    }
                }

                override fun close() = Unit
            }
        val state = state(GlossaryReviewSourceFactory { source })

        state.activate("")
        await { state.phase is GlossaryReviewPhase.Ready }
        state.loadMore()
        await { (state.phase as? GlossaryReviewPhase.Ready)?.terms?.size == 2 }
        state.update(TextFieldValue("beta"))
        await { (state.phase as? GlossaryReviewPhase.Ready)?.query == "beta" }
        state.activate("beta")
        await {
            synchronized(calls) { calls.size } == 4 && state.phase is GlossaryReviewPhase.Ready
        }

        assertEquals(
            listOf(
                Call(null, null, null),
                Call(DATE, null, "after-alpha"),
                Call(DATE, "beta", null),
                Call(null, "beta", null),
            ),
            synchronized(calls) { calls.toList() },
        )
        val ready = state.phase as GlossaryReviewPhase.Ready
        assertEquals(DATE, ready.referenceDate)
        assertEquals(listOf("beta"), ready.terms.map(NodeRecord::title))
        state.close()
    }

    @Test
    fun staleUnfilteredAnswerCannotReplaceANewerFilter() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val returned = CountDownLatch(1)
        val source =
            object : GlossaryReviewSource {
                override fun due(
                    referenceDate: String?,
                    query: String?,
                    after: String?,
                    limit: Int,
                ): GlossaryDueResult {
                    if (query == null) {
                        entered.countDown()
                        assertTrue(release.await(5, TimeUnit.SECONDS))
                        returned.countDown()
                        return page(listOf(term("old")), 1, false, null)
                    }
                    return page(listOf(term("new")), 1, false, null)
                }

                override fun close() = Unit
            }
        val state = state(GlossaryReviewSourceFactory { source })

        state.activate("")
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        state.update(TextFieldValue("new"))
        await { (state.phase as? GlossaryReviewPhase.Ready)?.query == "new" }
        release.countDown()
        assertTrue(returned.await(5, TimeUnit.SECONDS))

        val ready = state.phase as GlossaryReviewPhase.Ready
        assertEquals("new", ready.query)
        assertEquals(listOf("new"), ready.terms.map(NodeRecord::title))
        state.close()
    }

    private fun state(factory: GlossaryReviewSourceFactory): GlossaryReviewState =
        GlossaryReviewState(
            ready = ready(),
            factory = factory,
            delivery = ImportDelivery { it() },
            pageSize = 2,
            debounceMillis = 0,
        )

    private fun page(
        terms: List<NodeRecord>,
        total: Long,
        hasMore: Boolean,
        nextPosition: String?,
    ): GlossaryDueResult = GlossaryDueResult(DATE, terms, total, hasMore, nextPosition)

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
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
            aliases = emptyList(),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = true,
            glossaryStatus = "confirmed",
            srDue = DATE,
            srEase = "2.50",
            srInterval = "6",
            srReps = "2",
            srLast = "2026-09-27",
            level = 2,
            line = 1,
            kind = NodeKind.HEADING,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private data class Call(
        val referenceDate: String?,
        val query: String?,
        val after: String?,
    )

    private companion object {
        const val DATE = "2026-10-03"
        const val SOURCE = "0123456789abcdef0123456789abcdef"
    }
}
