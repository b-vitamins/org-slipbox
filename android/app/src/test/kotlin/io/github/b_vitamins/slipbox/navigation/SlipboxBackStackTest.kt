/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxBackStackTest {

    @Test
    fun theIdsAreTheOnesASavedStackIsWrittenWith() {
        assertEquals("library", SlipboxDestination.Library.id)
        assertEquals("about", SlipboxDestination.About.id)
    }

    @Test
    fun aStackSurvivesBeingSavedAndRead() {
        val stack = listOf<SlipboxDestination>(SlipboxDestination.Library, SlipboxDestination.About)
        assertEquals(stack, restoreBackStack(saveBackStack(stack)))
    }

    @Test
    fun theOrderOfTheStackIsKept() {
        assertEquals(
            listOf("library", "about"),
            saveBackStack(listOf(SlipboxDestination.Library, SlipboxDestination.About)),
        )
    }

    @Test
    fun anIdNoDestinationClaimsIsDropped() {
        assertEquals(
            listOf<SlipboxDestination>(SlipboxDestination.Library),
            restoreBackStack(listOf("library", "note")),
        )
        assertNull(SlipboxDestination.fromId("note"))
    }

    @Test
    fun aStackOfNothingButUnknownIdsRestoresTheRoot() {
        assertEquals(listOf(SlipboxDestination.start), restoreBackStack(listOf("note")))
        assertEquals(
            listOf(SlipboxDestination.start),
            restoreBackStack(listOf("note", "chapter")),
        )
    }

    @Test
    fun anEmptyStackRestoresTheRoot() {
        assertEquals(listOf(SlipboxDestination.start), restoreBackStack(emptyList()))
    }

    @Test
    fun anUnknownRootBeneathAboutRegainsTheRoot() {
        assertEquals(
            listOf(SlipboxDestination.Library, SlipboxDestination.About),
            restoreBackStack(listOf("note", "about")),
        )
    }

    @Test
    fun anAboutOnlyStackRegainsTheRootBeneathIt() {
        assertEquals(
            listOf(SlipboxDestination.Library, SlipboxDestination.About),
            restoreBackStack(listOf("about")),
        )
    }

    @Test
    fun aPopRemovesTheTopAndReportsIt() {
        val stack = mutableListOf<SlipboxDestination>(
            SlipboxDestination.Library,
            SlipboxDestination.About,
        )
        assertTrue(stack.popDestination())
        assertEquals(listOf(SlipboxDestination.Library), stack)
    }

    @Test
    fun repeatedPopsCannotEmptyTheStack() {
        val stack = mutableListOf<SlipboxDestination>(
            SlipboxDestination.Library,
            SlipboxDestination.About,
        )
        assertTrue(stack.popDestination())
        assertFalse(stack.popDestination())
        assertFalse(stack.popDestination())
        assertEquals(listOf(SlipboxDestination.start), stack)
    }

    @Test
    fun aReaderBeginsInTheLibrary() {
        assertSame(SlipboxDestination.Library, SlipboxDestination.start)
    }

    @Test
    fun aForwardNavigationAddsItsDestination() {
        val stack = mutableListOf(SlipboxDestination.start)
        assertTrue(stack.pushDestination(SlipboxDestination.About))
        assertEquals(listOf(SlipboxDestination.Library, SlipboxDestination.About), stack)
    }

    @Test
    fun repeatedForwardNavigationDoesNotDuplicateTheTop() {
        val stack = mutableListOf(SlipboxDestination.start)
        assertTrue(stack.pushDestination(SlipboxDestination.About))
        assertFalse(stack.pushDestination(SlipboxDestination.About))
        assertTrue(stack.popDestination())
        assertEquals(listOf(SlipboxDestination.start), stack)
    }

    @Test
    fun duplicateRootsAndAdjacentDestinationsRestoreACanonicalStack() {
        assertEquals(
            listOf(SlipboxDestination.Library, SlipboxDestination.About),
            restoreBackStack(listOf("library", "library", "about", "about", "library")),
        )
    }
}
