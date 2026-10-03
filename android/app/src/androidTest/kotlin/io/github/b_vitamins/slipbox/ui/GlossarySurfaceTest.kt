/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.unit.dp
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.navigation.SlipboxRoute
import io.github.b_vitamins.slipbox.navigation.SlipboxSurface
import io.github.b_vitamins.slipbox.navigation.SourceGenerations
import io.github.b_vitamins.slipbox.navigation.slipboxDestinations
import io.github.b_vitamins.slipbox.ui.content.DocumentSource
import io.github.b_vitamins.slipbox.ui.content.DOCUMENT_LINK
import io.github.b_vitamins.slipbox.ui.content.answer
import io.github.b_vitamins.slipbox.ui.content.awaitMounted
import io.github.b_vitamins.slipbox.ui.content.awaitTrue
import io.github.b_vitamins.slipbox.ui.content.documentViewIn
import io.github.b_vitamins.slipbox.ui.content.number
import io.github.b_vitamins.slipbox.ui.content.text
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class GlossarySurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun reachingTheBoundaryLoadsEveryTermWithoutDuplicatingTheList() {
        val all = (1..60).map(::term)
        var loads = 0
        var opened: NodeRecord? = null
        var phase by
            mutableStateOf<GlossaryInventoryPhase>(
                GlossaryInventoryPhase.Ready(all.take(50), 60, true, "after-50"),
            )
        composeRule.setContent {
            SlipboxTheme {
                GlossaryScreen(
                    phase = phase,
                    onBack = {},
                    onActivate = {},
                    onLoadMore = {
                        loads += 1
                        phase = GlossaryInventoryPhase.Ready(all, 60, false, null)
                    },
                    onRetry = {},
                    onOpenTerm = { opened = it },
                )
            }
        }

        composeRule.onNodeWithTag(GLOSSARY_LIST_TAG).performScrollToIndex(51)
        composeRule.waitUntil(5_000) { loads == 1 }
        composeRule
            .onNodeWithTag(GLOSSARY_LIST_TAG)
            .performScrollToNode(hasText("Term 60"))
        composeRule.onNodeWithText("Term 60").assertIsDisplayed().performClick()
        composeRule.onNodeWithText("Also alias 60 · T60").assertIsDisplayed()
        composeRule.runOnIdle {
            assertEquals(1, loads)
            assertEquals(all.last(), opened)
        }
        Evidence.image("glossary-complete-inventory", composeRule.onRoot().captureToImage())
    }

    @Test
    fun backRestoresThePreviousGlossaryListPlace() {
        val terms = (1..80).map(::term)
        val binding = GenerationBinding(SOURCE, GENERATION)
        val destinations =
            slipboxDestinations {
                surface(SlipboxSurface.Library) { _, _ -> Text("Library") }
                surface(SlipboxSurface.Glossary) { route, backStack ->
                    val glossary = route as SlipboxRoute.Glossary
                    if (glossary.term == null) {
                        GlossaryScreen(
                            phase = GlossaryInventoryPhase.Ready(terms, 80, false, null),
                            onBack = { backStack.back() },
                            onActivate = {},
                            onLoadMore = {},
                            onRetry = {},
                            onOpenTerm = { backStack.open(glossary.copy(term = it.nodeKey)) },
                        )
                    } else {
                        ReadingSurface(
                            title = terms.first { it.nodeKey == glossary.term }.title,
                            leading = {
                                TextControl(label = "Back", onClick = { backStack.back() })
                            },
                        ) {
                            Text("Definition")
                        }
                    }
                }
            }
        composeRule.setContent {
            SlipboxTheme {
                io.github.b_vitamins.slipbox.navigation.SlipboxNavigation(
                    destinations = destinations,
                    motion = SlipboxMotion(reduceMotion = true),
                    generations = SourceGenerations { GENERATION },
                    restored = listOf(SlipboxRoute.Glossary(binding)),
                )
            }
        }

        composeRule
            .onNodeWithTag(GLOSSARY_LIST_TAG)
            .performScrollToNode(hasText("Term 64"))
        composeRule.onNodeWithText("Term 64").assertIsDisplayed().performClick()
        composeRule.onNodeWithText("Definition").assertIsDisplayed()
        Espresso.pressBack()
        composeRule.onNodeWithText("Term 64").assertIsDisplayed()
        composeRule.onNodeWithText("Term 1").assertDoesNotExist()
        Evidence.image("glossary-back-restoration", composeRule.onRoot().captureToImage())
    }

    @Test
    fun compactTermReadingIsCompleteAndSwitchingTermsStartsFresh() {
        lateinit var settings: ReadingSettings
        var current by mutableStateOf(document("Fixed point", "invariant point", "first"))
        val compact = VisualCase("glossary-compact", 320.dp, 640.dp, false, 1f)
        composeRule.setContent {
            compact.Overridden {
                settings = remember { ReadingSettings(MemoryStore()) }
                SlipboxTheme(appearance = settings.preferences.appearance) {
                    ReaderScreen(
                        phase = DocumentReaderPhase.Ready(current),
                        settings = settings,
                        kind = ReaderSurfaceKind.GlossaryTerm,
                        onBack = {},
                        onRetry = {},
                    )
                }
            }
        }

        composeRule.onNodeWithText("Fixed point").assertIsDisplayed()
        composeRule.onNodeWithContentDescription("Bookmark").assertDoesNotExist()
        val first = shown()
        assertTrue(first.number("document.querySelectorAll('#document .katex').length") >= 1)
        assertEquals(
            1.0,
            first.number("document.querySelectorAll('$DOCUMENT_LINK').length"),
            0.0,
        )
        assertEquals(
            "The first definition ends here.",
            first.text("document.querySelector('#document p:last-child').textContent"),
        )
        Evidence.image("glossary-term-reading", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithText("Details").performClick()
        composeRule.onNodeWithText("Identity").assertIsDisplayed()
        composeRule.onNodeWithText("invariant point").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("heading:terms.org:first")
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("owner/knowledge").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText(GENERATION).performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("0123456789abcdef").performScrollTo().assertIsDisplayed()
        Evidence.image("glossary-term-details", composeRule.onRoot().captureToImage())
        Espresso.pressBack()

        first.answer("window.scrollTo(0, document.documentElement.scrollHeight)")
        first.awaitTrue("the first definition scrolled", "window.scrollY > 0")
        composeRule.runOnIdle {
            current = document("Uniform continuity", "global continuity", "second")
        }
        val second = shown()
        second.awaitTrue(
            "the new term replaced the old content at its beginning",
            "window.scrollY === 0 && " +
                "document.querySelector('#document p:last-child').textContent === " +
                "'The second definition ends here.'",
        )
        composeRule.onNodeWithText("Uniform continuity").assertIsDisplayed()
        assertEquals(0.0, second.number("window.scrollY"), 0.0)
        Evidence.image("glossary-term-switch", composeRule.onRoot().captureToImage())
    }

    private fun shown(): WebView {
        composeRule.waitForIdle()
        return checkNotNull(
            composeRule.runOnIdle { documentViewIn(composeRule.activity.window.decorView) },
        ) { "no glossary document is composed" }.also { it.awaitMounted() }
    }

    private fun document(title: String, alias: String, identity: String): ReaderDocument {
        val term =
            term(1).copy(
                nodeKey = "heading:terms.org:$identity",
                title = title,
                outlinePath = "Analysis/$title",
                aliases = listOf(alias),
            )
        val paragraphs = (1..40).joinToString("\n\n") { "Reading paragraph $it." }
        return ReaderDocument(
            anchor = term,
            source =
                DocumentSource(
                    source = SOURCE,
                    generation = GENERATION,
                    id = term.nodeKey,
                    filePath = term.filePath,
                    org =
                        "** $title\n" +
                            "A canonical definition with \\(f(x)=x\\).\n\n" +
                            "[[id:contraction][Contraction mapping]]\n\n" +
                            "$paragraphs\n\n" +
                            "The $identity definition ends here.",
                    baseLevel = 2,
                ),
            sourceName = "owner/knowledge",
            revision = "0123456789abcdef",
        )
    }

    private fun term(index: Int): NodeRecord =
        NodeRecord(
            nodeKey = "heading:terms.org:$index",
            explicitId = "term-$index",
            filePath = "terms.org",
            title = "Term $index",
            outlinePath = "Glossary/Term $index",
            aliases = listOf("alias $index", "T$index"),
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = true,
            glossaryStatus = "confirmed",
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 2,
            line = index.toLong(),
            kind = NodeKind.HEADING,
            fileMtimeNs = 0,
            backlinkCount = 0,
            forwardLinkCount = 1,
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-1"
    }
}
