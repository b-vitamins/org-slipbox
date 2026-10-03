/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.AnchorExplorationRecord
import io.github.b_vitamins.slipbox.engine.ExplorationEntry
import io.github.b_vitamins.slipbox.engine.ExplorationExplanation
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.ExplorationSection
import io.github.b_vitamins.slipbox.engine.ExplorationSectionKind
import io.github.b_vitamins.slipbox.engine.ExploreResult
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
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class ReaderExplorationStateTest {

    @Test
    fun everyLensIsExplicitBoundedAndRefreshIsTheOnlyRepeat() {
        val calls = mutableListOf<Triple<String, ExplorationLens, Int>>()
        val state =
            state(
                anchor = note(),
                factory =
                    ReaderExplorationSourceFactory {
                        source { nodeKey, lens, limit ->
                            synchronized(calls) { calls += Triple(nodeKey, lens, limit) }
                            emptyResult(lens)
                        }
                    },
            )

        assertEquals(ReaderExplorationPhase.AwaitingLens, state.phase)
        assertTrue(calls.isEmpty())

        for (lens in ExplorationLens.entries) {
            state.select(lens)
            await { state.phase !is ReaderExplorationPhase.Loading }
            assertEquals(ReaderExplorationPhase.Empty(lens), state.phase)
            state.select(lens)
        }
        val selectedCalls = synchronized(calls) { calls.toList() }
        assertEquals(ExplorationLens.entries.size, selectedCalls.size)
        assertTrue(
            selectedCalls.all { (key, _, limit) ->
                key == NODE_KEY && limit == EXPLORATION_QUERY_LIMIT
            },
        )

        state.refresh()
        await { synchronized(calls) { calls.size == ExplorationLens.entries.size + 1 } }
        assertEquals(ExplorationLens.UNRESOLVED, synchronized(calls) { calls.last().second })
        state.close()
    }

    @Test
    fun canonicalSectionsEntriesExplanationsAndEngineOrderArePreserved() {
        val first = note("unresolved").copy(todoKeyword = "TODO")
        val second = note("weak")
        val result =
            ExploreResult(
                lens = ExplorationLens.UNRESOLVED,
                sections =
                    listOf(
                        ExplorationSection(
                            ExplorationSectionKind.UNRESOLVED_TASKS,
                            listOf(
                                ExplorationEntry.Anchor(
                                    AnchorExplorationRecord(
                                        first,
                                        ExplorationExplanation.UnresolvedSharedReference(
                                            references = listOf("cite:one"),
                                            todoKeyword = "TODO",
                                        ),
                                    ),
                                ),
                            ),
                        ),
                        ExplorationSection(
                            ExplorationSectionKind.WEAKLY_INTEGRATED_NOTES,
                            listOf(
                                ExplorationEntry.Anchor(
                                    AnchorExplorationRecord(
                                        second,
                                        ExplorationExplanation.WeaklyIntegratedSharedReference(
                                            references = listOf("cite:two"),
                                            structuralLinkCount = 1,
                                            viaNotes = emptyList(),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
            )
        val state = state(factory = ReaderExplorationSourceFactory { source { _, _, _ -> result } })

        state.select(ExplorationLens.UNRESOLVED)
        await { state.phase is ReaderExplorationPhase.Ready }

        assertSame(result, (state.phase as ReaderExplorationPhase.Ready).result)
        assertEquals(
            listOf(first.nodeKey, second.nodeKey),
            result.sections.flatMap { it.entries }.map {
                (it as ExplorationEntry.Anchor).record.anchor.nodeKey
            },
        )
        state.close()
    }

    @Test
    fun malformedCanonicalShapeFailsAndMissingSubstrateIsNotReportedAsEmpty() {
        val wrongSection =
            ExploreResult(
                ExplorationLens.TIME,
                listOf(
                    ExplorationSection(
                        ExplorationSectionKind.TIME_NEIGHBORS,
                        listOf(
                            ExplorationEntry.Anchor(
                                AnchorExplorationRecord(
                                    note("wrong"),
                                    ExplorationExplanation.TaskNeighbor(null, emptyList()),
                                ),
                            ),
                        ),
                    ),
                ),
            )
        assertThrows(IllegalArgumentException::class.java) {
            validateExploration(ExplorationLens.TIME, wrongSection)
        }

        val state =
            state(
                anchor = note().copy(
                    refs = emptyList(),
                    todoKeyword = null,
                    scheduledFor = null,
                    deadlineFor = null,
                    closedAt = null,
                    fileMtimeNs = 0,
                    backlinkCount = 0,
                    forwardLinkCount = 0,
                ),
                factory =
                    ReaderExplorationSourceFactory {
                        source { _, lens, _ -> emptyResult(lens) }
                    },
            )
        state.select(ExplorationLens.TIME)
        await { state.phase !is ReaderExplorationPhase.Loading }
        assertEquals(
            ReaderExplorationPhase.NoSubstrate(ExplorationLens.TIME),
            state.phase,
        )
        state.close()
    }

    @Test
    fun newerLensCancelsAndSuppressesTheOlderReplyWhileFailuresStayLocal() {
        val blocked = CountDownLatch(1)
        val release = CountDownLatch(1)
        val finished = CountDownLatch(1)
        val opens = AtomicInteger()
        val state =
            state(
                factory =
                    ReaderExplorationSourceFactory {
                        val call = opens.incrementAndGet()
                        source { _, lens, _ ->
                            if (call == 1) {
                                blocked.countDown()
                                assertTrue(release.await(5, TimeUnit.SECONDS))
                                finished.countDown()
                            }
                            if (lens == ExplorationLens.REFS) emptyResult(lens) else error("stale")
                        }
                    },
            )

        state.select(ExplorationLens.STRUCTURE)
        assertTrue(blocked.await(5, TimeUnit.SECONDS))
        state.select(ExplorationLens.REFS)
        await { state.phase == ReaderExplorationPhase.Empty(ExplorationLens.REFS) }
        release.countDown()
        assertTrue(finished.await(5, TimeUnit.SECONDS))
        assertEquals(ReaderExplorationPhase.Empty(ExplorationLens.REFS), state.phase)

        state.select(ExplorationLens.BRIDGES)
        await { state.phase is ReaderExplorationPhase.Failed }
        assertEquals(
            ReaderExplorationPhase.Failed(ExplorationLens.BRIDGES),
            state.phase,
        )
        state.close()
    }

    private fun state(
        anchor: NodeRecord = note(),
        factory: ReaderExplorationSourceFactory,
    ): ReaderExplorationState {
        val ready = ready()
        return ReaderExplorationState(
            note = BoundNote(ready.binding, anchor.nodeKey),
            anchor = anchor,
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
        )
    }

    private fun source(
        answer: (String, ExplorationLens, Int) -> ExploreResult,
    ): ReaderExplorationSource =
        object : ReaderExplorationSource {
            override fun explore(
                nodeKey: String,
                lens: ExplorationLens,
                limit: Int,
            ): ExploreResult = answer(nodeKey, lens, limit)

            override fun close() = Unit
        }

    private fun emptyResult(lens: ExplorationLens): ExploreResult =
        ExploreResult(lens, lens.sectionKinds().map { ExplorationSection(it, emptyList()) })

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private fun note(name: String = "focus"): NodeRecord =
        NodeRecord(
            nodeKey = if (name == "focus") NODE_KEY else "file:$name.org",
            explicitId = null,
            filePath = "$name.org",
            title = name,
            outlinePath = "",
            aliases = emptyList(),
            tags = emptyList(),
            refs = listOf("cite:shared"),
            todoKeyword = "TODO",
            scheduledFor = "2026-10-03",
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
            fileMtimeNs = 1,
            backlinkCount = 1,
            forwardLinkCount = 1,
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
