/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.AnchorExplorationRecord
import io.github.b_vitamins.slipbox.engine.BridgeEvidenceRecord
import io.github.b_vitamins.slipbox.engine.ExplorationEntry
import io.github.b_vitamins.slipbox.engine.ExplorationExplanation
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.ExplorationSection
import io.github.b_vitamins.slipbox.engine.ExplorationSectionKind
import io.github.b_vitamins.slipbox.engine.ExploreResult
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.UnlinkedReferenceRecord
import io.github.b_vitamins.slipbox.engine.UnlinkedReferencesResult
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
import org.junit.Assert.assertTrue
import org.junit.Test

class ReaderDiscoveryStateTest {

    @Test
    fun eachDiscoveryIsLazyRunsOnceAndRefreshIsExplicit() {
        val relatedCalls = AtomicInteger()
        val mentionCalls = AtomicInteger()
        val source =
            object : ReaderDiscoverySource {
                override fun related(nodeKey: String, limit: Int): ExploreResult {
                    assertEquals(NODE_KEY, nodeKey)
                    assertEquals(RELATED_QUERY_LIMIT, limit)
                    relatedCalls.incrementAndGet()
                    return relatedResult(candidate("related", connector("via")))
                }

                override fun mentions(nodeKey: String, limit: Int): UnlinkedReferencesResult {
                    assertEquals(NODE_KEY, nodeKey)
                    assertEquals(MENTIONS_QUERY_LIMIT, limit)
                    mentionCalls.incrementAndGet()
                    return UnlinkedReferencesResult(listOf(mention("source", 1, "Focus")))
                }

                override fun close() = Unit
            }
        val state = state(ReaderDiscoverySourceFactory { source })

        assertEquals(RelatedDiscoveryPhase.Idle, state.related)
        assertEquals(MentionDiscoveryPhase.Idle, state.mentions)
        assertEquals(0, relatedCalls.get())
        assertEquals(0, mentionCalls.get())

        state.revealRelated()
        state.revealRelated()
        await { state.related is RelatedDiscoveryPhase.Ready }
        assertEquals(1, relatedCalls.get())
        assertEquals(0, mentionCalls.get())

        state.revealMentions()
        await { state.mentions is MentionDiscoveryPhase.Ready }
        state.revealMentions()
        assertEquals(1, mentionCalls.get())

        state.refreshRelated()
        await { relatedCalls.get() == 2 && state.related is RelatedDiscoveryPhase.Ready }
        state.refreshMentions()
        await { mentionCalls.get() == 2 && state.mentions is MentionDiscoveryPhase.Ready }
        state.close()
    }

    @Test
    fun oneFailureIsIsolatedAndCancellationSuppressesItsLateReply() {
        val blocked = CountDownLatch(1)
        val release = CountDownLatch(1)
        val closes = AtomicInteger()
        val source =
            object : ReaderDiscoverySource {
                override fun related(nodeKey: String, limit: Int): ExploreResult = error("offline")

                override fun mentions(nodeKey: String, limit: Int): UnlinkedReferencesResult {
                    blocked.countDown()
                    assertTrue(release.await(5, TimeUnit.SECONDS))
                    return UnlinkedReferencesResult(listOf(mention("source", 1, "Focus")))
                }

                override fun close() {
                    closes.incrementAndGet()
                }
            }
        val state = state(ReaderDiscoverySourceFactory { source })

        state.revealRelated()
        await { state.related == RelatedDiscoveryPhase.Failed }
        assertEquals(MentionDiscoveryPhase.Idle, state.mentions)

        state.revealMentions()
        assertTrue(blocked.await(5, TimeUnit.SECONDS))
        state.close()
        release.countDown()
        await { closes.get() >= 2 }

        assertEquals(RelatedDiscoveryPhase.Failed, state.related)
        assertEquals(MentionDiscoveryPhase.Loading, state.mentions)
    }

    @Test
    fun relatedNotesAreGroupedRankedBoundedAndDeduplicatedByIdentity() {
        val common = connector("connector", title = "Same title")
        val sameTitleDifferentIdentity = connector("other-connector", title = "Same title")
        val entries = mutableListOf<ExplorationEntry>()
        entries += candidate("already-listed", common)
        entries += candidate("a", common, connector("support-a"))
        entries += candidate("a", common, connector("duplicate-path"))
        entries += candidate("b", sameTitleDifferentIdentity)
        repeat(8) { index -> entries += candidate("tail-$index", common) }
        val result = relatedDiscovery(relatedResult(*entries.toTypedArray()), setOf("file:already-listed.org"))

        assertEquals(12, result.rankedCount)
        assertEquals(10, result.noteCount)
        assertEquals(
            listOf("file:connector.org", "file:other-connector.org"),
            result.groups.map { it.connector.nodeKey },
        )
        assertEquals(listOf("a", "tail-0", "tail-1"), result.groups.first().notes.take(3).map { it.note.title })

        val bounded = result.bounded(RELATED_DISPLAY_LIMIT)
        assertEquals(RELATED_DISPLAY_LIMIT, bounded.sumOf { it.notes.size })
        assertTrue(bounded.flatMap { it.notes }.any { it.note.title == "a" })
    }

    @Test
    fun mentionsKeepEveryDistinctOccurrenceUnderItsSourceAndHighlightTheExactUnicodeColumn() {
        val exact = mention("one", 10, "Focus", preview = "😀 Focus then Focus", col = 14)
        val another = mention("one", 11, "Focus", preview = "Another Focus", col = 9)
        val duplicate = exact.copy()
        val excluded = mention("listed", 12, "Focus")
        val groups =
            mentionGroups(
                UnlinkedReferencesResult(listOf(exact, another, duplicate, excluded)),
                setOf("file:listed.org"),
            )

        assertEquals(1, groups.size)
        assertEquals(2, groups.single().occurrences.size)
        val excerpt = groups.single().occurrences.first().excerpt
        assertEquals("😀 Focus then ", excerpt.first().text)
        assertEquals("Focus", excerpt.single { it.matched }.text)
        assertEquals("😀 Focus then Focus", excerpt.joinToString("") { it.text })
    }

    private fun state(factory: ReaderDiscoverySourceFactory): ReaderDiscoveryState {
        val ready = ready()
        return ReaderDiscoveryState(
            note = BoundNote(ready.binding, NODE_KEY),
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
        )
    }

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private fun relatedResult(vararg entries: ExplorationEntry): ExploreResult =
        ExploreResult(
            ExplorationLens.BRIDGES,
            listOf(ExplorationSection(ExplorationSectionKind.BRIDGE_CANDIDATES, entries.toList())),
        )

    private fun candidate(
        name: String,
        vararg via: BridgeEvidenceRecord,
    ): ExplorationEntry =
        ExplorationEntry.Anchor(
            AnchorExplorationRecord(
                note(name),
                ExplorationExplanation.BridgeCandidate(emptyList(), via.toList()),
            ),
        )

    private fun connector(name: String, title: String = name): BridgeEvidenceRecord =
        BridgeEvidenceRecord("file:$name.org", null, title)

    private fun mention(
        source: String,
        row: Long,
        matched: String,
        preview: String = "A $matched mention",
        col: Long = 3,
    ): UnlinkedReferenceRecord =
        UnlinkedReferenceRecord(
            sourceNote = note(source),
            sourceAnchor = note("$source-anchor"),
            row = row,
            col = col,
            preview = preview,
            matchedText = matched,
            explanation = ExplorationExplanation.UnlinkedReference(matched),
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
