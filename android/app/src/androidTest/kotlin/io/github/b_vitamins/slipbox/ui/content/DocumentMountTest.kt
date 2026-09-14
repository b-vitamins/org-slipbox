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
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class DocumentMountTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val raised = Gestures()

    private var mounted by mutableStateOf(true)
    private var dark by mutableStateOf(false)
    private var source by
        mutableStateOf(DocumentSource(id = "mount", generation = 1, org = Notes.TALL))

    @Before
    fun compose() {
        composeRule.setContent {
            if (mounted) {
                DocumentContentView(
                    source = source,
                    presentation =
                        documentPresentation(
                            density = LocalDensity.current,
                            dark = dark,
                            motion = SlipboxMotion(),
                            availableWidth = WIDTH,
                        ),
                    modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                    onIntent = raised,
                )
            }
        }
    }

    @Test
    fun aChangeOfPresentationLeavesTheReaderWhereTheyWere() {
        val view = shown()
        view.answer(MARK)
        view.answer("window.scrollTo(0, $SCROLL);")
        view.awaitTrue("the reader went down the note", "window.scrollY >= ${SCROLL - 1}")
        view.answer("document.querySelector('$DOCUMENT_LINK').focus();")
        val focused = view.text(FOCUSED)
        assertNotEquals("the reader put focus on a link", "none", focused)
        val place = view.number("window.scrollY")
        val token = view.mountToken()

        repaint(view, dark = true)

        assertEquals("the document that was mounted is still mounted", KEPT, view.text(PROBE))
        assertEquals("under the binding it was mounted under", token, view.mountToken())
        assertEquals("at the place the reader had reached", place, view.number("window.scrollY"), 1.0)
        assertEquals("with the element they had focused still focused", focused, view.text(FOCUSED))
        assertEquals(
            "and nothing raised by repainting it",
            emptyList<DocumentIntent>(),
            raised.quiet(),
        )
        Evidence.record(
            "content-mount",
            Record()
                .text("probe", view.text(PROBE))
                .text("focused", focused)
                .size("scrollY", place.toFloat())
                .text("theme", view.text(THEME)),
        )
    }

    @Test
    fun newTextWithdrawsThePreviewTheOldTextStoodUnder() {
        val view = shown()
        view.answer(MARK)
        view.answer(PRESS_LINK)
        val standing = raised.awaited(1)
        assertEquals("the reader asked for a preview", 1, standing.size)

        repaint(view, dark = true)
        assertEquals("which a repaint leaves standing", standing, raised.quiet())

        val token = view.mountToken()
        composeRule.runOnIdle {
            source = DocumentSource(id = "mount", generation = 2, org = Notes.ASSETS)
        }
        composeRule.waitForIdle()
        view.awaitTrue(
            "the new text is shown",
            "document.getElementById('document').textContent" +
                ".indexOf(${REPLACEMENT.quoted()}) >= 0",
        )

        assertEquals(
            "the withdrawal the renderer raises before it replaces the text",
            DocumentIntent.Dismiss,
            raised.awaited(2).last(),
        )
        assertNotEquals("new text is a new binding", token, view.mountToken())
        assertEquals("mounted in the container the app gave it", KEPT, view.text(PROBE))
        assertFalse(
            "and nothing of the text it replaced is left",
            view.text(TEXT).contains(REPLACED),
        )
    }

    @Test
    fun aDisposedMountLeavesNothingBehindIt() {
        val view = shown()
        view.answer(PRESS_LINK)
        val standing = raised.awaited(1)
        assertEquals("a preview stood when the mount was disposed", 1, standing.size)

        composeRule.runOnIdle { mounted = false }
        composeRule.waitForIdle()

        assertNull(
            "the view left with the mount",
            composeRule.runOnIdle { documentViewIn(composeRule.activity.window.decorView) },
        )
        assertEquals("a disposed mount raises nothing of its own", standing, raised.quiet())
    }

    @Test
    fun aRetiredHostIsInert() {
        val retired = Gestures()
        val presentation =
            documentPresentation(
                density = Density(DENSITY),
                dark = false,
                motion = SlipboxMotion(),
                availableWidth = WIDTH,
            )
        composeRule.runOnUiThread {
            val host =
                DocumentHost(
                    context = composeRule.activity,
                    onIntent = retired,
                    resolver = DocumentAssetResolver { _, _ -> null },
                )
            host.dispose()
            host.present(source, presentation)
            host.dispose()
        }
        composeRule.waitForIdle()
        assertEquals("a retired host raises nothing", emptyList<DocumentIntent>(), retired.quiet())
    }

    private fun repaint(view: WebView, dark: Boolean) {
        composeRule.runOnIdle { this.dark = dark }
        composeRule.waitForIdle()
        val scheme = if (dark) "dark" else "light"
        view.awaitTrue("the mount repainted in $scheme", "$THEME === ${scheme.quoted()}")
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

        const val DENSITY = 2f

        const val SCROLL = 240

        const val KEPT = "kept"

        const val REPLACEMENT = "one that walks out"

        const val REPLACED = "fixed point"

        const val MARK =
            "(function () {" +
                "  document.querySelector('.org-document-host').dataset.probe = '$KEPT';" +
                "})();"

        const val PROBE = "(document.querySelector('.org-document-host').dataset.probe || 'gone')"

        const val THEME = "document.querySelector('.org-document-host').dataset.theme"

        const val TEXT = "document.getElementById('document').textContent"

        const val FOCUSED =
            "(document.activeElement && document.activeElement.textContent) || 'none'"
    }
}
