/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxRouteRestorationTest {

    private val everything = destinations(
        SlipboxSurface.Reader,
        SlipboxSurface.Glossary,
        SlipboxSurface.SourceSettings,
        SlipboxSurface.Connection,
        SlipboxSurface.About,
    )

    private val alphaReady = ready(ALPHA to "g1")

    private fun restore(
        saved: List<String>,
        available: SlipboxDestinations = everything,
        generations: SourceGenerations = alphaReady,
    ) = restoreRoutes(saved, available, generations)

    @Test
    fun theSavedTokensAreTheOnesAHistoryIsWrittenWith() {
        assertEquals(
            listOf(
                """{"route":"library"}""",
                """{"route":"about"}""",
                """{"route":"source-settings","source":"$ALPHA"}""",
                """{"route":"connection"}""",
            ),
            saveRoutes(
                listOf(
                    SlipboxRoute.Library(),
                    SlipboxRoute.About,
                    SlipboxRoute.SourceSettings(ALPHA),
                    SlipboxRoute.Connection(),
                ),
            ),
        )
    }

    @Test
    fun aSavedEntryCarriesTheIdentityAndPositionAndNothingElse() {
        assertEquals(
            """{"route":"reader","note":{"binding":{"source":"$ALPHA","generation":"g1"},""" +
                """"nodeKey":"note-1"},"anchor":{"mark":"figure-2","progress":0.5}}""",
            saveRoutes(
                listOf(note(ALPHA, "note-1").copy(anchor = ReadingAnchor("figure-2", 0.5f))),
            ).single(),
        )
        assertEquals(
            """{"route":"glossary","binding":{"source":"$ALPHA","generation":"g1"},""" +
                """"term":"monad","query":"mon"}""",
            saveRoutes(
                listOf(SlipboxRoute.Glossary(bind(ALPHA), term = "monad", query = "mon")),
            ).single(),
        )
    }

    @Test
    fun aHistorySurvivesBeingSavedAndRead() {
        val history = listOf(
            SlipboxRoute.Library("kant"),
            note(ALPHA, "note-1").copy(anchor = ReadingAnchor("figure-2", 0.5f)),
            SlipboxRoute.Glossary(bind(ALPHA), term = "monad", query = "mon"),
        )
        assertEquals(history, restore(saveRoutes(history)))
    }

    @Test
    fun aSavedHistoryReturnsThroughTheOwnerItWasTakenFrom() {
        val stack = SlipboxBackStack(emptyList(), everything, alphaReady)
        assertTrue(stack.open(SlipboxRoute.About))
        assertTrue(stack.back())
        assertTrue(stack.open(note(ALPHA, "note-1").copy(anchor = ReadingAnchor("h2", 0.25f))))
        val saved = saveRoutes(stack.entries)
        val restored = SlipboxBackStack(restore(saved), everything, alphaReady)
        assertEquals(stack.entries.toList(), restored.entries)
    }

    @Test
    fun anEntryThatNoLongerReadsLeavesItsNeighbours() {
        assertEquals(
            listOf(SlipboxRoute.Library(), SlipboxRoute.About),
            restore(
                listOf("""{"route":"library"}""", "not a route at all", """{"route":"about"}"""),
            ),
        )
    }

    @Test
    fun anEntryThisReleaseDoesNotUnderstandIsDropped() {
        assertEquals(
            listOf(SlipboxRoute.Library(), SlipboxRoute.About),
            restore(
                listOf(
                    """{"route":"library"}""",
                    """{"route":"synthesis","source":"$ALPHA"}""",
                    """{"route":"source-settings","source":"$ALPHA","mirror":"origin"}""",
                    """{"route":"about"}""",
                ),
            ),
        )
    }

    @Test
    fun anEntryNamingASurfaceThisBuildDoesNotPresentIsDropped() {
        assertEquals(
            listOf(SlipboxRoute.Library("kant"), SlipboxRoute.About),
            restore(
                saveRoutes(
                    listOf(
                        SlipboxRoute.Library("kant"),
                        note(ALPHA, "note-1"),
                        SlipboxRoute.About,
                    ),
                ),
                available = destinations(SlipboxSurface.About),
            ),
        )
    }

    @Test
    fun aSavedIdentityNoSourceCouldAnswerForIsDropped() {
        assertEquals(
            listOf(SlipboxRoute.Library()),
            restore(
                listOf(
                    """{"route":"library"}""",
                    """{"route":"source-settings","source":"notes.git"}""",
                    """{"route":"reader","note":{"binding":{"source":"$ALPHA",""" +
                        """"generation":"g1"},"nodeKey":""}}""",
                ),
            ),
        )
    }

    @Test
    fun aSavedPositionResolvesAgainstTheGenerationThatIsReadyNow() {
        val stale = note(ALPHA, "note-1", generation = "g1")
        val saved = saveRoutes(listOf(stale.copy(anchor = ReadingAnchor("h2", 0.5f))))
        val restored = restore(saved, generations = ready(ALPHA to "g9")).single()
        assertEquals(bind(ALPHA, "g9"), restored.reads)
        assertEquals(ReadingAnchor("h2", 0.5f), (restored as SlipboxRoute.Reader).anchor)
        assertEquals("$ALPHA:reading_reference:note-1", restored.note.reference)
    }

    @Test
    fun anEntryReadingASourceTheDeviceNoLongerHasIsDropped() {
        assertEquals(
            listOf(SlipboxRoute.Library("kant"), SlipboxRoute.SourceSettings(BETA)),
            restore(
                saveRoutes(
                    listOf(
                        SlipboxRoute.Library("kant"),
                        note(BETA, "note-1"),
                        SlipboxRoute.SourceSettings(BETA),
                    ),
                ),
            ),
        )
    }

    @Test
    fun anotherSourcesNoteIsNeverShownUnderARestoredKey() {
        val saved = saveRoutes(listOf(SlipboxRoute.Library(), note(BETA, "note-1")))
        val restored = restore(saved, generations = ready(ALPHA to "g1", BETA to "g4"))
        assertEquals(BETA, restored.last().reads?.source)
        assertEquals(listOf(SlipboxRoute.Library()), restore(saved, generations = alphaReady))
    }

    @Test
    fun adjacentEntriesForOnePlaceReturnAsOne() {
        val opened = note(ALPHA, "note-1")
        assertEquals(
            listOf(SlipboxRoute.Library("kant"), opened.copy(anchor = ReadingAnchor("h2", 0.5f))),
            restore(
                saveRoutes(
                    listOf(
                        SlipboxRoute.Library(),
                        SlipboxRoute.Library("kant"),
                        opened,
                        opened.copy(anchor = ReadingAnchor("h2", 0.5f)),
                    ),
                ),
            ),
        )
    }

    @Test
    fun aHistoryOfNothingReadableRestoresTheLibraryAlone() {
        assertEquals(
            listOf(SlipboxRoute.Start),
            SlipboxBackStack(restore(listOf("", "{}")), everything, alphaReady).entries,
        )
    }
}
