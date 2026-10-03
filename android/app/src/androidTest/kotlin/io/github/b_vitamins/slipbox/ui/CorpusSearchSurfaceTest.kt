/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performScrollToNode
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
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
@OptIn(ExperimentalTestApi::class)
class CorpusSearchSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun neutralSearchShowsExactUnicodeContextAndLoadsTheBoundedContinuation() {
        val all = (1..60).map(::hit)
        var input by mutableStateOf(TextFieldValue())
        var loads = 0
        var search by mutableStateOf<CorpusSearchPhase>(CorpusSearchPhase.Dormant)
        composeRule.setContent {
            SlipboxTheme {
                LibraryScreen(
                    onOpenAbout = {},
                    phase = SourceLibraryPhase.Ready(ready(), 1),
                    hasSources = true,
                    inventory = NotesInventoryPhase.Empty,
                    searchInput = input,
                    searchPhase = search,
                    onSearchChange = {
                        input = it
                        search =
                            CorpusSearchPhase.Ready(
                                query = it.text,
                                hits = all.take(30),
                                total = 60,
                                queryBound = 200,
                                queryTruncated = false,
                                hasMore = true,
                                nextPosition = "after-30",
                            )
                    },
                    onLoadMoreSearch = {
                        loads += 1
                        search =
                            CorpusSearchPhase.Ready(
                                query = input.text,
                                hits = all,
                                total = 60,
                                queryBound = 200,
                                queryTruncated = false,
                                hasMore = false,
                                nextPosition = null,
                            )
                    },
                )
            }
        }

        composeRule.onNodeWithText("What are you looking for?").assertIsDisplayed()
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("théorie")
        composeRule.onNodeWithText("30 of 60 matches").assertIsDisplayed()
        composeRule.onNodeWithText("Théorie of fixed points").assertIsDisplayed()
        composeRule.onNodeWithText("théorie alias").assertIsDisplayed()
        composeRule.onNodeWithText("Context \\(f(λ)=λ\\) in théorie 3.").assertIsDisplayed()
        Evidence.image("corpus-search-unicode", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithTag(CORPUS_SEARCH_LIST_TAG).performScrollToIndex(31)
        composeRule.waitUntil(5_000) { loads == 1 }
        composeRule
            .onNodeWithTag(CORPUS_SEARCH_LIST_TAG)
            .performScrollToNode(hasText("Result 60"))
        composeRule.onNodeWithText("Result 60").assertIsDisplayed()
        composeRule.runOnIdle {
            assertEquals(1, loads)
            assertEquals(60, (search as CorpusSearchPhase.Ready).hits.distinctBy { it.node.nodeKey }.size)
        }
    }

    @Test
    fun typedResultsOpenTheirCompleteSurfaceAndBackRestoresQuerySelectionAndListPlace() {
        val all = (1..60).map(::hit)
        val binding = GenerationBinding(SOURCE, GENERATION)
        var input by mutableStateOf(TextFieldValue())
        var selected by mutableStateOf<String?>(null)
        val search =
            CorpusSearchPhase.Ready(
                query = "théorie",
                hits = all,
                total = 60,
                queryBound = 200,
                queryTruncated = false,
                hasMore = false,
                nextPosition = null,
            )
        val destinations =
            slipboxDestinations {
                surface(SlipboxSurface.Library) { route, backStack ->
                    val library = route as SlipboxRoute.Library
                    LibraryScreen(
                        onOpenAbout = {},
                        phase = SourceLibraryPhase.Ready(ready(), 1),
                        hasSources = true,
                        inventory = NotesInventoryPhase.Empty,
                        searchInput = input,
                        searchPhase = search,
                        selectedSearchNodeKey = selected,
                        onSearchChange = {
                            input = it
                            backStack.rememberLibrarySearch(library, it.text)
                        },
                        onOpenSearchHit = {
                            selected = it.node.nodeKey
                            backStack.open(it.readingRoute(binding))
                        },
                    )
                }
                surface(SlipboxSurface.Reader) { route, _ ->
                    val reader = route as SlipboxRoute.Reader
                    Text("Complete note ${reader.note.nodeKey.substringAfter("key-")}")
                }
                surface(SlipboxSurface.Glossary) { route, _ ->
                    val glossary = route as SlipboxRoute.Glossary
                    Text("Complete term ${glossary.term?.substringAfter("key-")}")
                }
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

        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).performTextInput("théorie")
        composeRule
            .onNodeWithTag(CORPUS_SEARCH_LIST_TAG)
            .performScrollToNode(hasText("Result 39"))
        composeRule.onNodeWithText("Result 39").performClick()
        composeRule.onNodeWithText("Complete note 39").assertIsDisplayed()
        Espresso.pressBack()
        composeRule.onNodeWithTag(CORPUS_SEARCH_FIELD_TAG).assertTextEquals("théorie")
        composeRule.onNodeWithText("Result 39").assertIsDisplayed().assertIsSelected()
        composeRule.onNodeWithText("Result 1").assertDoesNotExist()

        composeRule.onNodeWithText("Result 40").assertIsDisplayed().performClick()
        composeRule.onNodeWithText("Complete term 40").assertIsDisplayed()
        Espresso.pressBack()
        composeRule.onNodeWithText("Result 40").assertIsDisplayed().assertIsSelected()
        composeRule.onNodeWithText("Result 1").assertDoesNotExist()
        Evidence.image("corpus-search-back-restoration", composeRule.onRoot().captureToImage())
    }

    private fun hit(index: Int): CorpusSearchHit {
        val glossary = index % 2 == 0
        val title = if (index == 1) "Théorie of fixed points" else "Result $index"
        val aliases = if (index == 2) listOf("théorie alias") else emptyList()
        val field =
            when (index) {
                1 -> CorpusSearchField.TITLE
                2 -> CorpusSearchField.ALIAS
                else -> CorpusSearchField.CONTENT
            }
        return CorpusSearchHit(
            node = node(index, title, aliases, glossary),
            entity = if (glossary) CorpusSearchEntity.GLOSSARY else CorpusSearchEntity.NOTE,
            matchedField = field,
            title =
                ContentSnippet(
                    if (index == 1) {
                        listOf(
                            ContentSegment("Théorie", true),
                            ContentSegment(" of fixed points", false),
                        )
                    } else {
                        listOf(ContentSegment(title, false))
                    },
                ),
            aliases =
                ContentSnippet(
                    if (index == 2) listOf(ContentSegment("théorie alias", true)) else emptyList(),
                ),
            excerpt =
                ContentSnippet(
                    if (index > 2) {
                        listOf(
                            ContentSegment("Context \\(f(λ)=λ\\) in ", false),
                            ContentSegment("théorie", true),
                            ContentSegment(" $index.", false),
                        )
                    } else {
                        emptyList()
                    },
                ),
        )
    }

    private fun node(
        index: Int,
        title: String,
        aliases: List<String>,
        glossary: Boolean,
    ): NodeRecord =
        NodeRecord(
            nodeKey = "key-$index",
            explicitId = "id-$index",
            filePath = "note-$index.org",
            title = title,
            outlinePath = "Section/$title",
            aliases = aliases,
            tags = emptyList(),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = glossary,
            glossaryStatus = if (glossary) "confirmed" else null,
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
                    displayName = "owner/knowledge",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/knowledge.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(SOURCE, GENERATION),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(60, 60, 0),
        )

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val GENERATION = "generation-1"
    }
}
