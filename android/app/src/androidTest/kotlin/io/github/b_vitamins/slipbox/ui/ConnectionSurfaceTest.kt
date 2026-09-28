/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
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
class ConnectionSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun publicAndGithubEntryAreContextualAndCaptured() {
        composeRule.setContent {
            SlipboxTheme {
                ConnectionScreen(catalogRevision = 0, onBack = {}, onReady = { _, _ -> })
            }
        }

        composeRule.onNodeWithText("Repository URL").assertIsDisplayed()
        composeRule.onNodeWithText("Branch").assertIsDisplayed()
        composeRule.onNodeWithText("Notes folder (optional)").assertIsDisplayed()
        Evidence.image("connection-public", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithText("GitHub").performClick()
        composeRule.onNodeWithText("Authorize GitHub").assertIsDisplayed()
        Evidence.image("connection-github", composeRule.onRoot().captureToImage())
    }

    @Test
    fun verifiedSourceIdentityIsVisibleInTheLibrary() {
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    phase = SourceLibraryPhase.Ready(ready(), 3),
                    onConnect = {},
                    onOpenAbout = {},
                )
            }
        }

        composeRule.onNodeWithText("owner/notes").assertIsDisplayed()
        composeRule.onNodeWithText("4 files · 9 indexed nodes · revision 0123456789").assertIsDisplayed()
        Evidence.image("library-connected", composeRule.onRoot().captureToImage())
    }

    private fun ready(): ReadySource {
        val source =
            RefreshSource(
                id = SOURCE,
                displayName = "owner/notes",
                provider = RefreshProvider.GITHUB,
                visibility = RefreshVisibility.PRIVATE,
                providerRepositoryId = "9001",
                account = "42",
                remote = "https://github.com/owner/notes.git",
                branch = "main",
                notesFolder = "notes",
                credential = "slipbox.source.$SOURCE",
            )
        return ReadySource(
            source = source,
            binding = GenerationBinding(SOURCE, "generation-8"),
            revision = "0123456789012345678901234567890123456789",
            contentRoot = "/private/source",
            database = "/private/index/slipbox.db",
            stats = ReadySourceStats(4, 9, 5),
        )
    }

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
    }
}
