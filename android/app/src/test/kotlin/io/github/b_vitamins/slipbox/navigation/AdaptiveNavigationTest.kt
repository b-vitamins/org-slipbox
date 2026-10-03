/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AdaptiveNavigationTest {

    private val binding = GenerationBinding(SOURCE, GENERATION)
    private val library = SlipboxRoute.Library("fixed point")
    private val glossary = SlipboxRoute.Glossary(binding, review = true, query = "due")
    private val term = glossary.copy(term = "term-key")
    private val reader = SlipboxRoute.Reader(BoundNote(binding, "note-key"))

    @Test
    fun listRoutesOwnStableScenesIndependentOfTheirLiveQuery() {
        assertEquals("library", listDetailSceneKey(library, listOf(library)))
        assertEquals(glossary.place, listDetailSceneKey(glossary, listOf(library, glossary)))
        assertEquals(
            glossary.place,
            listDetailSceneKey(
                glossary.copy(review = false, query = "changed"),
                listOf(library, glossary),
            ),
        )
    }

    @Test
    fun detailsBelongToTheNearestListAlreadyInTheirHistory() {
        assertEquals(library.place, listDetailSceneKey(reader, listOf(library, reader)))
        assertEquals(
            glossary.place,
            listDetailSceneKey(term, listOf(library, glossary, term)),
        )
        assertEquals(
            glossary.place,
            listDetailSceneKey(reader, listOf(library, glossary, term, reader)),
        )
    }

    @Test
    fun aGlossarySearchHitKeepsTheLibraryAsItsListPane() {
        assertEquals(library.place, listDetailSceneKey(term, listOf(library, term)))
    }

    @Test
    fun utilityRoutesNeverAcquireLayoutSpecificNavigation() {
        assertNull(listDetailSceneKey(SlipboxRoute.About, listOf(library, SlipboxRoute.About)))
        assertNull(
            listDetailSceneKey(
                SlipboxRoute.SourceSettings(SOURCE),
                listOf(library, SlipboxRoute.SourceSettings(SOURCE)),
            ),
        )
    }

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "2026-10-04T00-00-00"
    }
}
