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
    ) = SlipboxBackStack(restored.toList(), available, generations)

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
    fun switchingSourceDropsTheOtherCorpusAndKeepsTheSearch() {
        val stack = history(SlipboxRoute.Library("kant"))
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertTrue(stack.switchSource(bind(BETA)))
        assertEquals(
            listOf(SlipboxRoute.Library("kant"), SlipboxRoute.SourceSettings(ALPHA)),
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
        val stack = history()
        assertTrue(stack.open(note(ALPHA, "note-1")))
        assertTrue(stack.open(SlipboxRoute.SourceSettings(ALPHA)))
        assertTrue(stack.open(note(BETA, "note-1")))
        assertTrue(stack.removeSource(ALPHA))
        assertEquals(listOf(SlipboxRoute.Start, note(BETA, "note-1")), stack.entries)
        assertFalse(stack.removeSource(ALPHA))
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
