/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.os.Build
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
class DocumentGestureTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val raised = Gestures()

    private lateinit var view: WebView

    @Before
    fun mount() {
        composeRule.setContent {
            DocumentContentView(
                source = DocumentSource(id = "gestures", generation = 3, org = Notes.RICH),
                presentation =
                    documentPresentation(
                        density = LocalDensity.current,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = 411.dp,
                    ),
                modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
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
        assertEquals("the link the app is told to open", DocumentIntent.Pin(SETTLED), reported.last())
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
        assertEquals(
            "the preview the reader asked for, and how they asked",
            DocumentIntent.Glance(SETTLED, DocumentGesture.Touch),
            raised.awaited(1).first(),
        )
        view.answer(WITHDRAW_PREVIEW)
        assertEquals(
            "which the escape key withdraws",
            DocumentIntent.Dismiss,
            raised.awaited(2).last(),
        )
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
            "${DocumentOrigin.assetBase(token)}file%3Adiagram.png",
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
