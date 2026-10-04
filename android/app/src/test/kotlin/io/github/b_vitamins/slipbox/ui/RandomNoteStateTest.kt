/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.RandomNodeResult
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RandomNoteStateTest {

    @Test
    fun oneSelectionDeliversTheCanonicalNode() {
        val calls = AtomicInteger()
        val state =
            state(
                RandomNoteSourceFactory {
                    source {
                        calls.incrementAndGet()
                        RandomNodeResult(note("chosen"))
                    }
                },
            )
        val chosen = AtomicReference<NodeRecord?>()

        state.choose(chosen::set)

        await { chosen.get() != null }
        assertEquals("chosen", chosen.get()?.title)
        assertEquals(1, calls.get())
        assertEquals(RandomNotePhase.Idle, state.phase)
        state.close()
    }

    @Test
    fun repeatedTapsCannotStartParallelSelections() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val calls = AtomicInteger()
        val chosen = AtomicReference<NodeRecord?>()
        val state =
            state(
                RandomNoteSourceFactory {
                    source {
                        calls.incrementAndGet()
                        entered.countDown()
                        release.await(5, TimeUnit.SECONDS)
                        RandomNodeResult(note("once"))
                    }
                },
            )

        state.choose(chosen::set)
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        state.choose(chosen::set)
        release.countDown()

        await { chosen.get() != null }
        assertEquals(1, calls.get())
        state.close()
    }

    private fun state(factory: RandomNoteSourceFactory): RandomNoteState =
        RandomNoteState(ready(), factory, ImportDelivery { it() })

    private fun source(choose: () -> RandomNodeResult): RandomNoteSource =
        object : RandomNoteSource {
            override fun choose(): RandomNodeResult = choose()

            override fun close() = Unit
        }

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
            stats = ReadySourceStats(1, 1, 0),
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
