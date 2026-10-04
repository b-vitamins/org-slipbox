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
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class NotesInventorySurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun theRestingLibraryDoesNotDumpTheRepositoryInventoryOrDiagnostics() {
        var loads = 0
        var retries = 0
        val note = note()
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    inventory = NotesInventoryPhase.Ready(listOf(note), 924, true, "after-1"),
                    onLoadMore = { loads += 1 },
                    onRetryInventory = { retries += 1 },
                )
            }
        }

        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).assertExists()
        composeRule.onNodeWithText("What are you looking for?").assertIsDisplayed()
        composeRule.onNodeWithText("Surprise me").assertIsDisplayed()
        composeRule.onNodeWithText("Glossary").assertIsDisplayed()
        composeRule.onNodeWithText(note.title).assertDoesNotExist()
        composeRule.onNodeWithText(note.filePath).assertDoesNotExist()
        composeRule.onNodeWithText("owner/notes").assertDoesNotExist()
        composeRule.onNodeWithText("0123456789abcdef").assertDoesNotExist()
        composeRule.onNodeWithText("924 files").assertDoesNotExist()
        composeRule.runOnIdle {
            assertEquals(0, loads)
            assertEquals(0, retries)
        }
    }

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
            binding = GenerationBinding(SOURCE, "generation-1"),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(924, 1_587, 17_833),
        )

    private fun note(): NodeRecord =
        NodeRecord(
            nodeKey = "file:inventory.org",
            explicitId = null,
            filePath = "inventory.org",
            title = "Inventory note",
            outlinePath = "",
            aliases = emptyList(),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = false,
            glossaryStatus = null,
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 0,
            line = 1,
            kind = NodeKind.FILE,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
    }
}
