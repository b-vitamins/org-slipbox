/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxBackStackTest {

    private val everything = destinations(
        SlipboxSurface.Reader,
        SlipboxSurface.Glossary,
        SlipboxSurface.SourceSettings,
        SlipboxSurface.Connection,
        SlipboxSurface.About,
    )

    private fun history(
        vararg restored: SlipboxRoute,
        available: SlipboxDestinations = everything,
        generations: SourceGenerations = ready(ALPHA to "g1", BETA to "g1"),
        trails: ReadingTrailSink = ReadingTrailSink.None,
    ) = SlipboxBackStack(restored.toList(), available, generations, trails)

    @Test
    fun aHistoryOfNothingBeginsInTheLibrary() {
        val stack = history()
        assertEquals(listOf(SlipboxRoute.Start), stack.entries)
        assertEquals(SlipboxRoute.Start, stack.current)
    }

    @Test
    fun aRestoredHistoryKeepsItsOwnRootAndOrder() {
        val stack = history(SlipboxRoute.Library("kant"), note(ALPHA, "note-1"))
        assertEquals(
            listOf(SlipboxRoute.Library("kant"), note(ALPHA, "note-1")),
            stack.entries,
        )
    }

    @Test
    fun aRestoredHistoryMissingItsRootRegainsIt() {
        val stack = history(SlipboxRoute.About)
        assertEquals(listOf(SlipboxRoute.Start, SlipboxRoute.About), stack.entries)
    }

    @Test
    fun openingADestinationPresentsIt() {
        val stack = history()
        assertTrue(stack.open(SlipboxRoute.About))
        assertEquals(listOf(SlipboxRoute.Start, SlipboxRoute.About), stack.entries)
    }

    @Test
    fun aLiveLibraryQueryRemainsBeneathTheResultItOpened() {
        val origin = SlipboxRoute.Library()
        val stack = history(origin)

        assertTrue(stack.rememberLibrarySearch(origin, "fixed point"))
        assertTrue(stack.open(note(ALPHA, "result-1")))
        assertFalse(stack.rememberLibrarySearch(origin, "stale"))
        assertTrue(stack.back())

        assertEquals(SlipboxRoute.Library("fixed point"), stack.current)
    }

    @Test
    fun openingThePlaceAlreadyPresentedDoesNotGrowTheHistory() {
        val stack = history()
        val opened = note(ALPHA, "note-1")
        assertTrue(stack.open(opened))
        assertTrue(stack.open(opened.copy(anchor = ReadingAnchor("figure-2", 0.5f))))
        assertEquals(2, stack.entries.size)
        val presented = stack.current as SlipboxRoute.Reader
        assertEquals(ReadingAnchor("figure-2", 0.5f), presented.anchor)
        assertTrue(stack.back())
        assertEquals(listOf(SlipboxRoute.Start), stack.entries)
    }

    @Test
    fun livePositionsAndBackPersistOnlyTheSourceScopedReaderPath() {
        val saved = mutableListOf<Pair<String, List<SlipboxRoute.Reader>>>()
        val origin = note(ALPHA, "note-1")
        val stack = history(origin, trails = ReadingTrailSink { source, routes -> saved += source to routes })

        assertTrue(stack.rememberReadingPlace(origin, ReadingAnchor("block:0", 0.4f, 0.2f)))
        assertEquals(ALPHA, saved.last().first)
        assertEquals(ReadingAnchor("block:0", 0.4f, 0.2f), saved.last().second.single().anchor)

        assertTrue(stack.back())
        assertTrue(saved.last().second.isEmpty())
    }

    @Test
    fun anExplicitIdentityPreventsAChangedKeyFromGrowingThePresentedPath() {
        val first =
            SlipboxRoute.Reader(
                BoundNote(bind(ALPHA), "file:before.org", "stable-id", "before.org"),
            )
        val moved =
            SlipboxRoute.Reader(
                BoundNote(bind(ALPHA), "file:after.org", "stable-id", "after.org"),
            )
        val stack = history(first)

        assertTrue(stack.open(moved))

        assertEquals(2, stack.entries.size)
        assertEquals("file:after.org", (stack.current as SlipboxRoute.Reader).note.nodeKey)
    }

    @Test
    fun followingSavesTheOriginAndBackRestoresItsPosition() {
        val origin = note(ALPHA, "note-1")
        val stack = history(origin)

        assertTrue(stack.follow(origin, "note-2", ReadingAnchor("paragraph-7", 0.42f)))
        assertEquals(note(ALPHA, "note-2"), stack.current)
        assertTrue(stack.back())
        assertEquals(
            origin.copy(anchor = ReadingAnchor("paragraph-7", 0.42f)),
            stack.current,
        )
    }

    @Test
    fun followingAnExistingReaderRewindsInsteadOfDuplicatingIt() {
        val first = note(ALPHA, "note-1")
        val stack = history(first)
        assertTrue(stack.follow(first, "note-2", ReadingAnchor(progress = 0.2f)))
        val second = stack.current as SlipboxRoute.Reader

        assertTrue(stack.follow(second, "note-1", ReadingAnchor(progress = 0.7f)))

        assertEquals(2, stack.entries.size)
        assertEquals(first.place, stack.current.place)
        assertEquals(ReadingAnchor(progress = 0.2f), (stack.current as SlipboxRoute.Reader).anchor)
    }

    @Test
    fun followingKeepsTheOriginGenerationEvenWhenANewerOneIsReady() {
        var readyGeneration = "g1"
        val origin = note(ALPHA, "note-1", generation = "g1")
        val stack =
            history(
                origin,
                generations = SourceGenerations { if (it == ALPHA) readyGeneration else null },
            )
        val presented = stack.current as SlipboxRoute.Reader
        readyGeneration = "g2"

        assertTrue(stack.follow(presented, "note-2", ReadingAnchor.Start))

        assertEquals(bind(ALPHA, "g1"), presented.note.binding)
        assertEquals(presented.note.binding, (stack.current as SlipboxRoute.Reader).note.binding)
    }

    @Test
    fun aFollowFromAReaderThatHasGoneIsRefused() {
        val origin = note(ALPHA, "note-1")
        val stack = history(origin)
        assertTrue(stack.open(SlipboxRoute.About))

        assertFalse(stack.follow(origin, "note-2", ReadingAnchor(progress = 0.5f)))
        assertFalse(stack.rememberReadingPlace(origin, ReadingAnchor(progress = 0.5f)))
        assertEquals(SlipboxRoute.About, stack.current)
    }

    @Test
    fun theSameKeyUnderTwoSourcesOpensTwoEntries() {
        val stack = history()
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(note(BETA, "note-1")))
        assertEquals(3, stack.entries.size)
        assertEquals(BETA, stack.current.reads?.source)
    }

    @Test
    fun aSurfaceThisBuildDoesNotPresentIsRefused() {
        val stack = history(available = destinations(SlipboxSurface.About))
        assertFalse(stack.open(note(ALPHA, "note-1")))
        assertFalse(stack.open(SlipboxRoute.Connection()))
        assertFalse(stack.open(SlipboxRoute.Glossary(bind(ALPHA))))
        assertTrue(stack.open(SlipboxRoute.About))
        assertEquals(listOf(SlipboxRoute.Start, SlipboxRoute.About), stack.entries)
    }

    @Test
    fun anIdentityNoSourceCouldAnswerForIsRefused() {
        val stack = history()
        assertFalse(stack.open(note("notes.git", "note-1")))
        assertFalse(stack.open(note(ALPHA, "")))
        assertFalse(stack.open(SlipboxRoute.SourceSettings("notes.git")))
        assertEquals(listOf(SlipboxRoute.Start), stack.entries)
    }

    @Test
    fun aCorpusWithNoReadyGenerationIsRefused() {
        val stack = history(generations = ready(BETA to "g1"))
        assertFalse(stack.open(note(ALPHA, "note-1")))
        assertFalse(stack.open(SlipboxRoute.Glossary(bind(ALPHA))))
        assertTrue(stack.open(note(BETA, "note-1")))
    }

    @Test
    fun aRouteWithNoCorpusToReadNeedsNoGeneration() {
        val stack = history(generations = SourceGenerations.None)
        assertTrue(stack.open(SlipboxRoute.About))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertFalse(stack.open(note(ALPHA, "note-1")))
    }

    @Test
    fun aRouteBindsToTheGenerationThatIsReadyAsItOpens() {
        val stack = history(generations = ready(ALPHA to "g7"))
        assertTrue(stack.open(note(ALPHA, "note-1", generation = "g1")))
        assertEquals(bind(ALPHA, "g7"), stack.current.reads)
    }

    @Test
    fun anEntryKeepsItsGenerationWhenANewerOneIsBuilt() {
        var readyGeneration = "g1"
        val stack = SlipboxBackStack(
            emptyList(),
            everything,
            SourceGenerations { if (it == ALPHA) readyGeneration else null },
        )
        assertTrue(stack.open(note(ALPHA, "note-1")))
        readyGeneration = "g2"
        assertTrue(stack.open(SlipboxRoute.Glossary(bind(ALPHA, "g1"))))
        assertEquals(bind(ALPHA, "g1"), stack.entries[1].reads)
        assertEquals(bind(ALPHA, "g2"), stack.entries[2].reads)
    }

    @Test
    fun backLeavesThePresentedEntry() {
        val stack = history(SlipboxRoute.About)
        assertTrue(stack.back())
        assertEquals(listOf(SlipboxRoute.Start), stack.entries)
    }

    @Test
    fun theRootIsNeverPopped() {
        val stack = history(SlipboxRoute.About)
        assertTrue(stack.back())
        assertFalse(stack.back())
        assertFalse(stack.back())
        assertEquals(listOf(SlipboxRoute.Start), stack.entries)
    }

    @Test
    fun switchingSourceDropsTheOtherCorpusAndItsUnboundSearch() {
        val stack = history(SlipboxRoute.Library("kant"))
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertTrue(stack.switchSource(bind(BETA)))
        assertEquals(
            listOf(SlipboxRoute.Library(), SlipboxRoute.SourceSettings(ALPHA)),
            stack.entries,
        )
    }

    @Test
    fun switchingToTheCorpusAlreadyReadLeavesTheHistoryAlone() {
        val stack = history()
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertFalse(stack.switchSource(bind(ALPHA, "g2")))
        assertEquals(2, stack.entries.size)
    }

    @Test
    fun switchingToAnIdentityNoSourceCouldAnswerForIsRefused() {
        val stack = history()
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertFalse(stack.switchSource(bind("notes.git")))
        assertFalse(stack.switchSource(bind(BETA, generation = "")))
        assertEquals(2, stack.entries.size)
    }

    @Test
    fun removingASourceDropsEveryEntryNamingIt() {
        val stack = history(SlipboxRoute.Library("kant"))
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertTrue(stack.open(note(BETA, "note-1")))
        assertTrue(stack.removeSource(ALPHA))
        assertEquals(listOf(SlipboxRoute.Start, note(BETA, "note-1")), stack.entries)
        assertFalse(stack.removeSource(ALPHA))
    }

    @Test
    fun partialRemovalKeepsOnlyItsSettingsEntryForTheCleanupWarning() {
        val stack = history(SlipboxRoute.Library("kant"))
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))

        assertTrue(stack.removeSource(ALPHA, keepSettings = true))

        assertEquals(
            listOf(SlipboxRoute.Start, SlipboxRoute.SourceSettings(ALPHA)),
            stack.entries,
        )
    }

    @Test
    fun removingAnInactiveSourceDoesNotClearTheActiveSearch() {
        val stack = history(SlipboxRoute.Library("kant"))
        assertTrue(stack.open(note(BETA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))

        assertTrue(stack.removeSource(ALPHA, clearSearch = false))

        assertEquals(
            listOf(SlipboxRoute.Library("kant"), note(BETA, "note-1")),
            stack.entries,
        )
    }

    @Test
    fun aRequestFromAPresentationThatHasGoneIsRefused() {
        val stack = history(SlipboxRoute.About)
        stack.detach()
        assertFalse(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertFalse(stack.back())
        assertFalse(stack.switchSource(bind(BETA)))
        assertFalse(stack.removeSource(ALPHA))
        assertEquals(listOf(SlipboxRoute.Start, SlipboxRoute.About), stack.entries)
    }
}
