/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.graphics.Color
import android.os.Build
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class DocumentAssetTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val store =
        AssetStore(
            mapOf(
                "file:diagram.png" to pngAsset(DIAGRAM, Color.RED),
                "file:figures/nested.png" to pngAsset(NESTED, Color.BLUE),
                "figure.png" to DocumentAsset("text/html", PAGE.toByteArray()),
                "figure.svg" to DocumentAsset("image/svg+xml", SVG.toByteArray()),
                "page.html" to DocumentAsset("text/html", PAGE.toByteArray()),
            ),
        )

    private var source by
        mutableStateOf(DocumentSource(id = "assets", generation = 4, org = Notes.ASSETS))

    private lateinit var view: WebView

    @Before
    fun mount() {
        composeRule.setContent {
            DocumentContentView(
                source = source,
                presentation =
                    documentPresentation(
                        density = LocalDensity.current,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = 411.dp,
                    ),
                modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                resolveAsset = store,
            )
        }
        composeRule.waitForIdle()
        view =
            checkNotNull(
                composeRule.runOnIdle { documentViewIn(composeRule.activity.window.decorView) },
            ) {
                "no document view is composed"
            }
        view.awaitMounted()
    }

    @Test
    fun theStoresOwnBytesReachThePageAndAMissOneIsTruthful() {
        val hrefs = view.strings(HREFS)
        assertEquals("every unfollowable target got a URL", 4, hrefs.size)
        assertEquals("the diagram", "loaded ${DIAGRAM}x$DIAGRAM", view.probeImage(hrefs[0]))
        assertEquals("one held under a name with a separator", "loaded ${NESTED}x$NESTED", view.probeImage(hrefs[1]))
        assertEquals("one the store does not hold", "failed", view.probeImage(hrefs[2]))
        assertEquals("one that walks out of it", "failed", view.probeImage(hrefs[3]))
        assertEquals(
            "the store was asked by the names the note spelled, and about nothing else",
            listOf("file:diagram.png", "file:figures/nested.png", "file:missing.png"),
            store.asks().map { it.target },
        )
        assertTrue(
            "under the binding of the document that asked",
            store.asks().all { it.binding == DocumentBinding("assets", 4) },
        )
        assertEquals("a miss is a refusal, not a fallback", 404.0, view.number(status("missing.png")), 0.0)
        assertEquals("a hit is served", 200.0, view.number(status("diagram.png")), 0.0)
        Evidence.record(
            "content-assets",
            Record()
                .count("targets", hrefs.size)
                .count("asked", store.asks().size)
                .text("names", store.asks().joinToString(" ") { it.target })
                .text("binding", store.asks().first().binding.toString()),
        )
    }

    @Test
    fun bytesADocumentCouldRunAreRefusedWhateverTheStoreCallsThem() {
        val base = DocumentOrigin.assetBase(view.mountToken())
        assertEquals("failed", view.probeImage("${base}page.html"))
        assertTrue("a document was asked for: ${store.asks()}", store.asks().isEmpty())
        assertEquals(404.0, view.number(status("page.html")), 0.0)

        for (target in DISGUISED) {
            assertEquals("$target was served", "failed", view.probeImage("$base$target"))
            assertEquals("$target was not refused", 404.0, view.number(status(target)), 0.0)
        }
        assertEquals(
            "the store was asked for each, and every answer refused on its type",
            DISGUISED,
            store.asks().map { it.target },
        )
        Evidence.record(
            "content-asset-types",
            Record()
                .text("refusedBeforeTheStore", "page.html")
                .text("refusedOnTheirType", DISGUISED.joinToString(" "))
                .count("asked", store.asks().size),
        )
    }

    @Test
    fun aTargetThatWalksOutIsRefusedBeforeTheStoreIsAsked() {
        val base = DocumentOrigin.assetBase(view.mountToken())
        for (walk in WALKS) {
            assertEquals("$walk was answered", "failed", view.probeImage("$base$walk"))
        }
        assertTrue("the store was asked to walk: ${store.asks()}", store.asks().isEmpty())
    }

    @Test
    fun aUrlFromAnEarlierMountIsRefusedRatherThanRetargeted() {
        val issued = view.strings(HREFS).first()
        val token = view.mountToken()
        composeRule.runOnIdle {
            source = DocumentSource(id = "assets", generation = 5, org = Notes.ASSETS)
        }
        composeRule.waitForIdle()
        view.awaitTrue(
            "the new text was mounted under a token of its own",
            "document.querySelector('#document a.org-link--asset')" +
                ".href.indexOf(${token.quoted()}) < 0",
        )
        store.forget()

        assertEquals("the URL the replaced text was given", "failed", view.probeImage(issued))
        assertTrue("which no store was asked about: ${store.asks()}", store.asks().isEmpty())

        val reissued = view.strings(HREFS).first()
        assertEquals("the same bytes under the new binding", "loaded ${DIAGRAM}x$DIAGRAM", view.probeImage(reissued))
        assertEquals(
            "asked for under the generation that is mounted now",
            AssetAsk(DocumentBinding("assets", 5), "file:diagram.png"),
            store.asks().single(),
        )
    }

    private companion object {
        const val DIAGRAM = 48
        const val NESTED = 32

        const val PAGE = "<script>window.slipboxOwned = true;</script>"

        const val SVG =
            "<svg xmlns=\"http://www.w3.org/2000/svg\">" +
                "<script>window.slipboxOwned = true;</script></svg>"

        val DISGUISED = listOf("figure.png", "figure.svg")

        val WALKS =
            listOf(
                "..%2F..%2Fetc%2Fhosts",
                "%2e%2e%2Fdiagram.png",
                "figures%2F..%2F..%2Fdiagram.png",
            )

        const val HREFS =
            "JSON.stringify(Array.from(" +
                "document.querySelectorAll('#document a.org-link--asset')).map(a => a.href))"

        fun status(name: String): String =
            "Number((performance.getEntriesByType('resource')" +
                ".filter(entry => entry.name.indexOf(${name.quoted()}) >= 0).pop() || {})" +
                ".responseStatus || 0)"
    }
}
