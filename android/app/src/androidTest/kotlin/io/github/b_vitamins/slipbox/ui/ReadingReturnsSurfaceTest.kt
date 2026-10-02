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
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
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
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class ReadingReturnsSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun returnPathsStayCompactActionableAndHonestAboutMissingNotes() {
        var snapshot by
            mutableStateOf(
                ReadingReturnsSnapshot(
                    bookmarks = listOf(entry("book.org", "Bookmarked note")),
                    recents =
                        listOf(
                            entry(
                                "continue.org",
                                "Long reading",
                                anchor = ReadingAnchor(progress = 0.63f),
                            ),
                            entry(
                                "missing.org",
                                "Missing note",
                                availability = ReadingReturnAvailability.Missing,
                            ),
                            entry("earlier.org", "Earlier note"),
                        ),
                ),
            )
        var opened: ReadingReturn? = null
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    inventory =
                        NotesInventoryPhase.Ready(
                            notes = listOf(note()),
                            total = 1,
                            hasMore = false,
                            nextPosition = null,
                        ),
                    readingReturns = snapshot,
                    onOpenReadingReturn = { opened = it },
                    onRemoveBookmark = { removing ->
                        snapshot =
                            snapshot.copy(
                                bookmarks = snapshot.bookmarks.filterNot { it == removing },
                            )
                    },
                    onRemoveRecent = { removing ->
                        snapshot =
                            snapshot.copy(recents = snapshot.recents.filterNot { it == removing })
                    },
                    onClearBookmarks = { snapshot = snapshot.copy(bookmarks = emptyList()) },
                    onClearRecents = { snapshot = snapshot.copy(recents = emptyList()) },
                )
            }
        }

        composeRule.onNodeWithText("Continue reading").assertIsDisplayed()
        Evidence.image("reading-returns", composeRule.onRoot().captureToImage())
        composeRule.onNodeWithText("Long reading").performClick()
        composeRule.runOnIdle { assertEquals("Long reading", opened?.title) }
        composeRule.onNodeWithText("continue.org · 63%").assertIsDisplayed()

        composeRule
            .onNode(hasScrollAction())
            .performScrollToNode(hasText("Missing note"))
        composeRule.onNodeWithText("Missing note").assertIsNotEnabled().performClick()
        composeRule
            .onNodeWithText("Not present in this repository version")
            .assertIsDisplayed()
        composeRule.runOnIdle { assertEquals("Long reading", opened?.title) }

        composeRule
            .onNodeWithContentDescription("Remove Missing note from this list")
            .performClick()
        composeRule.onNodeWithText("Missing note").assertDoesNotExist()
        composeRule.onNode(hasScrollAction()).performScrollToNode(hasText("Clear bookmarks"))
        composeRule.onNodeWithText("Clear bookmarks").performClick()
        composeRule.onNodeWithText("Bookmarked note").assertDoesNotExist()
        composeRule.onNode(hasScrollAction()).performScrollToNode(hasText("Clear recent"))
        composeRule.onNodeWithText("Clear recent").performClick()
        composeRule.onNodeWithText("Continue reading").assertDoesNotExist()
        composeRule.runOnIdle { assertNull(opened?.takeIf { it.title != "Long reading" }) }
    }

    private fun entry(
        file: String,
        title: String,
        anchor: ReadingAnchor = ReadingAnchor.Start,
        availability: ReadingReturnAvailability = ReadingReturnAvailability.Available,
    ): ReadingReturn =
        ReadingReturn(
            note = BoundNote(GenerationBinding(SOURCE, GENERATION), "file:$file", null, file),
            title = title,
            anchor = anchor,
            availability = availability,
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
        const val GENERATION = "generation-1"
    }
}
