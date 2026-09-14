/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.os.Build
import android.os.SystemClock
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.fillMaxSize
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
class DocumentContainmentTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val raised = Gestures()
    private val store = AssetStore()

    private lateinit var view: WebView

    @Before
    fun mount() {
        composeRule.setContent {
            DocumentContentView(
                source = DocumentSource(id = "hostile", generation = 7, org = Notes.HOSTILE),
                presentation =
                    documentPresentation(
                        density = LocalDensity.current,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = 411.dp,
                    ),
                modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                onIntent = raised,
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
    fun whatTheNoteSuppliedIsReadAndNotRun() {
        val document = view.record(CONTENT)
        assertEquals("nothing the note wrote reached the page", "undefined", document.getString("owned"))
        assertEquals("the note supplied no script", 0, document.getInt("scripts"))
        assertEquals("and no element to fail into a handler", 0, document.getInt("images"))
        assertEquals("nor a handler on one", 0, document.getInt("handlers"))
        assertTrue("its markup is shown as the text it is", document.getBoolean("literal"))
        assertEquals("no anchor carries a scheme that executes", 0, document.getInt("executable"))
        assertEquals("one target the renderer will not follow", 1, document.getInt("assetLinks"))
        assertEquals(
            "the safe external target is an ordinary link",
            "https://elsewhere.invalid/page",
            document.getString("external"),
        )
        val scoped = document.getString("scoped")
        assertTrue(
            "an unfollowable target is scoped to the mount: $scoped",
            scoped.startsWith("${DocumentOrigin.ORIGIN}${DocumentOrigin.ASSET_PREFIX}${view.mountToken()}/"),
        )
        Evidence.record("content-containment", Record().raw("content", document.toString()))
    }

    @Test
    fun untrustedMathIsShownAndNotObeyed() {
        val math = view.record(MATH)
        assertTrue("rejected TeX is marked as rejected", math.getInt("errors") >= 1)
        assertEquals("no equation built a link", 0, math.getInt("anchors"))
        assertEquals("and none built an image", 0, math.getInt("images"))
        assertTrue(
            "the untrusted command is shown verbatim: ${math.getString("text")}",
            math.getString("text").contains("href"),
        )
        assertTrue(
            "the equations are still set",
            math.getInt("rendered") >= 1 && math.getInt("height") > 0,
        )
        assertTrue(
            "nothing was fetched from the target the TeX named",
            view.fetched().none { it.contains("elsewhere.invalid") },
        )
        Evidence.record("content-math", Record().raw("math", math.toString()))
    }

    @Test
    fun nothingNavigatesTheViewAndNothingReachesANetwork() {
        view.answer("document.querySelector('#document a.org-link--external').click();")
        view.answer("window.location.href = 'https://elsewhere.invalid/';")
        SystemClock.sleep(SETTLE_MILLIS)
        assertEquals("the view is still reading the page it loaded", DocumentOrigin.PAGE, view.text("location.href"))
        assertEquals(DocumentOrigin.PAGE, composeRule.runOnIdle { view.url })

        view.answer(FETCH)
        view.awaitTrue("the fetch settled", "window.__fetch !== 'pending'")
        assertEquals("the page may not ask for bytes itself", "rejected", view.text("window.__fetch"))

        view.answer(FRAME)
        SystemClock.sleep(SETTLE_MILLIS)
        assertEquals("a frame was appended", 1.0, view.number("document.querySelectorAll('iframe').length"), 0.0)
        // CSP may stop a frame before interception (status 0); either path must refuse it.
        val framed = view.number(status(DocumentOrigin.PAGE_FILE))
        assertTrue("the frame was served a document to run: $framed", framed != 200.0)
        assertEquals("so it holds nothing of the renderer", 0.0, view.number(FRAMED), 0.0)

        assertEquals("failed", view.probeImage(DocumentOrigin.PAGE))
        assertEquals(
            "the app's own page is refused to anything but the page",
            404.0,
            view.number(status(DocumentOrigin.PAGE_FILE)),
            0.0,
        )
        assertEquals("while the bundle it loaded was served", 200.0, view.number(status("host.js")), 0.0)
        assertTrue("no gesture came of any of it", raised.reported().isEmpty())
        assertTrue("and no asset was asked for", store.asks().isEmpty())
        Evidence.record(
            "content-navigation",
            Record()
                .text("url", view.text("location.href"))
                .text("fetch", view.text("window.__fetch"))
                .count("pageAsSubresource", view.number(status(DocumentOrigin.PAGE_FILE)).toInt())
                .count("pageRequests", view.number(entries(DocumentOrigin.PAGE_FILE)).toInt())
                .count("frameAnswered", framed.toInt())
                .count("framedNodes", view.number(FRAMED).toInt())
                .count("intents", raised.reported().size),
        )
    }

    private companion object {
        const val SETTLE_MILLIS = 750L

        fun status(name: String): String =
            "Number((performance.getEntriesByType('resource')" +
                ".filter(entry => entry.name.indexOf(${name.quoted()}) >= 0).pop() || {})" +
                ".responseStatus || 0)"

        fun entries(name: String): String =
            "performance.getEntriesByType('resource')" +
                ".filter(entry => entry.name.indexOf(${name.quoted()}) >= 0).length"

        const val CONTENT =
            "JSON.stringify((function () {" +
                "  const found = selector => document.querySelectorAll(selector).length;" +
                "  const mount = document.getElementById('document');" +
                "  return {" +
                "    owned: typeof window.slipboxOwned," +
                "    scripts: found('#document script')," +
                "    images: found('#document img')," +
                "    handlers: found('#document [onerror]')," +
                "    literal: mount.textContent.indexOf('<script>') >= 0," +
                "    executable: found('#document a[href^=\"javascript:\"]')," +
                "    assetLinks: found('#document a.org-link--asset')," +
                "    external: document.querySelector('#document a.org-link--external').href," +
                "    scoped: document.querySelector('#document a.org-link--asset').href" +
                "  };" +
                "})())"

        const val MATH =
            "JSON.stringify((function () {" +
                "  const found = selector => document.querySelectorAll(selector).length;" +
                "  const set = document.querySelector('#document .org-math--display .katex');" +
                "  return {" +
                "    errors: found('#document .katex-error')," +
                "    anchors: found('#document .org-math a')," +
                "    images: found('#document .org-math img')," +
                "    rendered: found('#document .org-math .katex')," +
                "    height: set === null ? 0 : Math.round(set.getBoundingClientRect().height)," +
                "    text: document.querySelector('#document .org-math').textContent" +
                "  };" +
                "})())"

        const val FETCH =
            "window.__fetch = 'pending';" +
                "fetch('/bundle/host.js').then(" +
                "  () => { window.__fetch = 'resolved'; }," +
                "  () => { window.__fetch = 'rejected'; });"

        const val FRAME =
            "(function () {" +
                "  const frame = document.createElement('iframe');" +
                "  frame.src = '/bundle/index.html';" +
                "  document.body.appendChild(frame);" +
                "})();"

        const val FRAMED =
            "(function () {" +
                "  const inner = document.querySelector('iframe').contentDocument;" +
                "  return inner === null ? 0 :" +
                "    inner.querySelectorAll('#document, script, link').length;" +
                "})()"
    }
}
