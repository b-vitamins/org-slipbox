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
class DocumentGestureTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val raised = Gestures()

    private var focusRequest by mutableStateOf<DocumentFocusRequest?>(null)

    private lateinit var view: WebView

    @Before
    fun mount() {
        composeRule.setContent {
            DocumentContentView(
                source =
                    DocumentSource(
                        source = "test-source",
                        generation = "3",
                        id = "gestures",
                        filePath = "notes/gestures.org",
                        org = Notes.RICH,
                    ),
                presentation =
                    documentPresentation(
                        density = LocalDensity.current,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = 411.dp,
                    ),
                modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
                restoreFocus = focusRequest,
                onIntent = raised,
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
    fun theKeyboardOpensALinkAndTheAppDecidesWhatItOpens() {
        view.answer(OPEN_LINK)
        val reported = raised.awaited(2)
        val opened = reported.last() as DocumentIntent.Pin
        assertEquals("the link the app is told to open", SETTLED, opened.link)
        assertTrue("the link carries its reading block", opened.position?.mark?.isNotEmpty() == true)
        assertTrue(
            "nothing else was raised but the withdrawal that precedes opening: $reported",
            reported.dropLast(1).all { it == DocumentIntent.Dismiss },
        )
        Evidence.record(
            "content-gestures",
            Record()
                .text("raised", reported.joinToString(" ") { it.javaClass.simpleName })
                .text("id", SETTLED.id.orEmpty())
                .text("reference", SETTLED.reference),
        )
    }

    @Test
    fun aTouchAsksForAPreviewAndEscapeWithdrawsIt() {
        view.answer(PRESS_LINK)
        val preview = raised.awaited(1).first() as DocumentIntent.Glance
        assertEquals("the preview target", SETTLED, preview.link)
        assertEquals("how the reader asked", DocumentGesture.Touch, preview.gesture)
        assertEquals("where the preview returns focus", "${view.mountToken()}:1", preview.origin)
        view.answer(WITHDRAW_PREVIEW)
        assertEquals(
            "which the escape key withdraws",
            DocumentIntent.Dismiss,
            raised.awaited(2).last(),
        )
    }

    @Test
    fun dismissingANativePreviewRestoresItsExactLinkWithoutMovingTheDocument() {
        view.answer("window.scrollTo(0, 80)")
        view.answer(PRESS_LINK)
        val preview = raised.awaited(1).first() as DocumentIntent.Glance
        assertEquals(
            preview.origin,
            view.text("document.querySelector('$DOCUMENT_LINK').dataset.slipboxPreviewOrigin"),
        )
        view.answer(
            "const other = document.createElement('button');" +
                "other.id = 'other-focus'; document.body.append(other); other.focus();",
        )
        view.awaitTrue(
            "the origin yielded focus before restoration",
            "document.activeElement.id === 'other-focus'",
        )
        view.answer("window.scrollTo(0, 80)")
        val before = view.number("window.scrollY")

        composeRule.runOnIdle { focusRequest = DocumentFocusRequest(preview.origin) }
        composeRule.waitForIdle()
        view.awaitTrue(
            "the exact preview origin regained focus",
            "document.activeElement === document.querySelector('$DOCUMENT_LINK')",
        )

        assertEquals(
            "restoring focus kept the reading place",
            before,
            view.number("window.scrollY"),
            0.0,
        )
    }

    @Test
    fun orgAndExternalTargetsBothCrossTheTypedChannel() {
        view.answer("Array.from(document.querySelectorAll('#document a')).find(a => " +
            "a.textContent === 'the next note').click();")
        val org = raised.awaited(2).last() as DocumentIntent.Pin
        assertEquals(NEXT, org.link)
        assertTrue(org.position?.mark?.isNotEmpty() == true)

        raised.forget()
        view.answer("Array.from(document.querySelectorAll('#document a')).find(a => " +
            "a.textContent === 'the paper').click();")
        val external = raised.awaited(1).single() as DocumentIntent.Go
        assertEquals(EXTERNAL, external.link)
        assertTrue(external.position?.mark?.isNotEmpty() == true)
    }

    @Test
    fun aMessageTheChannelDidNotIssueIsRefused() {
        val live = view.mountToken()
        val link = "{\"id\":${Notes.NOTE_ID.quoted()},\"target\":${SETTLED.target.quoted()}," +
            "\"reference\":${SETTLED.reference.quoted()}}"
        val refused =
            listOf(
                "another mount's token" to raisedIntent(STALE, "pin", link),
                "a verb the renderer never raises" to raisedIntent(live, "levitate"),
                "a pin naming nothing" to raisedIntent(live, "pin"),
                "a glance with no gesture behind it" to raisedIntent(live, "glance", link),
                "a gesture that is not one" to raisedIntent(live, "glance", link, "\"stare\""),
                "a pin a gesture asked for" to raisedIntent(live, "pin", link, "\"touch\""),
                "a dismissal naming a link" to raisedIntent(live, "dismiss", link),
                "a key the message may not carry" to
                    raisedIntent(live, "dismiss", extra = ",\"origin\":\"elsewhere\""),
                "text that is not a message" to "not a message",
                "more text than a gesture needs" to
                    raisedIntent(live, "pin", flooded()),
            )
        for ((what, payload) in refused) {
            post(payload)
            assertEquals("$what was carried", emptyList<DocumentIntent>(), raised.quiet())
        }
        post(raisedIntent(live, "dismiss"))
        assertEquals(listOf(DocumentIntent.Dismiss), raised.awaited(1))
        Evidence.record(
            "content-refusals",
            Record().count("refused", refused.size).count("carried", raised.reported().size),
        )
    }

    @Test
    fun aMountsOwnTokenIsTheOnlyOneItsUrlsCarry() {
        val token = view.mountToken()
        assertTrue("the token is the app's own opaque name: $token", token.isNotEmpty())
        val hrefs = view.strings(HREFS)
        assertEquals("one target the renderer cannot follow", 1, hrefs.size)
        assertEquals(
            documentAssetUrl(token, "file:diagram.png"),
            hrefs.single(),
        )
    }

    private fun post(payload: String) {
        view.answer("window.${DocumentChannel.OBJECT_NAME}.postMessage(${payload.quoted()});")
    }

    private companion object {
        val SETTLED =
            DocumentLink(
                id = Notes.NOTE_ID,
                target = "id:${Notes.NOTE_ID}",
                reference = "id:${Notes.NOTE_ID}",
            )

        val NEXT =
            DocumentLink(
                id = null,
                target = "file:next.org::*Target",
                reference = "file:next.org::*Target",
            )

        val EXTERNAL =
            DocumentLink(
                id = null,
                target = "https://example.org/page",
                reference = "https://example.org/page",
            )

        const val STALE = "0f9e8d7c-6b5a-4938-8271-605f4e3d2c1b"

        fun flooded(): String =
            "{\"id\":null,\"target\":${"x".repeat(DocumentChannel.MESSAGE_LIMIT).quoted()}," +
                "\"reference\":\"x\"}"

        fun raisedIntent(
            token: String,
            verb: String,
            link: String = "null",
            gesture: String = "null",
            extra: String = "",
        ): String =
            "{\"token\":${token.quoted()},\"verb\":${verb.quoted()}," +
                "\"link\":$link,\"gesture\":$gesture$extra}"

        const val HREFS =
            "JSON.stringify(Array.from(" +
                "document.querySelectorAll('#document a.org-link--asset')).map(a => a.href))"
    }
}
