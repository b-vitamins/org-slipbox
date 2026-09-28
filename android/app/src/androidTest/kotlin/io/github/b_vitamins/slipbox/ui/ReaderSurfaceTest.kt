/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.ui.content.DocumentSource
import io.github.b_vitamins.slipbox.ui.content.Notes
import io.github.b_vitamins.slipbox.ui.content.answer
import io.github.b_vitamins.slipbox.ui.content.awaitMounted
import io.github.b_vitamins.slipbox.ui.content.awaitTrue
import io.github.b_vitamins.slipbox.ui.content.documentViewIn
import io.github.b_vitamins.slipbox.ui.content.number
import io.github.b_vitamins.slipbox.ui.content.text
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class ReaderSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun oneSourceBoundDocumentSuppliesTheChromeAndTheCompleteRichReadingSurface() {
        var backed = 0
        lateinit var settings: ReadingSettings
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = { backed += 1 },
                    onRetry = {},
                )
            }
        }

        composeRule.onNodeWithText("A complete note").assertExists()
        composeRule.onNodeWithText("note.org · 2 backlinks · 3 links").assertExists()
        val view = shown()
        assertEquals(1.0, view.number("document.querySelectorAll('#document table').length"), 0.0)
        assertEquals(1.0, view.number("document.querySelectorAll('#document pre code').length"), 0.0)
        assertTrue(view.number("document.querySelectorAll('#document .katex').length") >= 2)
        assertEquals(
            "The complete final paragraph.",
            view.text("document.querySelector('#document p:last-child').textContent"),
        )
        Evidence.image("reader-rich-light", composeRule.onRoot().captureToImage())

        view.answer(SELECT_LEAD)
        assertTrue(view.text("window.getSelection().toString()").contains("fixed point"))
        view.answer("window.getSelection().removeAllRanges()")

        composeRule.onNodeWithText("Appearance").performClick()
        composeRule.onNodeWithText("Dark").performClick()
        composeRule.runOnIdle {
            assertEquals(
                SlipboxAppearance.Dark,
                settings.preferences.appearance,
            )
        }
        Espresso.pressBack()
        composeRule.waitForIdle()
        view.awaitTrue(
            "the selected reader scheme reached the document",
            "document.querySelector('.org-document-host').dataset.theme === 'dark'",
        )
        Evidence.image("reader-rich-dark", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithContentDescription("Back").performClick()
        composeRule.runOnIdle { assertEquals(1, backed) }
    }

    @Test
    fun loadingMissingOverLimitAndFailedReadsAreExplicit() {
        lateinit var settings: ReadingSettings
        var retries = 0
        var phase by mutableStateOf<DocumentReaderPhase>(DocumentReaderPhase.Loading)
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = phase,
                    settings = settings,
                    onBack = {},
                    onRetry = { retries += 1 },
                )
            }
        }

        composeRule.onNodeWithText("Loading note.").assertExists()
        composeRule.runOnIdle { phase = DocumentReaderPhase.NotFound }
        composeRule
            .onNodeWithText("This note is not present in this repository version.")
            .assertExists()
        composeRule.runOnIdle { phase = DocumentReaderPhase.UnsupportedSize(1000) }
        composeRule
            .onNodeWithText(
                "This note exceeds the offline reader’s current limit of 1000 lines. " +
                    "Reduce or split it, then refresh the repository.",
            )
            .assertExists()
        Evidence.image("reader-size-limit", composeRule.onRoot().captureToImage())

        composeRule.runOnIdle { phase = DocumentReaderPhase.Failed }
        composeRule.onNodeWithText("This note could not be loaded.").assertExists()
        composeRule.onNodeWithText("Try again").performClick()
        composeRule.runOnIdle { assertEquals(1, retries) }
    }

    @Test
    fun unresolvedDocumentLinksStayInTheReaderWithContext() {
        lateinit var settings: ReadingSettings
        var linkPhase by
            mutableStateOf<ReaderLinkPhase>(ReaderLinkPhase.Missing("file:missing.org"))
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    linkPhase = linkPhase,
                )
            }
        }

        val missing =
            composeRule.onNodeWithText("No indexed target was found for “file:missing.org”.")
        missing.assertExists()
        shown()

        composeRule.runOnIdle {
            linkPhase = ReaderLinkPhase.Unsupported("javascript:alert(1)")
        }
        composeRule
            .onNodeWithText("This reader cannot open “javascript:alert(1)”.")
            .assertExists()
    }

    private fun shown(): WebView {
        composeRule.waitForIdle()
        val view =
            checkNotNull(
                composeRule.runOnIdle { documentViewIn(composeRule.activity.window.decorView) },
            ) {
                "no document view is composed"
            }
        view.awaitMounted()
        return view
    }

    private fun document(): ReaderDocument =
        ReaderDocument(
            anchor = node(),
            source =
                DocumentSource(
                    source = "0123456789abcdef0123456789abcdef",
                    generation = "generation-1",
                    id = "file:note.org",
                    org = Notes.RICH + "\n\nThe complete final paragraph.",
                ),
        )

    private fun node(): NodeRecord =
        NodeRecord(
            nodeKey = "file:note.org",
            explicitId = null,
            filePath = "note.org",
            title = "A complete note",
            outlinePath = "",
            aliases = emptyList(),
            tags = listOf("mathematics"),
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
            backlinkCount = 2,
            forwardLinkCount = 3,
        )

    private companion object {
        const val SELECT_LEAD =
            "(function () {" +
                "  const node = document.querySelector('#document p').firstChild;" +
                "  const range = document.createRange();" +
                "  range.selectNodeContents(node);" +
                "  const selection = window.getSelection();" +
                "  selection.removeAllRanges();" +
                "  selection.addRange(range);" +
                "})()"
    }
}
