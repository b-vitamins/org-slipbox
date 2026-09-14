/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

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
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.FontScale
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.bounds
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
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
class DocumentReadingTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private var scale by mutableStateOf(1f)

    private lateinit var density: Density
    private lateinit var presented: DocumentPresentation

    @Before
    fun mount() {
        composeRule.setContent {
            DeviceConfigurationOverride(DeviceConfigurationOverride.FontScale(scale)) {
                density = LocalDensity.current
                presented =
                    documentPresentation(
                        density = density,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = WIDTH,
                    )
                DocumentContentView(
                    source = DocumentSource(id = "reading", generation = 1, org = Notes.RICH),
                    presentation = presented,
                    modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                )
            }
        }
    }

    @Test
    fun theWholeDocumentIsReadFromTheBundleAndNothingElse() {
        val view = shown()
        view.awaitTrue(
            "the equations fetched a font",
            "performance.getEntriesByType('resource').some(e => e.name.endsWith('.woff2'))",
        )
        // Chromium can also request a favicon, which the host refuses.
        val requests = view.requests()
        val bundle = "${DocumentOrigin.ORIGIN}${DocumentOrigin.BUNDLE_PREFIX}"
        for (request in requests) {
            assertTrue(
                "${request.name} was asked of somewhere other than the app",
                request.name.startsWith(DocumentOrigin.ORIGIN),
            )
            assertEquals(
                "${request.name} was answered ${request.status}",
                request.name.startsWith(bundle),
                request.status == 200,
            )
            assertTrue(
                "${request.name} was neither served nor refused",
                request.status == 200 || request.status == 404,
            )
        }
        val fetched = requests.filter { it.status == 200 }.map { it.name }
        assertTrue("the renderer was loaded", fetched.contains("${bundle}document.js"))
        assertTrue("its stylesheet was loaded", fetched.contains("${bundle}document.css"))
        assertTrue("the app's own script mounted it", fetched.contains("${bundle}host.js"))
        view.answer(LOADED_FACES)
        view.awaitTrue(
            "the maths faces loaded from the bundle",
            "typeof window.__fonts === 'string' && window.__fonts.length > 0",
        )
        Evidence.record(
            "content-offline",
            Record()
                .count("requests", requests.size)
                .count("served", fetched.size)
                .count("refused", requests.count { it.status == 404 })
                .count("fonts", fetched.count { it.endsWith(".woff2") })
                .text("origin", bundle)
                .text("faces", view.text("window.__fonts"))
                .text("names", fetched.joinToString(" ") { it.removePrefix(bundle) })
                .text(
                    "unserved",
                    requests
                        .filter { it.status != 200 }
                        .joinToString(" ") { "${it.name.removePrefix(DocumentOrigin.ORIGIN)}=${it.status}" },
                ),
        )
    }

    @Test
    fun theNoteIsStructuredTheWayItWasWritten() {
        val view = shown()
        val document = view.record(STRUCTURE)
        assertEquals(
            "both headings, at the levels a note nests under",
            "H2,H3",
            document.getString("headings"),
        )
        assertEquals("a table's header cells are column headers", 2, document.getInt("headers"))
        assertEquals("and the rows under them", 2, document.getInt("rows"))
        assertTrue("the code is set verbatim", document.getString("code").contains("fun settle"))
        assertEquals("the inline equation", 1, document.getInt("inlineMath"))
        assertEquals("the display equation", 1, document.getInt("displayMath"))
        assertTrue("both equations carry their reading", document.getInt("mathml") >= 2)
        assertEquals("and neither was rejected", 0, document.getInt("mathErrors"))
        assertTrue("the display equation is set, not collapsed", document.getInt("mathHeight") > 0)
        assertEquals("an id link is a link carrying no URL", 1, document.getInt("idLinks"))
        assertEquals("the file target is left to the app", 1, document.getInt("assetLinks"))
        assertEquals("the note supplied no script", 0, document.getInt("scripts"))
        assertEquals("the page is declared in a language", "en", document.getString("lang"))
        assertEquals("the scheme the app asked for", "light", document.getString("theme"))
        assertEquals("which the page paints in", "light", document.getString("scheme"))
        Evidence.record("content-structure", Record().raw("document", document.toString()))
    }

    @Test
    fun theDocumentIsSetInTheMetricsTheAppSent() {
        val view = shown()
        val expected = presented.cssVariables()
        val applied = view.record(variables(expected.keys))
        for ((name, value) in expected) {
            assertEquals("$name is applied to the mount", value, applied.getString(name))
        }
        assertEquals(
            "one CSS pixel is one density-independent pixel",
            density.density.toDouble(),
            view.number("window.devicePixelRatio"),
            0.01,
        )
        val laid = composeRule.onNodeWithTag(DOCUMENT_TAG).bounds().width / density.density
        assertEquals(
            "the page lays out in the view's own width",
            laid.toDouble(),
            view.number("document.documentElement.clientWidth"),
            2.0,
        )
    }

    @Test
    fun theReadersTextScaleIsAppliedExactlyOnce() {
        val ordinaryMetrics = shown().record(METRICS)
        val ordinary = size(presented.cssVariables())
        assertEquals(
            "the mount is set at the size the app sent",
            ordinary,
            ordinaryMetrics.getDouble("fontSize"),
            0.5,
        )
        composeRule.runOnIdle { scale = 2f }
        val scaledMetrics = shown().record(METRICS)
        val scaled = size(presented.cssVariables())
        // Android's large-text scaling is nonlinear.
        assertTrue("the app sent a larger size than $ordinary: $scaled", scaled > ordinary)
        assertEquals(
            "and the document is set at exactly that",
            scaled,
            scaledMetrics.getDouble("fontSize"),
            0.5,
        )
        assertEquals(
            "which is the app's growth and not a second one on top of it",
            scaled / ordinary,
            scaledMetrics.getDouble("fontSize") / ordinaryMetrics.getDouble("fontSize"),
            0.02,
        )
        assertEquals(
            "the page's own pixel is unchanged by the reader's scale",
            ordinaryMetrics.getDouble("ratio"),
            scaledMetrics.getDouble("ratio"),
            0.01,
        )
        assertEquals(
            "so is the width it lays out in",
            ordinaryMetrics.getDouble("width"),
            scaledMetrics.getDouble("width"),
            1.0,
        )
        Evidence.record(
            "content-scale",
            Record()
                .raw("ordinary", ordinaryMetrics.toString())
                .raw("enlarged", scaledMetrics.toString())
                .size("growth", (scaled / ordinary).toFloat())
                .count("pinnedTextZoom", DocumentSettings.TEXT_ZOOM),
        )
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

    private companion object {
        val WIDTH = 411.dp

        fun size(variables: Map<String, String>): Double =
            checkNotNull(variables["--text-size"]).removeSuffix("px").toDouble()

        fun variables(names: Set<String>): String {
            val listed = names.joinToString(",") { it.quoted() }
            return "JSON.stringify((function () {" +
                "  const style = getComputedStyle(document.getElementById('document'));" +
                "  const applied = {};" +
                "  for (const name of [$listed]) {" +
                "    applied[name] = style.getPropertyValue(name).trim();" +
                "  }" +
                "  return applied;" +
                "})())"
        }

        const val LOADED_FACES =
            "document.fonts.ready.then(() => { window.__fonts = Array.from(document.fonts)" +
                ".filter(face => face.family.indexOf('KaTeX') === 0 && face.status === 'loaded')" +
                ".map(face => face.family).sort().join(','); });"

        const val METRICS =
            "JSON.stringify({" +
                "fontSize: parseFloat(" +
                "getComputedStyle(document.querySelector('.org-document')).fontSize)," +
                "ratio: window.devicePixelRatio," +
                "width: document.documentElement.clientWidth})"

        const val STRUCTURE =
            "JSON.stringify((function () {" +
                "  const found = selector => document.querySelectorAll(selector).length;" +
                "  const display = document.querySelector('#document .org-math--display .katex');" +
                "  const host = document.querySelector('.org-document-host');" +
                "  return {" +
                "    headings: Array.from(document.querySelectorAll('#document .org-heading'))" +
                "      .map(node => node.tagName).join(',')," +
                "    headers: found('#document table.org-table th[scope=\"col\"]')," +
                "    rows: found('#document table.org-table tbody tr')," +
                "    code: document.querySelector('#document pre code').textContent," +
                "    inlineMath: found('#document .org-math--inline .katex')," +
                "    displayMath: found('#document .org-math--display .katex')," +
                "    mathml: found('#document .katex-mathml')," +
                "    mathErrors: found('#document .katex-error')," +
                "    mathHeight: Math.round(display.getBoundingClientRect().height)," +
                "    idLinks: found('#document a.org-link[role=\"link\"][tabindex=\"0\"]')," +
                "    assetLinks: found('#document a.org-link--asset')," +
                "    scripts: found('#document script')," +
                "    lang: document.documentElement.lang," +
                "    theme: host.dataset.theme," +
                "    scheme: getComputedStyle(document.documentElement).colorScheme" +
                "  };" +
                "})())"
    }
}
