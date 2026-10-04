/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.navigation.ReadingAnchor
import io.github.b_vitamins.slipbox.navigation.ReadingReturn
import io.github.b_vitamins.slipbox.navigation.ReadingReturnAvailability
import io.github.b_vitamins.slipbox.navigation.ReadingReturnsSnapshot
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class ReadingReturnsSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun readingHistoryDoesNotCompeteWithTheRestingSearchSurface() {
        val continuing = entry("continue.org", "Long reading", ReadingAnchor(progress = 0.63f))
        val bookmarked = entry("book.org", "Bookmarked note")
        val snapshot =
            ReadingReturnsSnapshot(
                bookmarks = listOf(bookmarked),
                recents =
                    listOf(
                        continuing,
                        entry("earlier.org", "Earlier note"),
                    ),
            )
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    readingReturns = snapshot,
                )
            }
        }

        composeRule.onNodeWithText("What are you looking for?").assertIsDisplayed()
        composeRule.onNodeWithText("Surprise me").assertIsDisplayed()
        composeRule.onNodeWithText("Continue reading").assertDoesNotExist()
        composeRule.onNodeWithText("Long reading").assertDoesNotExist()
        composeRule.onNodeWithText("Bookmarks").assertDoesNotExist()
        composeRule.onNodeWithText("Bookmarked note").assertDoesNotExist()
        composeRule.onNodeWithText("continue.org").assertDoesNotExist()
        composeRule.onNodeWithText("63%").assertDoesNotExist()
        composeRule.onNodeWithText("Earlier note").assertDoesNotExist()
        composeRule.onNodeWithText("Clear recent").assertDoesNotExist()
        composeRule.onNodeWithText("Clear bookmarks").assertDoesNotExist()
        composeRule.onNodeWithText("Remove").assertDoesNotExist()
    }

    private fun entry(
        file: String,
        title: String,
        anchor: ReadingAnchor = ReadingAnchor.Start,
    ): ReadingReturn =
        ReadingReturn(
            note = BoundNote(GenerationBinding(SOURCE, GENERATION), "file:$file", null, file),
            title = title,
            anchor = anchor,
            availability = ReadingReturnAvailability.Available,
        )

    private fun ready(): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = SOURCE,
                    displayName = "owner/notes",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/notes.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(SOURCE, GENERATION),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(1, 1, 0),
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-1"
    }
}
