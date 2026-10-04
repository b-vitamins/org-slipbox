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
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.text.input.TextFieldValue
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.ContentSnippet
import io.github.b_vitamins.slipbox.engine.CorpusSearchEntity
import io.github.b_vitamins.slipbox.engine.CorpusSearchField
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.navigation.SlipboxRoute
import io.github.b_vitamins.slipbox.navigation.SlipboxSurface
import io.github.b_vitamins.slipbox.navigation.SourceGenerations
import io.github.b_vitamins.slipbox.navigation.readingRoute
import io.github.b_vitamins.slipbox.navigation.slipboxDestinations
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.ui.content.answer
import io.github.b_vitamins.slipbox.ui.content.awaitState
import io.github.b_vitamins.slipbox.ui.content.awaitTrue
import io.github.b_vitamins.slipbox.ui.content.count
import io.github.b_vitamins.slipbox.ui.content.documentViewIn
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class CorpusSearchSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun enteringTheFirstCharacterKeepsTheSearchFieldAndItsFocus() {
        var input by mutableStateOf(TextFieldValue())
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    searchInput = input,
                    searchPhase = CorpusSearchPhase.Empty("c"),
                    onSearchChange = { input = it },
                )
            }
        }

        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("c")
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).assertIsFocused()
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("ontrol")
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).assertTextEquals("control")
    }

    @Test
    fun resultsRenderAsReadableProseWithoutOrgIdsOrRawTex() {
        var input by mutableStateOf(TextFieldValue())
        var opened: CorpusSearchHit? = null
        val hits = listOf(hit(1), hit(2), hit(3))
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    searchInput = input,
                    searchPhase = readySearch(hits),
                    onSearchChange = { input = it },
                    onOpenSearchHit = { opened = it },
                )
            }
        }

        composeRule.onNodeWithText("What are you looking for?").assertIsDisplayed()
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("control")
        val view = searchView()
        view.awaitState("ready")
        view.awaitTrue(
            "all result rows rendered",
            "document.querySelectorAll('.org-search-result').length === 3",
        )

        assertEquals(3, view.count(".org-search-result"))
        assertEquals(1, view.count(".org-search-result .katex"))
        val markup = view.answer("document.querySelector('.org-search-result').innerHTML")
        assertFalse("an Org identifier reached the row", markup.contains("private-identity"))
        assertFalse("an Org link reached the row", markup.contains("[[id:"))
        assertFalse("a TeX delimiter reached the row", markup.contains("\\\\("))
        assertTrue("the matching prose is marked", view.count(".org-search-result mark") > 0)

        view.answer("document.querySelector('.org-search-result button').click()")
        composeRule.waitUntil(5_000) { opened != null }
        composeRule.runOnIdle { assertEquals(hits.first(), opened) }
    }

    @Test
    fun openingAResultAndReturningPreservesTheQuery() {
        val result = hit(1)
        val binding = GenerationBinding(SOURCE, GENERATION)
        var input by mutableStateOf(TextFieldValue())
        val destinations =
            slipboxDestinations {
                surface(SlipboxSurface.Library) { route, backStack ->
                    val library = route as SlipboxRoute.Library
                    LibraryScreen(
                        onOpenAbout = {},
                        phase = SourceLibraryPhase.Ready(ready(), 1),
                        hasSources = true,
                        searchInput = input,
                        searchPhase = readySearch(listOf(result)),
                        onSearchChange = {
                            input = it
                            backStack.rememberLibrarySearch(library, it.text)
                        },
                        onOpenSearchHit = { backStack.open(it.readingRoute(binding)) },
                    )
                }
                surface(SlipboxSurface.Reader) { _, _ -> Text("Complete note") }
                surface(SlipboxSurface.Glossary) { _, _ -> Text("Complete term") }
            }
        composeRule.setContent {
            SlipboxTheme {
                io.github.b_vitamins.slipbox.navigation.SlipboxNavigation(
                    destinations = destinations,
                    motion = SlipboxMotion(reduceMotion = true),
                    generations = SourceGenerations { GENERATION },
                )
            }
        }

        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("control")
        searchView().apply {
            awaitState("ready")
            awaitTrue(
                "the result row rendered",
                "document.querySelectorAll('.org-search-result').length === 1",
            )
            answer("document.querySelector('.org-search-result button').click()")
        }
        composeRule.onNodeWithText("Complete note").assertIsDisplayed()

        Espresso.pressBack()
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).assertTextEquals("control")
        searchView().awaitTrue(
            "the search results returned",
            "document.querySelectorAll('.org-search-result').length === 1",
        )
    }

    private fun searchView(): WebView {
        composeRule.waitUntil(5_000) {
            documentViewIn(composeRule.activity.window.decorView) != null
        }
        return checkNotNull(documentViewIn(composeRule.activity.window.decorView))
    }

    private fun readySearch(hits: List<CorpusSearchHit>): CorpusSearchPhase.Ready =
        CorpusSearchPhase.Ready(
            query = "control",
            hits = hits,
            total = hits.size.toLong(),
            queryBound = 200,
            queryTruncated = false,
            hasMore = false,
            nextPosition = null,
        )

    private fun hit(index: Int): CorpusSearchHit {
        val title = if (index == 1) "Control systems" else "Result $index"
        val excerpt =
            if (index == 1) {
                listOf(
                    ContentSegment(
                        "A [[id:private-identity][control input]] \\(u(t)\\) drives the ",
                        false,
                    ),
                    ContentSegment("system", true),
                    ContentSegment(".", false),
                )
            } else {
                listOf(ContentSegment("Readable context for result $index.", false))
            }
        return CorpusSearchHit(
            node = node(index, title),
            entity = CorpusSearchEntity.NOTE,
            matchedField = CorpusSearchField.CONTENT,
            title = ContentSnippet(listOf(ContentSegment(title, false))),
            aliases = ContentSnippet(emptyList()),
            excerpt = ContentSnippet(excerpt),
        )
    }

    private fun node(index: Int, title: String): NodeRecord =
        NodeRecord(
            nodeKey = "key-$index",
            explicitId = "id-$index",
            filePath = "note-$index.org",
            title = title,
            outlinePath = "Section/$title",
            aliases = emptyList(),
            tags = listOf("control"),
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
            stats = ReadySourceStats(3, 3, 0),
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-1"
    }
}
