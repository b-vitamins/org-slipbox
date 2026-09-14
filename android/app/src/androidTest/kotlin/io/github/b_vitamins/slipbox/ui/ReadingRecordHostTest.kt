/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.content.Context
import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesFile
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.settings.rememberReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class ReadingRecordHostTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val directories =
        listOf(ONE, OTHER).map { name -> File(context.noBackupFilesDir, name) }

    @Before
    fun emptyTheOwnedDirectories() {
        discard()
    }

    @After
    fun discardTheOwnedDirectories() {
        discard()
    }

    private fun discard() {
        for (directory in directories) {
            directory.listFiles()?.forEach { file -> file.delete() }
            directory.delete()
        }
    }

    @Test
    fun theRecordIsOpenedOncePerContextRatherThanOncePerRedisplay() {
        val one = RecordingContext(context, directories.first())
        val other = RecordingContext(context, directories.last())
        var host by mutableStateOf<Context>(one)
        var redisplays by mutableIntStateOf(0)
        val seen = mutableListOf<Seen>()
        composeRule.setContent {
            CompositionLocalProvider(LocalContext provides host) {
                val settings = rememberReadingSettings()
                val redisplay = redisplays
                SideEffect { seen += Seen(redisplay, settings) }
            }
        }
        composeRule.waitForIdle()

        for (redisplay in 1..REDISPLAYS) {
            composeRule.runOnIdle { redisplays = redisplay }
            composeRule.waitForIdle()
        }
        assertEquals(
            "every redisplay was displayed",
            (0..REDISPLAYS).toList(),
            seen.map { it.redisplay }.distinct(),
        )
        assertEquals("the record's directory was asked for once", 1, one.asked)
        assertEquals(
            "and the store was the one remembered throughout",
            1,
            seen.map { it.settings }.distinct().size,
        )

        composeRule.runOnIdle { host = other }
        composeRule.waitForIdle()
        assertEquals("a context that answers elsewhere opens its own", 1, other.asked)
        assertEquals("and the first is not asked again", 1, one.asked)
        val stores = seen.map { it.settings }.distinct()
        assertEquals("the store is rebuilt for it", 2, stores.size)

        composeRule.runOnIdle { stores.last().select(SlipboxAppearance.Dark) }
        composeRule.waitForIdle()
        assertTrue("a choice is recorded where the context answered", recorded(directories.last()))
        assertFalse("and nowhere else", recorded(directories.first()))
        Evidence.record(
            "record-host",
            Record()
                .count("redisplays", REDISPLAYS)
                .count("firstContextAsks", one.asked)
                .count("otherContextAsks", other.asked)
                .count("storesBuilt", stores.size),
        )
    }

    private fun recorded(directory: File): Boolean =
        File(directory, ReadingPreferencesFile.FILE_NAME).isFile

    private data class Seen(val redisplay: Int, val settings: ReadingSettings)

    private companion object {
        const val ONE = "record-host-one"
        const val OTHER = "record-host-other"

        const val REDISPLAYS = 3
    }
}
