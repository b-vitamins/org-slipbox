/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performScrollToNode
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
    fun reachingThePageBoundaryLoadsAndShowsTheNextBoundedPage() {
        val all = (1..60).map(::note)
        var loads = 0
        var inventory by
            mutableStateOf<NotesInventoryPhase>(
                NotesInventoryPhase.Ready(all.take(50), 60, true, "after-50"),
            )
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    inventory = inventory,
                    onLoadMore = {
                        loads += 1
                        inventory = NotesInventoryPhase.Ready(all, 60, false, null)
                    },
                )
            }
        }

        composeRule
            .onNode(hasScrollAction())
            .performScrollToIndex(50)
        composeRule.waitUntil(5_000) { loads == 1 }
        composeRule
            .onNode(hasScrollAction())
            .performScrollToNode(hasText("Note 60"))
        composeRule.onNodeWithText("Note 60").assertIsDisplayed()
        composeRule.runOnIdle { assertEquals(1, loads) }
        Evidence.image("notes-inventory-continuation", composeRule.onRoot().captureToImage())
    }

    @Test
    fun initialAndContinuationFailuresRemainCompactAndRetryable() {
        var retries = 0
        var inventory by mutableStateOf<NotesInventoryPhase>(NotesInventoryPhase.Failed)
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    inventory = inventory,
                    onRetryInventory = {
                        retries += 1
                        inventory =
                            NotesInventoryPhase.Ready(
                                notes = listOf(note(1)),
                                total = 2,
                                hasMore = true,
                                nextPosition = "after-1",
                                continuationFailed = true,
                            )
                    },
                )
            }
        }

        composeRule.onNodeWithText("Notes could not be loaded.").assertIsDisplayed()
        Evidence.image("notes-inventory-error", composeRule.onRoot().captureToImage())
        composeRule.onNodeWithText("Try again").performClick()
        composeRule.onNodeWithText("Note 1").assertIsDisplayed()
        composeRule.onNodeWithText("More notes could not be loaded.").assertIsDisplayed()
        composeRule.runOnIdle { assertEquals(1, retries) }
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
            stats = ReadySourceStats(60, 60, 0),
        )

    private fun note(index: Int): NodeRecord =
        NodeRecord(
            nodeKey = "file:${index.toString().padStart(3, '0')}.org",
            explicitId = null,
            filePath = "${index.toString().padStart(3, '0')}.org",
            title = "Note $index",
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
