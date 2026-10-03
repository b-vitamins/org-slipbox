/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.ContentSnippet
import io.github.b_vitamins.slipbox.engine.CorpusSearchEntity
import io.github.b_vitamins.slipbox.engine.CorpusSearchField
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.SearchCorpusResult
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

class CorpusSearchStateTest {

    @Test
    fun aLateOlderQueryCannotReplaceTheLatestAnswer() {
        val oldStarted = CountDownLatch(1)
        val releaseOld = CountDownLatch(1)
        val source =
            object : CorpusSearchSource {
                override fun search(query: String, after: String?, limit: Int): SearchCorpusResult {
                    if (query == "old") {
                        oldStarted.countDown()
                        assertTrue(releaseOld.await(5, TimeUnit.SECONDS))
                    }
                    return page(hit(query))
                }

                override fun close() = Unit
            }
        val state = state(factory = CorpusSearchSourceFactory { source })

        state.update(TextFieldValue("old"))
        assertTrue(oldStarted.await(5, TimeUnit.SECONDS))
        state.update(TextFieldValue("new", TextRange(1, 2)))
        await { (state.phase as? CorpusSearchPhase.Ready)?.query == "new" }
        releaseOld.countDown()
        await { (state.phase as? CorpusSearchPhase.Ready)?.query == "new" }

        val ready = state.phase as CorpusSearchPhase.Ready
        assertEquals(listOf("new"), ready.hits.map { it.node.title })
        assertEquals(TextRange(1, 2), state.input.selection)
        state.close()
    }

    @Test
    fun pagingRetainsSelectionAndUsesTheOpaqueContinuation() {
        val calls = mutableListOf<String?>()
        val source =
            object : CorpusSearchSource {
                override fun search(query: String, after: String?, limit: Int): SearchCorpusResult {
                    calls += after
                    return if (after == null) {
                        SearchCorpusResult(
                            hits = listOf(hit("alpha"), hit("beta", glossary = true)),
                            total = 3,
                            hasMore = true,
                            nextPosition = "opaque",
                            queryBound = 200,
                            queryTruncated = false,
                        )
                    } else {
                        SearchCorpusResult(
                            hits = listOf(hit("gamma")),
                            total = 3,
                            hasMore = false,
                            nextPosition = null,
                            queryBound = 200,
                            queryTruncated = false,
                        )
                    }
                }

                override fun close() = Unit
            }
        val state = state(factory = CorpusSearchSourceFactory { source })
        state.update(TextFieldValue("shared"))
        await { state.phase is CorpusSearchPhase.Ready }
        state.select((state.phase as CorpusSearchPhase.Ready).hits[1])
        state.loadMore()
        await { (state.phase as? CorpusSearchPhase.Ready)?.hits?.size == 3 }

        assertEquals(listOf(null, "opaque"), calls)
        assertEquals("key-beta", state.selectedNodeKey)
        assertEquals(listOf("alpha", "beta", "gamma"), (state.phase as CorpusSearchPhase.Ready).hits.map { it.node.title })
        state.close()
    }

    @Test
    fun closingOneGenerationWithdrawsItsLateAnswerFromTheNextSource() {
        val started = CountDownLatch(1)
        val release = CountDownLatch(1)
        val old =
            state(
                ready = ready(SOURCE_A, "generation-a"),
                factory =
                    CorpusSearchSourceFactory {
                        object : CorpusSearchSource {
                            override fun search(
                                query: String,
                                after: String?,
                                limit: Int,
                            ): SearchCorpusResult {
                                started.countDown()
                                assertTrue(release.await(5, TimeUnit.SECONDS))
                                return page(hit("old"))
                            }

                            override fun close() = Unit
                        }
                    },
            )
        old.update(TextFieldValue("shared"))
        assertTrue(started.await(5, TimeUnit.SECONDS))
        old.close()

        val fresh =
            state(
                ready = ready(SOURCE_B, "generation-b"),
                factory = CorpusSearchSourceFactory { FixedSource(page(hit("fresh"))) },
            )
        fresh.update(TextFieldValue("shared"))
        await { fresh.phase is CorpusSearchPhase.Ready }
        release.countDown()
        await { (fresh.phase as? CorpusSearchPhase.Ready)?.hits?.single()?.node?.title == "fresh" }

        assertEquals("fresh", (fresh.phase as CorpusSearchPhase.Ready).hits.single().node.title)
        fresh.close()
    }

    private fun state(
        ready: ReadySource = ready(SOURCE_A, "generation-a"),
        factory: CorpusSearchSourceFactory,
    ): CorpusSearchState =
        CorpusSearchState(
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
            pageSize = 2,
            debounceMillis = 0,
        )

    private fun page(hit: CorpusSearchHit): SearchCorpusResult =
        SearchCorpusResult(
            hits = listOf(hit),
            total = 1,
            hasMore = false,
            nextPosition = null,
            queryBound = 200,
            queryTruncated = false,
        )

    private fun hit(title: String, glossary: Boolean = false): CorpusSearchHit {
        val node =
            NodeRecord(
                nodeKey = "key-$title",
                explicitId = "id-$title",
                filePath = "$title.org",
                title = title,
                outlinePath = title,
                aliases = emptyList(),
                tags = emptyList(),
                refs = emptyList(),
                todoKeyword = null,
                scheduledFor = null,
                deadlineFor = null,
                closedAt = null,
                glossary = glossary,
                glossaryStatus = if (glossary) "confirmed" else null,
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
        return CorpusSearchHit(
            node = node,
            entity = if (glossary) CorpusSearchEntity.GLOSSARY else CorpusSearchEntity.NOTE,
            matchedField = CorpusSearchField.TITLE,
            title = ContentSnippet(listOf(ContentSegment(title, true))),
            aliases = ContentSnippet(emptyList()),
            excerpt = ContentSnippet(emptyList()),
        )
    }

    private fun ready(source: String, generation: String): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = source,
                    displayName = source.take(8),
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/$source.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(source, generation),
            revision = generation,
            contentRoot = "/private/$source/source",
            database = "/private/$source/index.sqlite",
            stats = ReadySourceStats(1, 1, 0),
        )

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private class FixedSource(private val result: SearchCorpusResult) : CorpusSearchSource {
        override fun search(query: String, after: String?, limit: Int): SearchCorpusResult = result

        override fun close() = Unit
    }

    private companion object {
        const val SOURCE_A = "0123456789abcdef0123456789abcdef"
        const val SOURCE_B = "fedcba9876543210fedcba9876543210"
    }
}
