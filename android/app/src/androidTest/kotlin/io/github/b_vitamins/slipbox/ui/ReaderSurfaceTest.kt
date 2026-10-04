/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.graphics.Color
import android.os.Build
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.engine.AnchorExplorationRecord
import io.github.b_vitamins.slipbox.engine.BridgeEvidenceRecord
import io.github.b_vitamins.slipbox.engine.DirectedRelationDirection
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.ExplorationEntry
import io.github.b_vitamins.slipbox.engine.ExplorationExplanation
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.ExplorationSection
import io.github.b_vitamins.slipbox.engine.ExplorationSectionKind
import io.github.b_vitamins.slipbox.engine.ExploreResult
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.NotePlace
import io.github.b_vitamins.slipbox.engine.NotePlaceNeighbor
import io.github.b_vitamins.slipbox.engine.UnlinkedReferenceRecord
import io.github.b_vitamins.slipbox.engine.UnlinkedReferencesResult
import io.github.b_vitamins.slipbox.ui.content.DOCUMENT_LINK
import io.github.b_vitamins.slipbox.ui.content.DocumentAssetResolver
import io.github.b_vitamins.slipbox.ui.content.DocumentFocusRequest
import io.github.b_vitamins.slipbox.ui.content.DocumentIntent
import io.github.b_vitamins.slipbox.ui.content.DocumentSource
import io.github.b_vitamins.slipbox.ui.content.Notes
import io.github.b_vitamins.slipbox.ui.content.PRESS_LINK
import io.github.b_vitamins.slipbox.ui.content.answer
import io.github.b_vitamins.slipbox.ui.content.awaitMounted
import io.github.b_vitamins.slipbox.ui.content.awaitTrue
import io.github.b_vitamins.slipbox.ui.content.documentViewIn
import io.github.b_vitamins.slipbox.ui.content.documentViewsIn
import io.github.b_vitamins.slipbox.ui.content.number
import io.github.b_vitamins.slipbox.ui.content.pngAsset
import io.github.b_vitamins.slipbox.ui.content.text
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
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
class ReaderSurfaceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun readingChromeYieldsToDownwardReadingAndReturnsOnUpwardScroll() {
        lateinit var settings: ReadingSettings
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                )
            }
        }
        val view = shown()
        val back =
            SemanticsMatcher.expectValue(
                SemanticsProperties.ContentDescription,
                listOf("Back"),
            )

        view.answer("window.scrollTo(0, document.scrollingElement.scrollHeight)")
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodes(back).fetchSemanticsNodes().isEmpty()
        }

        view.answer("window.scrollTo(0, 0)")
        composeRule.waitUntil(timeoutMillis = 5_000) {
            composeRule.onAllNodes(back).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Test
    fun oneSourceBoundDocumentSuppliesTheChromeAndTheCompleteRichReadingSurface() {
        var backed = 0
        var neighbor: NotePlaceNeighbor? = null
        lateinit var settings: ReadingSettings
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = { backed += 1 },
                    onRetry = {},
                    onOpenFilingNeighbor = { neighbor = it },
                )
            }
        }

        composeRule.onNodeWithText("A complete note").assertDoesNotExist()
        composeRule.onNodeWithText("note.org").assertDoesNotExist()
        composeRule.onNodeWithText("Relations").assertDoesNotExist()
        composeRule.onNodeWithText("Explore").assertDoesNotExist()
        val view = shown()
        assertEquals(
            "A complete note",
            view.text("document.querySelector('#document-title').textContent"),
        )
        assertEquals(1.0, view.number("document.querySelectorAll('#document table').length"), 0.0)
        assertEquals(1.0, view.number("document.querySelectorAll('#document pre code').length"), 0.0)
        assertTrue(view.number("document.querySelectorAll('#document .katex').length") >= 2)
        assertEquals(
            "The complete final paragraph.",
            view.text("document.querySelector('#document p:last-child').textContent"),
        )
        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Outline").assertIsDisplayed()
        composeRule.onNodeWithText("Filed 2 of 3").assertIsDisplayed()
        composeRule.onNodeWithText("Notes").assertIsDisplayed()
        composeRule.onNodeWithText("note.org").assertIsDisplayed()
        composeRule.onNodeWithText("0123456789abcdef").assertIsDisplayed()
        composeRule.runOnIdle {
            assertFalse("the covered document leaves the accessibility tree", view.isImportantForAccessibility)
        }
        composeRule.regression("reader-note-details")

        Espresso.pressBack()
        composeRule.onNodeWithText("Note details").assertDoesNotExist()
        composeRule.runOnIdle {
            assertTrue("the uncovered document returns to accessibility", view.isImportantForAccessibility)
        }
        composeRule.regression("reader-rich-light")

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("The table it settles into").performClick()
        composeRule.onNodeWithText("Note details").assertDoesNotExist()
        composeRule.runOnIdle {
            assertTrue("the uncovered document returns to accessibility", view.isImportantForAccessibility)
        }
        composeRule.regression("reader-outline-target")
        view.awaitTrue(
            "the outline reached the rendered heading",
            "(() => {" +
                "  const heading = document.querySelector('[aria-current=location]');" +
                "  if (!heading || heading.textContent.trim() !== 'The table it settles into') return false;" +
                "  const bounds = heading.getBoundingClientRect();" +
                "  return bounds.bottom > 0 && bounds.top < window.innerHeight;" +
                "})()",
        )
        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Earlier note").performClick()
        composeRule.runOnIdle {
            assertEquals(NotePlaceNeighbor("file:earlier.org", "Earlier note"), neighbor)
        }

        view.answer(SELECT_LEAD)
        assertTrue(view.text("window.getSelection().toString()").contains("fixed point"))
        view.answer("window.getSelection().removeAllRanges()")

        composeRule.onNodeWithContentDescription("Info").performClick()
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
        composeRule.regression("reader-rich-dark")

        composeRule.onNodeWithContentDescription("Back").performClick()
        composeRule.runOnIdle { assertEquals(1, backed) }
    }

    @Test
    fun bookmarkStateIsAnExplicitReaderActionWithoutObscuringThePage() {
        lateinit var settings: ReadingSettings
        var bookmarked by mutableStateOf(false)
        var toggles = 0
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    bookmarked = bookmarked,
                    onToggleBookmark = {
                        toggles += 1
                        bookmarked = !bookmarked
                    },
                )
            }
        }

        composeRule.onNodeWithContentDescription("Bookmark").performClick()
        composeRule.onNodeWithContentDescription("Remove bookmark").assertIsDisplayed()
        shown()
        Evidence.image("reader-bookmarked", composeRule.onRoot().captureToImage())
        composeRule.onNodeWithContentDescription("Remove bookmark").performClick()
        composeRule.onNodeWithContentDescription("Bookmark").assertIsDisplayed()
        composeRule.runOnIdle { assertEquals(2, toggles) }
    }

    @Test
    fun filingControlsExistOnlyAtRealBoundariesAndAnOnlyNoteHasNoProgressStrip() {
        lateinit var settings: ReadingSettings
        var readerDocument by
            mutableStateOf(
                document().copy(
                    place =
                        NotePlace(
                            ordinal = 1,
                            total = 2,
                            later = NotePlaceNeighbor("file:later.org", "Later note"),
                        ),
                ),
            )
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(readerDocument),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                )
            }
        }

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Filed 1 of 2").assertIsDisplayed()
        composeRule.onNodeWithText("Later note").assertIsDisplayed()
        composeRule.onNodeWithText("Earlier note").assertDoesNotExist()
        Espresso.pressBack()
        composeRule.runOnIdle {
            readerDocument = readerDocument.copy(place = NotePlace(ordinal = 1, total = 1))
        }
        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Filed 1 of 1").assertDoesNotExist()
        composeRule.onNodeWithText("Earlier note").assertDoesNotExist()
        composeRule.onNodeWithText("Later note").assertDoesNotExist()
    }

    @Test
    fun anAddressedHeadingOpensAtItsMatchingRenderedOutlineEntry() {
        lateinit var settings: ReadingSettings
        val source = document()
        val addressed = source.outline.last()
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase =
                        DocumentReaderPhase.Ready(
                            source.copy(addressedAnchor = addressed),
                        ),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                )
            }
        }

        shown().awaitTrue(
            "the addressed source heading reached the same rendered heading",
            "document.querySelector('[aria-current=location]')?.textContent.trim() === " +
                "'The table it settles into'",
        )
    }

    @Test
    fun relationsNameEveryDirectionAndExposeExplicitContinuationAndPreview() {
        lateinit var settings: ReadingSettings
        var activations = 0
        var continuations = 0
        var previewed: DirectedRelationRecord? = null
        val incoming = relation("Incoming note", DirectedRelationDirection.INCOMING)
        val outgoing = relation("Outgoing note", DirectedRelationDirection.OUTGOING)
        val bidirectional = relation("Mutual note", DirectedRelationDirection.BIDIRECTIONAL)
        val relations =
            DirectedRelationsPhase.Ready(
                relations = listOf(incoming, outgoing, bidirectional),
                total = 5,
                incomingTotal = 3,
                outgoingTotal = 3,
                hasMore = true,
                nextPosition = "after-mutual",
            )
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    relationsPhase = relations,
                    onOpenRelations = { activations += 1 },
                    onLoadMoreRelations = { continuations += 1 },
                    onPreviewRelation = { previewed = it },
                )
            }
        }

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Relations").performClick()
        composeRule.onNodeWithText("5 related notes").assertIsDisplayed()
        composeRule.onNodeWithText("3 incoming · 3 outgoing").assertIsDisplayed()
        composeRule.onNodeWithText("Links to this note").assertExists()
        composeRule.onNodeWithText("Linked from this note").assertExists()
        composeRule.onNodeWithText("Links both ways").assertExists()
        composeRule.runOnIdle { assertEquals(1, activations) }
        Evidence.image("reader-directed-relations", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithText("Show more relations").performScrollTo().performClick()
        composeRule.onNodeWithText("Incoming note").performScrollTo().performClick()
        composeRule.runOnIdle {
            assertEquals(1, continuations)
            assertEquals(incoming, previewed)
        }
        composeRule.onNodeWithText("5 related notes").assertDoesNotExist()
    }

    @Test
    fun relatedNotesAndGroupedMentionsRevealOnlyWhenAsked() {
        lateinit var settings: ReadingSettings
        var relatedReveals = 0
        var mentionReveals = 0
        val direct = relation("Direct duplicate", DirectedRelationDirection.INCOMING)
        val latent = relation("Latent connection", DirectedRelationDirection.OUTGOING).note
        val related =
            RelatedDiscoveryPhase.Ready(
                ExploreResult(
                    lens = ExplorationLens.BRIDGES,
                    sections =
                        listOf(
                            ExplorationSection(
                                kind = ExplorationSectionKind.BRIDGE_CANDIDATES,
                                entries =
                                    listOf(
                                        bridgeCandidate(direct.note, "Bridge note"),
                                        bridgeCandidate(latent, "Bridge note"),
                                    ),
                            ),
                        ),
                ),
            )
        val source =
            node().copy(
                nodeKey = "file:source.org",
                filePath = "source.org",
                title = "A source note",
            )
        val mentions =
            MentionDiscoveryPhase.Ready(
                UnlinkedReferencesResult(
                    listOf(
                        mention(source, 8, "The idea returns to A complete note here."),
                        mention(source, 13, "A complete note also appears in this conclusion."),
                    ),
                ),
            )
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    relationsPhase =
                        DirectedRelationsPhase.Ready(
                            relations = listOf(direct),
                            total = 1,
                            incomingTotal = 1,
                            outgoingTotal = 0,
                            hasMore = false,
                            nextPosition = null,
                        ),
                    relatedPhase = related,
                    mentionPhase = mentions,
                    onRevealRelated = { relatedReveals += 1 },
                    onRevealMentions = { mentionReveals += 1 },
                )
            }
        }

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Relations").performClick()
        composeRule.onNodeWithText("Latent connection").assertDoesNotExist()
        composeRule.runOnIdle {
            assertEquals(0, relatedReveals)
            assertEquals(0, mentionReveals)
        }

        composeRule.onNodeWithText("Related notes").performScrollTo().performClick()
        composeRule.onNodeWithText("Latent connection").assertIsDisplayed()
        composeRule.onNodeWithText("Direct duplicate").assertIsDisplayed()
        composeRule.onNodeWithText("Unlinked mentions").performScrollTo().performClick()
        composeRule.onNodeWithText("A source note").performScrollTo().assertIsDisplayed()
        composeRule.onNodeWithText("2 mentions").performScrollTo().assertIsDisplayed()
        composeRule.runOnIdle {
            assertEquals(1, relatedReveals)
            assertEquals(1, mentionReveals)
        }
        Evidence.image("reader-related-and-mentions", composeRule.onRoot().captureToImage())
    }

    @Test
    fun explorationWaitsForALensAndPreservesUnresolvedEvidenceAndTargets() {
        lateinit var settings: ReadingSettings
        var phase by mutableStateOf<ReaderExplorationPhase>(ReaderExplorationPhase.AwaitingLens)
        val selected = mutableListOf<ExplorationLens>()
        var previewed: NodeRecord? = null
        var opened: NodeRecord? = null
        val unfinished = explorationNode("Finish the migration", "unfinished")
        val quiet = explorationNode("A quiet connection", "quiet")
        val result = unresolvedExploration(unfinished, quiet)
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    explorationPhase = phase,
                    onSelectExplorationLens = { lens ->
                        selected += lens
                        phase = ReaderExplorationPhase.Ready(result)
                    },
                    onPreviewExploration = { previewed = it },
                    onOpenExploration = { opened = it },
                )
            }
        }

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Explore").performClick()
        composeRule.onNodeWithText("Choose a lens.").assertIsDisplayed()
        composeRule.runOnIdle { assertTrue(selected.isEmpty()) }

        composeRule.onNodeWithText("Unresolved").performScrollTo().performClick()
        composeRule.onNodeWithText("Unresolved tasks").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText("Task state · TODO · References · cite:unfinished")
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Weakly integrated notes").performScrollTo().assertIsDisplayed()
        composeRule
            .onNodeWithText(
                "1 structural link · References · cite:quiet · Via · Bridge note",
            )
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.runOnIdle { assertEquals(listOf(ExplorationLens.UNRESOLVED), selected) }
        Evidence.image("reader-exploration-unresolved", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithText("Finish the migration").performScrollTo().performClick()
        composeRule.runOnIdle { assertEquals(unfinished, previewed) }
        composeRule.onNodeWithText("Unresolved tasks").assertDoesNotExist()

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Explore").performClick()
        composeRule.onAllNodesWithText("Open")[1].performScrollTo().performClick()
        composeRule.runOnIdle { assertEquals(quiet, opened) }
    }

    @Test
    fun explorationNamesFailureRefreshAndThePerSectionQueryBound() {
        lateinit var settings: ReadingSettings
        var phase by mutableStateOf<ReaderExplorationPhase>(ReaderExplorationPhase.AwaitingLens)
        var refreshes = 0
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    explorationPhase = phase,
                    onSelectExplorationLens = { lens ->
                        phase = ReaderExplorationPhase.Failed(lens)
                    },
                    onRefreshExploration = { refreshes += 1 },
                )
            }
        }

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Explore").performClick()
        composeRule.onNodeWithText("Bridges").performScrollTo().performClick()
        composeRule
            .onNodeWithText("Bridges could not be loaded.")
            .performScrollTo()
            .assertIsDisplayed()
        composeRule.onNodeWithText("Refresh").performScrollTo().performClick()
        composeRule.runOnIdle {
            assertEquals(1, refreshes)
            phase = boundedBridgeExploration()
        }

        composeRule
            .onNodeWithText("A section reached the 50-result query bound; more may exist.")
            .performScrollTo()
            .assertIsDisplayed()
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
        composeRule.accessibleRegression("reader-size-limit")

        composeRule.runOnIdle { phase = DocumentReaderPhase.Failed }
        composeRule.onNodeWithText("This note could not be loaded.").assertExists()
        composeRule.onNodeWithText("Try again").performClick()
        composeRule.runOnIdle { assertEquals(1, retries) }
    }

    @Test
    fun aRepositoryImageSettlesIntoTheReadingMeasureInLightAndDark() {
        lateinit var settings: ReadingSettings
        val illustrated =
            document().let { document ->
                document.copy(
                    source =
                        document.source.copy(
                            org =
                                "The source keeps the diagram beside the note, available offline.\n\n" +
                                    "[[file:../assets/field-map.png]]\n\n" +
                                    "The reading path continues without leaving the page.",
                        ),
                )
            }
        val resolver =
            DocumentAssetResolver { binding, target ->
                check(binding == illustrated.source.binding)
                check(target == "file:../assets/field-map.png")
                pngAsset(360, Color.rgb(76, 111, 132))
            }
        composeRule.setContent {
            settings = remember { ReadingSettings(MemoryStore()) }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(illustrated),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    resolveAsset = resolver,
                )
            }
        }

        val view = shown()
        view.awaitTrue(
            "the repository image decoded inside the reading column",
            "document.querySelector('img.org-image__content')?.complete === true && " +
                "document.querySelector('img.org-image__content')?.naturalWidth === 360",
        )
        waitForImagePaint(view, "light")
        assertEquals(
            "the image fills no more than the reading measure",
            288.0,
            view.number(
                "document.querySelector('img.org-image__content').getBoundingClientRect().width",
            ),
            0.0,
        )
        assertEquals(
            "field-map.png",
            view.text("document.querySelector('img.org-image__content').alt"),
        )
        composeRule.waitForIdle()
        Evidence.image("reader-repository-image-light", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithContentDescription("Info").performClick()
        composeRule.onNodeWithText("Appearance").performClick()
        composeRule.onNodeWithText("Dark").performClick()
        Espresso.pressBack()
        composeRule.waitForIdle()
        view.awaitTrue(
            "the repository image retained the selected dark reading surface",
            "document.querySelector('.org-document-host').dataset.theme === 'dark'",
        )
        waitForImagePaint(view, "dark")
        composeRule.waitForIdle()
        Evidence.image("reader-repository-image-dark", composeRule.onRoot().captureToImage())
    }

    private fun waitForImagePaint(view: WebView, marker: String) {
        view.answer(
            "(function () {" +
                "  const image = document.querySelector('img.org-image__content');" +
                "  image.decode().then(function () {" +
                "    requestAnimationFrame(function () { requestAnimationFrame(function () {" +
                "      document.documentElement.dataset.imagePaint = '$marker';" +
                "    }); });" +
                "  });" +
                "})();",
        )
        view.awaitTrue(
            "the repository image reached the $marker painted frame",
            "document.documentElement.dataset.imagePaint === '$marker'",
        )
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

        val missing = composeRule.onNodeWithText("Linked note unavailable")
        missing.assertExists()
        shown()

        composeRule.runOnIdle {
            linkPhase = ReaderLinkPhase.Unsupported("javascript:alert(1)")
        }
        composeRule
            .onNodeWithText("This link can’t be opened")
            .assertExists()

        composeRule.runOnIdle {
            linkPhase = ReaderLinkPhase.AssetMissing("field-notes.txt")
        }
        composeRule
            .onNodeWithText("Attachment unavailable")
            .assertExists()

        composeRule.runOnIdle {
            linkPhase = ReaderLinkPhase.AssetOversized("atlas.png", 32L * 1024L * 1024L)
        }
        composeRule
            .onNodeWithText("This attachment is too large")
            .assertExists()
    }

    @Test
    fun noteAndGlossaryPreviewsFitTheWindowRenderMathAndExposeExplicitActions() {
        lateinit var settings: ReadingSettings
        var phase by mutableStateOf<ReaderPreviewPhase>(
            ReaderPreviewPhase.Ready(preview(glossary = true, shortened = true)),
        )
        var focusRequest by mutableStateOf<DocumentFocusRequest?>(null)
        var opened = 0
        var dismissed = 0
        composeRule.setContent {
            settings =
                remember {
                    ReadingSettings(
                        MemoryStore(
                            ReadingPreferencesRecord.Stored(
                                ReadingPreferences(reduceMotion = true),
                            ),
                        ),
                    )
                }
            SlipboxTheme(appearance = settings.preferences.appearance) {
                ReaderScreen(
                    phase = DocumentReaderPhase.Ready(document()),
                    settings = settings,
                    onBack = {},
                    onRetry = {},
                    previewPhase = phase,
                    focusRequest = focusRequest,
                    onIntent = { intent ->
                        if (intent is DocumentIntent.Glance) {
                            phase = ReaderPreviewPhase.Ready(preview(origin = intent.origin))
                        }
                    },
                    onDismissPreview = {
                        val request = (phase as? ReaderPreviewPhase.Ready)?.preview?.request
                        dismissed += 1
                        phase = ReaderPreviewPhase.Hidden
                        focusRequest = request?.let { DocumentFocusRequest(it.origin) }
                    },
                    onOpenPreview = {
                        opened += 1
                        phase = ReaderPreviewPhase.Hidden
                    },
                )
            }
        }

        val pane =
            composeRule.onNode(
                SemanticsMatcher.expectValue(
                    SemanticsProperties.PaneTitle,
                    "Glossary term",
                ),
                useUnmergedTree = true,
            )
        pane.assertIsDisplayed()
        composeRule.onNodeWithText("Derivative").assertIsDisplayed()
        composeRule.onNodeWithText("Preview shortened. Open the note to read the complete target.")
            .assertIsDisplayed()
        val root = composeRule.onRoot().bounds()
        val sheet = pane.bounds()
        assertTrue(
            "the preview stays inside the window: $sheet of $root",
            sheet.left >= root.left &&
                sheet.top >= root.top &&
                sheet.right <= root.right &&
                sheet.bottom <= root.bottom,
        )

        val previewView = previewView()
        assertTrue(previewView.number("document.querySelectorAll('#document .katex').length") >= 1)
        composeRule.mainClock.advanceTimeBy(1_000)
        composeRule.waitForIdle()
        Evidence.image("reader-glossary-preview", composeRule.onRoot().captureToImage())

        composeRule.runOnIdle { settings.select(SlipboxAppearance.Dark) }
        composeRule.waitForIdle()
        previewView.awaitTrue(
            "the preview followed the reader into dark appearance",
            "document.querySelector('.org-document-host').dataset.theme === 'dark'",
        )
        Evidence.image("reader-glossary-preview-dark", composeRule.onRoot().captureToImage())

        composeRule.onNodeWithText("Open term").performClick()
        composeRule.runOnIdle { assertEquals(1, opened) }
        composeRule.onNodeWithText("Derivative").assertDoesNotExist()

        val main = shown()
        main.answer("window.scrollTo(0, 80)")
        val before = main.number("window.scrollY")
        main.answer(PRESS_LINK)
        composeRule.onNodeWithText("Target note").assertIsDisplayed()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.runOnIdle { assertEquals(1, dismissed) }
        composeRule.onNodeWithText("Target note").assertDoesNotExist()
        main.awaitTrue(
            "dismissing the sheet restored its exact link",
            "document.activeElement === document.querySelector('$DOCUMENT_LINK')",
        )
        assertEquals("focus kept the reading place", before, main.number("window.scrollY"), 0.0)
        main.answer(SELECT_LEAD)
        assertTrue(main.text("window.getSelection().toString()").contains("fixed point"))
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

    private fun previewView(): WebView {
        composeRule.waitForIdle()
        val views =
            composeRule.runOnIdle {
                documentViewsIn(composeRule.activity.window.decorView)
            }
        assertEquals("the reader and preview each own one document", 2, views.size)
        return views.last().also { it.awaitMounted() }
    }

    private fun document(): ReaderDocument =
        ReaderDocument(
            anchor = node(),
            source =
                DocumentSource(
                    source = "0123456789abcdef0123456789abcdef",
                    generation = "generation-1",
                    id = "file:note.org",
                    filePath = "notes/note.org",
                    org = Notes.RICH + "\n\nThe complete final paragraph.",
                ),
            outline =
                listOf(
                    node().copy(
                        nodeKey = "heading:note.org:6",
                        title = "The measure",
                        outlinePath = "The measure",
                        level = 1,
                        line = 6,
                        kind = NodeKind.HEADING,
                    ),
                    node().copy(
                        nodeKey = "heading:note.org:14",
                        title = "The table it settles into",
                        outlinePath = "The measure / The table it settles into",
                        level = 2,
                        line = 14,
                        kind = NodeKind.HEADING,
                    ),
                ),
            place =
                NotePlace(
                    ordinal = 2,
                    total = 3,
                    earlier = NotePlaceNeighbor("file:earlier.org", "Earlier note"),
                    later = NotePlaceNeighbor("file:later.org", "Later note"),
                ),
            sourceName = "Notes",
            revision = "0123456789abcdef",
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

    private fun relation(
        title: String,
        direction: DirectedRelationDirection,
    ): DirectedRelationRecord =
        DirectedRelationRecord(
            note =
                node().copy(
                    nodeKey = "file:${title.lowercase().replace(' ', '-')}.org",
                    filePath = "related/${title.lowercase().replace(' ', '-')}.org",
                    title = title,
                    backlinkCount = 0,
                    forwardLinkCount = 0,
                ),
            direction = direction,
            preview = "A short excerpt that makes this relation intelligible.",
        )

    private fun bridgeCandidate(note: NodeRecord, connector: String): ExplorationEntry =
        ExplorationEntry.Anchor(
            AnchorExplorationRecord(
                anchor = note,
                explanation =
                    ExplorationExplanation.BridgeCandidate(
                        references = emptyList(),
                        viaNotes =
                            listOf(
                                BridgeEvidenceRecord(
                                    nodeKey = "file:${connector.lowercase().replace(' ', '-')}.org",
                                    explicitId = null,
                                    title = connector,
                                ),
                            ),
                    ),
            ),
        )

    private fun unresolvedExploration(
        unfinished: NodeRecord,
        quiet: NodeRecord,
    ): ExploreResult =
        ExploreResult(
            lens = ExplorationLens.UNRESOLVED,
            sections =
                listOf(
                    ExplorationSection(
                        kind = ExplorationSectionKind.UNRESOLVED_TASKS,
                        entries =
                            listOf(
                                ExplorationEntry.Anchor(
                                    AnchorExplorationRecord(
                                        anchor = unfinished,
                                        explanation =
                                            ExplorationExplanation.UnresolvedSharedReference(
                                                references = listOf("cite:unfinished"),
                                                todoKeyword = "TODO",
                                            ),
                                    ),
                                ),
                            ),
                    ),
                    ExplorationSection(
                        kind = ExplorationSectionKind.WEAKLY_INTEGRATED_NOTES,
                        entries =
                            listOf(
                                ExplorationEntry.Anchor(
                                    AnchorExplorationRecord(
                                        anchor = quiet,
                                        explanation =
                                            ExplorationExplanation
                                                .WeaklyIntegratedSharedReference(
                                                    references = listOf("cite:quiet"),
                                                    structuralLinkCount = 1,
                                                    viaNotes =
                                                        listOf(
                                                            BridgeEvidenceRecord(
                                                                nodeKey = "file:bridge.org",
                                                                explicitId = null,
                                                                title = "Bridge note",
                                                            ),
                                                        ),
                                                ),
                                    ),
                                ),
                            ),
                    ),
                ),
        )

    private fun boundedBridgeExploration(): ReaderExplorationPhase =
        ReaderExplorationPhase.Ready(
            ExploreResult(
                lens = ExplorationLens.BRIDGES,
                sections =
                    listOf(
                        ExplorationSection(
                            kind = ExplorationSectionKind.BRIDGE_CANDIDATES,
                            entries =
                                (1..EXPLORATION_QUERY_LIMIT).map { index ->
                                    ExplorationEntry.Anchor(
                                        AnchorExplorationRecord(
                                            anchor =
                                                explorationNode(
                                                    "Bridge candidate $index",
                                                    "bridge-$index",
                                                ),
                                            explanation =
                                                ExplorationExplanation.BridgeCandidate(
                                                    references = listOf("cite:$index"),
                                                    viaNotes = emptyList(),
                                                ),
                                        ),
                                    )
                                },
                        ),
                    ),
            ),
        )

    private fun explorationNode(title: String, stem: String): NodeRecord =
        node().copy(
            nodeKey = "file:$stem.org",
            filePath = "$stem.org",
            title = title,
            backlinkCount = 0,
            forwardLinkCount = 0,
        )

    private fun mention(note: NodeRecord, row: Long, preview: String): UnlinkedReferenceRecord =
        UnlinkedReferenceRecord(
            sourceNote = note,
            sourceAnchor = note,
            row = row,
            col = preview.indexOf("A complete note").toLong() + 1,
            preview = preview,
            matchedText = "A complete note",
            explanation = ExplorationExplanation.UnlinkedReference("A complete note"),
        )

    private fun preview(
        glossary: Boolean = false,
        shortened: Boolean = false,
        origin: String = "3f2a9c81-4d5e-4f60-9a1b-0c2d3e4f5061:1",
    ): ReaderPreview =
        ReaderPreview(
            request =
                ReaderPreviewRequest(
                    target = "id:target",
                    gesture = io.github.b_vitamins.slipbox.ui.content.DocumentGesture.Touch,
                    originProgress = 0.4f,
                    origin = origin,
                ),
            anchor =
                node().copy(
                    nodeKey = "file:target.org",
                    filePath = "target.org",
                    title = if (glossary) "Derivative" else "Target note",
                    glossary = glossary,
                ),
            source =
                DocumentSource(
                    source = "0123456789abcdef0123456789abcdef",
                    generation = "generation-1",
                    id = "file:target.org",
                    filePath = "notes/target.org",
                    org =
                        if (shortened) {
                            "A rate of change with \\(f'(x)\\).\n\n" +
                                "* Reading the term\n\n" +
                                "The derivative describes local change and the slope of a tangent.\n\n" +
                                "- Compare nearby values.\n" +
                                "- Follow how a quantity moves.\n"
                        } else {
                            "A rate of change with \\(f'(x)\\).\n"
                        },
                ),
            excerptLines = if (shortened) 12 else 1,
            shortened = shortened,
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
