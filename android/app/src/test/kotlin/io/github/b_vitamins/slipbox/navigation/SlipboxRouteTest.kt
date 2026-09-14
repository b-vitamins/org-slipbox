/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxRouteTest {

    @Test
    fun readingBeginsInTheLibraryWithNoSearch() {
        assertEquals(SlipboxRoute.Library(), SlipboxRoute.Start)
        assertEquals(SlipboxSurface.Library, SlipboxRoute.Start.surface)
        assertEquals("", (SlipboxRoute.Start as SlipboxRoute.Library).query)
    }

    @Test
    fun theSameKeyUnderTwoSourcesNamesTwoNotes() {
        val here = note(ALPHA, "20260914T101500")
        val there = note(BETA, "20260914T101500")
        assertNotEquals(here, there)
        assertNotEquals(here.place, there.place)
        assertNotEquals(here.note.reference, there.note.reference)
    }

    @Test
    fun aReadingReferenceNamesTheSourceAndKeyAndNotTheGeneration() {
        val first = note(ALPHA, "note-1", generation = "2026-09-14T10-15-00")
        val rebuilt = note(ALPHA, "note-1", generation = "2026-09-14T18-40-00")
        assertEquals("$ALPHA:reading_reference:note-1", first.note.reference)
        assertEquals(first.note.reference, rebuilt.note.reference)
        assertEquals(first.place, rebuilt.place)
        assertNotEquals(first.reads, rebuilt.reads)
    }

    @Test
    fun aPlaceIsIndifferentToThePositionAndSearchItCarries() {
        val opened = note(ALPHA, "note-1")
        val resumed = opened.copy(anchor = ReadingAnchor("figure-2", 0.75f))
        assertEquals(opened.place, resumed.place)
        assertEquals(SlipboxRoute.Library().place, SlipboxRoute.Library("kant").place)
        val glossary = SlipboxRoute.Glossary(bind(ALPHA), term = "monad")
        assertEquals(glossary.place, glossary.copy(query = "mon").place)
        assertNotEquals(glossary.place, glossary.copy(term = "functor").place)
        assertNotEquals(glossary.place, glossary.copy(term = null).place)
    }

    @Test
    fun everyRouteNamesTheSurfaceThatPresentsIt() {
        assertEquals(SlipboxSurface.Reader, note(ALPHA, "note-1").surface)
        assertEquals(SlipboxSurface.Glossary, SlipboxRoute.Glossary(bind(ALPHA)).surface)
        assertEquals(SlipboxSurface.SourceSettings, SlipboxRoute.SourceSettings(ALPHA).surface)
        assertEquals(SlipboxSurface.Connection, SlipboxRoute.Connection().surface)
        assertEquals(SlipboxSurface.About, SlipboxRoute.About.surface)
    }

    @Test
    fun onlyTheCorpusReadingRoutesReadACorpus() {
        assertEquals(bind(ALPHA), note(ALPHA, "note-1").reads)
        assertEquals(bind(ALPHA), SlipboxRoute.Glossary(bind(ALPHA)).reads)
        assertNull(SlipboxRoute.Library("kant").reads)
        assertNull(SlipboxRoute.SourceSettings(ALPHA).reads)
        assertNull(SlipboxRoute.Connection(ALPHA).reads)
        assertNull(SlipboxRoute.About.reads)
    }

    @Test
    fun onlyAMintedSourceIdentityIsCanonical() {
        assertTrue(isSourceIdentity(ALPHA))
        assertFalse(isSourceIdentity(ALPHA.uppercase()))
        assertFalse(isSourceIdentity(ALPHA.drop(1)))
        assertFalse(isSourceIdentity(ALPHA + "0"))
        assertFalse(isSourceIdentity("https://example.invalid/notes.git"))
        assertFalse(isSourceIdentity(""))
    }

    @Test
    fun aGenerationIdentityIsBoundedAndUnpunctuated() {
        assertTrue(isGenerationIdentity("2026-09-14T10-15-00.4"))
        assertTrue(isGenerationIdentity("a".repeat(64)))
        assertFalse(isGenerationIdentity("a".repeat(65)))
        assertFalse(isGenerationIdentity(""))
        assertFalse(isGenerationIdentity("2026/09/14"))
        assertFalse(isGenerationIdentity("g 1"))
    }

    @Test
    fun aRouteCarryingAnIdentityNoSourceCouldAnswerForIsNotCanonical() {
        assertTrue(note(ALPHA, "note-1").isCanonical())
        assertFalse(note("notes.git", "note-1").isCanonical())
        assertFalse(note(ALPHA, "note-1", generation = "").isCanonical())
        assertFalse(note(ALPHA, "").isCanonical())
        assertFalse(note(ALPHA, "note\u0000one").isCanonical())
        assertFalse(note(ALPHA, " note-1").isCanonical())
        assertFalse(note(ALPHA, "a".repeat(1025)).isCanonical())
        assertTrue(note(ALPHA, "a".repeat(1024)).isCanonical())
    }

    @Test
    fun aPositionOutsideTheNoteIsNotCanonical() {
        val opened = note(ALPHA, "note-1")
        assertTrue(opened.copy(anchor = ReadingAnchor("figure-2", 1f)).isCanonical())
        assertFalse(opened.copy(anchor = ReadingAnchor(progress = 1.5f)).isCanonical())
        assertFalse(opened.copy(anchor = ReadingAnchor(progress = -0.1f)).isCanonical())
        assertFalse(opened.copy(anchor = ReadingAnchor(progress = Float.NaN)).isCanonical())
        assertFalse(opened.copy(anchor = ReadingAnchor(mark = "figure\n2")).isCanonical())
    }

    @Test
    fun theRoutesThatNameASourceWithoutReadingItAreStillCanonicallyIdentified() {
        assertTrue(SlipboxRoute.SourceSettings(ALPHA).isCanonical())
        assertFalse(SlipboxRoute.SourceSettings("notes.git").isCanonical())
        assertTrue(SlipboxRoute.Connection().isCanonical())
        assertTrue(SlipboxRoute.Connection(ALPHA).isCanonical())
        assertFalse(SlipboxRoute.Connection("notes.git").isCanonical())
    }

    @Test
    fun aSearchIsCarriedOnlyWhileTheEngineCouldAnswerIt() {
        assertTrue(SlipboxRoute.Library("critique of").isCanonical())
        assertFalse(SlipboxRoute.Library("critique\tof").isCanonical())
        assertFalse(SlipboxRoute.Library("a".repeat(1025)).isCanonical())
        assertTrue(SlipboxRoute.Glossary(bind(ALPHA), query = "mon").isCanonical())
        assertFalse(SlipboxRoute.Glossary(bind(ALPHA), term = "").isCanonical())
    }

    @Test
    fun aRouteNamesTheSourceItReadsOrConfigures() {
        assertTrue(note(ALPHA, "note-1").names(ALPHA))
        assertFalse(note(BETA, "note-1").names(ALPHA))
        assertTrue(SlipboxRoute.Glossary(bind(ALPHA)).names(ALPHA))
        assertTrue(SlipboxRoute.SourceSettings(ALPHA).names(ALPHA))
        assertTrue(SlipboxRoute.Connection(ALPHA).names(ALPHA))
        assertFalse(SlipboxRoute.Connection().names(ALPHA))
        assertFalse(SlipboxRoute.Library("kant").names(ALPHA))
        assertFalse(SlipboxRoute.About.names(ALPHA))
    }
}
