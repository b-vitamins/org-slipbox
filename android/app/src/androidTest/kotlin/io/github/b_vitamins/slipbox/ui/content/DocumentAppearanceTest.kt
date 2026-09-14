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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.PixelMap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.FontScale
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.contrast
import io.github.b_vitamins.slipbox.ui.darkest
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.hex
import io.github.b_vitamins.slipbox.ui.lightest
import io.github.b_vitamins.slipbox.ui.paper
import io.github.b_vitamins.slipbox.ui.pixels
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class DocumentAppearanceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val raised = Gestures()
    private val store = AssetStore(mapOf("file:diagram.png" to pngAsset(DIAGRAM, DIAGRAM_TONE)))

    private var dark by mutableStateOf(false)
    private var scale = 1f

    private lateinit var density: Density

    @Test
    fun theNoteIsSetReadablyInEitherScheme() = read(1f)

    @Test
    fun andAtTheScaleTheReaderAskedFor() = read(2f)

    private fun read(fontScale: Float) {
        scale = fontScale
        composeRule.setContent {
            DeviceConfigurationOverride(DeviceConfigurationOverride.FontScale(scale)) {
                density = LocalDensity.current
                DocumentContentView(
                    source = SOURCE,
                    presentation =
                        documentPresentation(
                            density = density,
                            dark = dark,
                            motion = SlipboxMotion(),
                            availableWidth = WIDTH,
                        ),
                    modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                    onIntent = raised,
                    resolveAsset = store,
                )
            }
        }
        val view = shown()
        assertTrue(
            "the document is offered to accessibility",
            composeRule.runOnIdle { view.isImportantForAccessibility },
        )
        view.answer(SHOW_ASSET)
        view.awaitTrue("the served asset decoded", DECODED)

        for (scheme in listOf(false, true)) {
            composeRule.runOnIdle { dark = scheme }
            composeRule.waitForIdle()
            view.awaitTrue(
                "the mount repainted in the scheme the app asked for",
                "document.querySelector('.org-document-host').dataset.theme === " +
                    (if (scheme) "'dark'" else "'light'"),
            )
            measure(view, scheme)
        }
        assertEquals(
            "reading the note raised nothing",
            emptyList<DocumentIntent>(),
            raised.reported(),
        )
    }

    private fun measure(view: WebView, scheme: Boolean) {
        val label = "${if (scheme) "dark" else "light"}-${if (scale == 1f) "1_0" else "2_0"}"
        view.answer("window.scrollTo(0, 0);")
        view.awaitTrue("the note is back at its head", "window.scrollY === 0")
        val page = snapshot()
        Evidence.image("content-$label", page)
        val canvas = page.pixels()[EDGE, EDGE]
        assertEquals(
            "$label: the note is set on the app's own paper",
            hex(paper(scheme)),
            hex(canvas),
        )

        val heading = painted(view, HEADING)
        assertTrue("$label: the heading reads: ${heading.ratio}", heading.ratio >= INK)
        val math = painted(view, MATH)
        assertTrue("$label: the display equation is set: ${math.region}", math.region.height > 1)
        assertTrue("$label: and reads: ${math.ratio}", math.ratio >= MUTED)
        val code = painted(view, CODE)
        assertTrue("$label: the code reads: ${code.ratio}", code.ratio >= MUTED)
        val table = painted(view, TABLE)
        assertTrue("$label: the table reads: ${table.ratio}", table.ratio >= MUTED)
        val asset = painted(view, ASSET)
        val tone = asset.middle
        assertTrue(
            "$label: the figure is painted from the bytes the app served: ${hex(tone)}",
            tone.red > 0.9f && tone.green < 0.1f && tone.blue < 0.1f,
        )

        Evidence.record(
            "content-$label",
            Record()
                .text("label", label)
                .size("density", density.density)
                .size("fontScale", scale)
                .colour("canvas", canvas)
                .size("heading", heading.ratio)
                .size("math", math.ratio)
                .size("code", code.ratio)
                .size("table", table.ratio)
                .count("mathHeightPx", math.region.height)
                .colour("asset", tone)
                .count("assetPx", asset.region.width)
                .text("faces", view.text(FACES)),
        )
    }

    private fun painted(view: WebView, selector: String): Painted {
        view.answer("document.querySelector(${selector.quoted()}).scrollIntoView({block:'center'});")
        view.awaitTrue("$selector came into view", inView(selector))
        val box = view.record(boxOf(selector))
        val pixels = snapshot().pixels()
        return Painted(pixels, region(box, pixels))
    }

    private fun region(box: JSONObject, pixels: PixelMap): IntRect {
        fun across(name: String, extent: Int): Int =
            (box.getDouble(name) * density.density).toInt().coerceIn(0, extent)
        val left = across("left", pixels.width - 1)
        val top = across("top", pixels.height - 1)
        return IntRect(
            left = left,
            top = top,
            right = across("right", pixels.width).coerceAtLeast(left + 1),
            bottom = across("bottom", pixels.height).coerceAtLeast(top + 1),
        )
    }

    private fun snapshot(): ImageBitmap = composeRule.onNodeWithTag(DOCUMENT_TAG).captureToImage()

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

    private class Painted(private val pixels: PixelMap, val region: IntRect) {
        val ratio: Float
            get() =
                contrast(
                    pixels.darkest(region.left, region.top, region.right, region.bottom),
                    pixels.lightest(region.left, region.top, region.right, region.bottom),
                )

        val middle: Color
            get() = pixels[(region.left + region.right) / 2, (region.top + region.bottom) / 2]
    }

    private companion object {
        val WIDTH = 411.dp

        val SOURCE = DocumentSource(id = "appearance", generation = 1, org = Notes.RICH)

        const val EDGE = 2

        const val INK = 7f
        const val MUTED = 4.5f

        const val DIAGRAM = 48
        val DIAGRAM_TONE: Int = 0xFFFF0000.toInt()

        const val HEADING = "#document .org-heading"
        const val MATH = "#document .org-math--display .katex"
        const val CODE = "#document pre.org-src__code"
        const val TABLE = "#document table.org-table"
        const val ASSET = "#document img.probe-asset"

        const val SHOW_ASSET =
            "(function () {" +
                "  const link = document.querySelector('#document a.org-link--asset');" +
                "  const figure = document.createElement('img');" +
                "  figure.className = 'probe-asset';" +
                "  figure.width = $DIAGRAM;" +
                "  figure.height = $DIAGRAM;" +
                "  figure.src = link.href;" +
                "  link.parentNode.insertBefore(figure, link);" +
                "})();"

        const val DECODED =
            "(function () {" +
                "  const figure = document.querySelector('$ASSET');" +
                "  return figure !== null && figure.complete && figure.naturalWidth === $DIAGRAM;" +
                "})()"

        const val FACES =
            "Array.from(document.fonts)" +
                ".filter(face => face.family.indexOf('KaTeX') === 0 && face.status === 'loaded')" +
                ".map(face => face.family).sort().join(',')"

        fun inView(selector: String): String =
            "(function () {" +
                "  const box = document.querySelector(${selector.quoted()})" +
                "    .getBoundingClientRect();" +
                "  return box.width > 0 && box.height > 0 && box.bottom > 0 &&" +
                "    box.top < document.documentElement.clientHeight;" +
                "})()"

        fun boxOf(selector: String): String =
            "JSON.stringify((function () {" +
                "  const box = document.querySelector(${selector.quoted()})" +
                "    .getBoundingClientRect();" +
                "  return { left: box.left, top: box.top, right: box.right," +
                "    bottom: box.bottom };" +
                "})())"
    }
}
