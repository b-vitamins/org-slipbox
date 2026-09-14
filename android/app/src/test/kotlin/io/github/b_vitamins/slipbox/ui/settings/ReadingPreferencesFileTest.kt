/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.settings

import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ReadingPreferencesFileTest {

    @get:Rule val folder = TemporaryFolder()

    @Test
    fun aStoreThatWasNeverWrittenIsAbsent() {
        assertEquals(ReadingPreferencesRecord.Absent, store().read())
    }

    @Test
    fun everyChoiceSurvivesARoundTrip() {
        for (appearance in SlipboxAppearance.entries) {
            for (reduceMotion in listOf(false, true)) {
                val store = store("$appearance-$reduceMotion")
                val written = ReadingPreferences(appearance, reduceMotion)
                assertNull(store.write(written))
                assertEquals(ReadingPreferencesRecord.Stored(written), store.read())
            }
        }
    }

    @Test
    fun aWriteReplacesWhatWasThereAndLeavesNothingBeside() {
        val file = folder.newFile("preferences")
        val store = ReadingPreferencesFile(file)
        assertNull(store.write(ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = false)))
        assertNull(store.write(ReadingPreferences(SlipboxAppearance.Light, reduceMotion = true)))
        assertEquals(
            ReadingPreferencesRecord.Stored(
                ReadingPreferences(SlipboxAppearance.Light, reduceMotion = true),
            ),
            store.read(),
        )
        assertEquals(listOf(file.name), folder.root.list()?.sorted())
    }

    @Test
    fun aRecordTooLargeToBeOneIsRefused() {
        assertEquals(
            ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Oversized),
            stored("x".repeat(513)).read(),
        )
    }

    @Test
    fun aRecordFromALaterVersionIsRefusedAndKept() {
        val file = write("later", "slipbox.reading 2\nappearance=Dark\nreduce-motion=false\n")
        val before = file.readText()
        assertEquals(
            ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.UnsupportedVersion),
            ReadingPreferencesFile(file).read(),
        )
        assertEquals("a record read but not understood is left alone", before, file.readText())
    }

    @Test
    fun everyRecordThatIsNotOneIsMalformed() {
        val texts =
            mapOf(
                "no header" to "appearance=Dark\nreduce-motion=false\n",
                "wrong magic" to "slipbox.other 1\nappearance=Dark\nreduce-motion=false\n",
                "no version" to "slipbox.reading\nappearance=Dark\nreduce-motion=false\n",
                "unreadable version" to "slipbox.reading one\nappearance=Dark\n",
                "unknown key" to "slipbox.reading 1\nappearance=Dark\nreduce-motion=false\nx=1\n",
                "unknown appearance" to
                    "slipbox.reading 1\nappearance=Sepia\nreduce-motion=false\n",
                "unreadable flag" to "slipbox.reading 1\nappearance=Dark\nreduce-motion=maybe\n",
                "missing flag" to "slipbox.reading 1\nappearance=Dark\n",
                "missing appearance" to "slipbox.reading 1\nreduce-motion=true\n",
                "no separator" to "slipbox.reading 1\nappearance\n",
                "empty key" to "slipbox.reading 1\n=Dark\n",
                "nothing at all" to "",
            )
        for ((name, text) in texts) {
            assertEquals(
                name,
                ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Malformed),
                stored(text, name).read(),
            )
        }
    }

    @Test
    fun aChoiceThatCannotBeRecordedSaysSo() {
        val obstruction = folder.newFile("obstruction")
        val store = ReadingPreferencesFile(File(obstruction, "preferences"))
        assertEquals(
            ReadingPreferenceFault.Unwritable,
            store.write(ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = true)),
        )
        assertTrue("nothing was left beside the obstruction", obstruction.isFile)
    }

    @Test
    fun aParentThatIsNotADirectoryIsAFaultAndNotAMissingRecord() {
        val obstruction = folder.newFile("obstruction")
        assertEquals(
            unusable(ReadingPreferenceFault.Unreadable),
            ReadingPreferencesFile(File(obstruction, "preferences")).read(),
        )
    }

    @Test
    fun aRecordBehindADeniedDirectoryIsAFaultAndNotAMissingRecord() {
        val directory = folder.newFolder("denied")
        val file = File(directory, "preferences")
        file.writeText(RECORD)
        // The test reads as the owner, so the owner's bits are the ones that deny it.
        assertTrue("the directory is closed", directory.setReadable(false))
        assertTrue("and left unsearchable", directory.setExecutable(false))
        try {
            assertEquals(
                unusable(ReadingPreferenceFault.Unreadable),
                ReadingPreferencesFile(file).read(),
            )
        } finally {
            directory.setExecutable(true)
            directory.setReadable(true)
        }
    }

    @Test
    fun aRecordThatCannotBeOpenedIsAFaultAndNotAMissingRecord() {
        val file = folder.newFile("closed")
        file.writeText(RECORD)
        assertTrue("the record is closed", file.setReadable(false))
        try {
            assertEquals(
                unusable(ReadingPreferenceFault.Unreadable),
                ReadingPreferencesFile(file).read(),
            )
        } finally {
            file.setReadable(true)
        }
    }

    @Test
    fun aDirectoryWhereTheRecordBelongsIsAFaultAndNotAMissingRecord() {
        val file = File(folder.root, "preferences")
        assertTrue(file.mkdir())
        assertEquals(
            unusable(ReadingPreferenceFault.Unreadable),
            ReadingPreferencesFile(file).read(),
        )
    }

    @Test
    fun aStreamLongerThanTheLengthItReportsIsRefused() {
        val file = folder.newFile("stale")
        file.writeText(RECORD + "\n".repeat(131_072))
        assertEquals(
            unusable(ReadingPreferenceFault.Oversized),
            ReadingPreferencesFile(StaleLength(file, RECORD.length.toLong())).read(),
        )
    }

    @Test
    fun theLimitIsTheLastByteARecordMayHave() {
        assertEquals(
            "at the limit the bytes are read, and refused as a record rather than as a size",
            unusable(ReadingPreferenceFault.Malformed),
            stored("x".repeat(512), "at-the-limit").read(),
        )
        assertEquals(
            unusable(ReadingPreferenceFault.Oversized),
            stored("x".repeat(513), "past-the-limit").read(),
        )
    }

    @Test
    fun bytesThatAreNotTextAreRefused() {
        val file = folder.newFile("not-text")
        file.writeBytes(
            "slipbox.reading 1\nappearance=".toByteArray() +
                byteArrayOf(0xC3.toByte()) +
                "\nreduce-motion=false\n".toByteArray(),
        )
        assertEquals(
            unusable(ReadingPreferenceFault.Malformed),
            ReadingPreferencesFile(file).read(),
        )
    }

    @Test
    fun aFieldStatedTwiceIsRefusedRatherThanResolved() {
        val texts =
            mapOf(
                "two-appearances" to
                    "slipbox.reading 1\nappearance=Light\nappearance=Dark\nreduce-motion=false\n",
                "two-flags" to
                    "slipbox.reading 1\nappearance=Light\n" +
                        "reduce-motion=false\nreduce-motion=true\n",
                "the-same-answer-twice" to
                    "slipbox.reading 1\nappearance=Dark\nappearance=Dark\nreduce-motion=true\n",
                "a-blank-line" to "slipbox.reading 1\n\nappearance=Dark\nreduce-motion=true\n",
                "an-unfinished-line" to "slipbox.reading 1\nappearance=Dark\nreduce-motion=true",
            )
        for ((name, text) in texts) {
            assertEquals(
                name,
                unusable(ReadingPreferenceFault.Malformed),
                stored(text, name).read(),
            )
        }
    }

    @Test
    fun theRecordTheStoreWritesIsTheRecordItReads() {
        for (appearance in SlipboxAppearance.entries) {
            for (reduceMotion in listOf(false, true)) {
                val file = File(folder.root, "encoded-$appearance-$reduceMotion")
                val store = ReadingPreferencesFile(file)
                val written = ReadingPreferences(appearance, reduceMotion)
                assertNull(store.write(written))
                assertEquals(
                    "slipbox.reading 1\nappearance=$appearance\nreduce-motion=$reduceMotion\n",
                    file.readText(),
                )
                assertEquals(ReadingPreferencesRecord.Stored(written), store.read())
            }
        }
    }

    @Test
    fun aWriteThatCannotBeClearedUpStillReportsTheWriteThatFailed() {
        val file = folder.newFile("preferences")
        file.writeText(RECORD)
        val obstruction = File(folder.root, "${file.name}.pending")
        assertTrue(obstruction.mkdir())
        val held = File(obstruction, "held").apply { writeText("held") }
        val store = ReadingPreferencesFile(file)

        assertEquals(
            ReadingPreferenceFault.Unwritable,
            store.write(ReadingPreferences(SlipboxAppearance.Light, reduceMotion = true)),
        )
        assertTrue("what could not be cleared up is left as it stands", held.isFile)
        assertEquals("and the record already there is untouched", RECORD, file.readText())
        assertEquals(
            ReadingPreferencesRecord.Stored(
                ReadingPreferences(SlipboxAppearance.Dark, reduceMotion = false),
            ),
            store.read(),
        )
        assertTrue("clearing up replaced nothing it did not make", obstruction.isDirectory)
    }

    private fun store(name: String = "preferences") =
        ReadingPreferencesFile(File(folder.root, name))

    private fun stored(text: String, name: String = "preferences") =
        ReadingPreferencesFile(write(name, text))

    private fun write(name: String, text: String): File =
        folder.newFile(name).apply { writeText(text) }

    private fun unusable(fault: ReadingPreferenceFault) = ReadingPreferencesRecord.Unusable(fault)

    /** A record whose metadata was observed before it grew. */
    private class StaleLength(file: File, private val stale: Long) : File(file.path) {
        override fun length(): Long = stale
    }

    private companion object {
        const val RECORD = "slipbox.reading 1\nappearance=Dark\nreduce-motion=false\n"
    }
}
