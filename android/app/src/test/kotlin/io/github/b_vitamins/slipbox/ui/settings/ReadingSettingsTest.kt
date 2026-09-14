/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.settings

import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ReadingSettingsTest {

    @get:Rule val folder = TemporaryFolder()

    @Test
    fun anEmptyStoreLeavesTheDefaultsShowing() {
        val settings = ReadingSettings(FakeStore(ReadingPreferencesRecord.Absent))
        assertEquals(ReadingPreferences(), settings.preferences)
        assertFalse(settings.stored)
        assertNull(settings.fault)
    }

    @Test
    fun aStoredChoiceIsTheOneShowing() {
        val recorded = ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = true)
        val settings = ReadingSettings(FakeStore(ReadingPreferencesRecord.Stored(recorded)))
        assertEquals(recorded, settings.preferences)
        assertTrue(settings.stored)
        assertNull(settings.fault)
    }

    @Test
    fun aRecordThatCannotBeUsedIsNeitherTrustedNorDiscarded() {
        val store = FakeStore(ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Malformed))
        val settings = ReadingSettings(store)
        assertEquals(ReadingPreferences(), settings.preferences)
        assertFalse(settings.stored)
        assertEquals(ReadingPreferenceFault.Malformed, settings.fault)
        assertEquals(
            "the record was left where it was",
            emptyList<ReadingPreferences>(),
            store.written,
        )
    }

    @Test
    fun aChoiceIsShownAndRecorded() {
        val store = FakeStore(ReadingPreferencesRecord.Absent)
        val settings = ReadingSettings(store)
        settings.select(SlipboxAppearance.Dark)
        settings.selectReduceMotion(true)
        assertEquals(
            ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = true),
            settings.preferences,
        )
        assertTrue(settings.stored)
        assertEquals(
            listOf(
                ReadingPreferences(SlipboxAppearance.Dark),
                ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = true),
            ),
            store.written,
        )
    }

    @Test
    fun aChoiceThatCannotBeRecordedStillApplies() {
        val store = FakeStore(ReadingPreferencesRecord.Absent, ReadingPreferenceFault.Unwritable)
        val settings = ReadingSettings(store)
        settings.select(SlipboxAppearance.Light)
        assertEquals(SlipboxAppearance.Light, settings.preferences.appearance)
        assertFalse(settings.stored)
        assertEquals(ReadingPreferenceFault.Unwritable, settings.fault)
    }

    @Test
    fun aChoiceThatIsAlreadyRecordedIsNotRecordedAgain() {
        val recorded = ReadingPreferences(SlipboxAppearance.Light)
        val store = FakeStore(ReadingPreferencesRecord.Stored(recorded))
        val settings = ReadingSettings(store)
        settings.select(SlipboxAppearance.Light)
        assertEquals(emptyList<ReadingPreferences>(), store.written)
    }

    /** The defaults are what is showing, not a choice: choosing them records them. */
    @Test
    fun choosingTheDefaultRecordsIt() {
        val store = FakeStore(ReadingPreferencesRecord.Absent)
        val settings = ReadingSettings(store)
        settings.select(SlipboxAppearance.System)
        assertEquals(listOf(ReadingPreferences()), store.written)
        assertTrue(settings.stored)
    }

    @Test
    fun aRecordedChoiceClearsAnEarlierFault() {
        val store = FakeStore(ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Oversized))
        val settings = ReadingSettings(store)
        settings.select(SlipboxAppearance.Dark)
        assertNull(settings.fault)
        assertTrue(settings.stored)
    }

    /** The same settings over the store a reader actually gets: one file on disk. */
    @Test
    fun aChoiceRecordedOnDiskIsTheOneTheNextSettingsShow() {
        val file = File(folder.root, ReadingPreferencesFile.FILE_NAME)
        val first = ReadingSettings(ReadingPreferencesFile(file))
        assertEquals(ReadingPreferences(), first.preferences)
        assertFalse(first.stored)
        first.select(SlipboxAppearance.Dark)
        first.selectReduceMotion(true)
        assertTrue(first.stored)
        assertNull(first.fault)

        val later = ReadingSettings(ReadingPreferencesFile(file))
        assertEquals(
            ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = true),
            later.preferences,
        )
        assertTrue(later.stored)
        assertNull(later.fault)
    }

    @Test
    fun aRecordThatCannotBeReachedOnDiskShowsTheDefaultsAndTheFault() {
        val obstruction = folder.newFile("obstruction")
        val settings =
            ReadingSettings(
                ReadingPreferencesFile(File(obstruction, ReadingPreferencesFile.FILE_NAME)),
            )
        assertEquals(ReadingPreferences(), settings.preferences)
        assertFalse(settings.stored)
        assertEquals(ReadingPreferenceFault.Unreadable, settings.fault)
    }

    @Test
    fun aChoiceThatCannotBeRecordedOnDiskStillApplies() {
        val obstruction = folder.newFile("obstruction")
        val settings =
            ReadingSettings(
                ReadingPreferencesFile(File(obstruction, ReadingPreferencesFile.FILE_NAME)),
            )
        settings.select(SlipboxAppearance.Light)
        assertEquals(SlipboxAppearance.Light, settings.preferences.appearance)
        assertFalse(settings.stored)
        assertEquals(ReadingPreferenceFault.Unwritable, settings.fault)
    }

    private class FakeStore(
        private val record: ReadingPreferencesRecord,
        private val fault: ReadingPreferenceFault? = null,
    ) : ReadingPreferencesStore {
        val written = mutableListOf<ReadingPreferences>()

        override fun read(): ReadingPreferencesRecord = record

        override fun write(preferences: ReadingPreferences): ReadingPreferenceFault? {
            if (fault != null) return fault
            written += preferences
            return null
        }
    }
}
