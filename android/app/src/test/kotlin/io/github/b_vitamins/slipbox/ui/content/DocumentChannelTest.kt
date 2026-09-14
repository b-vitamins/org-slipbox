/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import org.junit.Assert.assertEquals
import org.junit.Test

class DocumentChannelTest {

    @Test
    fun theRenderersOwnVocabularyCrosses() {
        assertEquals(
            DocumentIntent.Glance(ID_LINK, DocumentGesture.Touch),
            raised("""{"token":"$TOKEN","verb":"glance","link":$ID_LINK_JSON,"gesture":"touch"}"""),
        )
        assertEquals(
            DocumentIntent.Glance(ID_LINK, DocumentGesture.Hover),
            raised("""{"token":"$TOKEN","verb":"glance","link":$ID_LINK_JSON,"gesture":"hover"}"""),
        )
        assertEquals(
            DocumentIntent.Glance(ID_LINK, DocumentGesture.Focus),
            raised("""{"token":"$TOKEN","verb":"glance","link":$ID_LINK_JSON,"gesture":"focus"}"""),
        )
        assertEquals(
            DocumentIntent.Pin(ID_LINK),
            raised("""{"token":"$TOKEN","verb":"pin","link":$ID_LINK_JSON,"gesture":null}"""),
        )
        assertEquals(
            DocumentIntent.Go(FILE_LINK),
            raised("""{"token":"$TOKEN","verb":"go","link":$FILE_LINK_JSON,"gesture":null}"""),
        )
        assertEquals(
            DocumentIntent.Dismiss,
            raised("""{"token":"$TOKEN","verb":"dismiss","link":null,"gesture":null}"""),
        )
    }

    @Test
    fun onlyTheLocalOriginIsHeard() {
        for (origin in
            listOf(
                null,
                "",
                "https://elsewhere.invalid",
                "http://appassets.androidplatform.net",
                "https://appassets.androidplatform.net.elsewhere.invalid",
                "https://appassets.androidplatform.net:8443",
            )
        ) {
            assertEquals(
                "$origin was heard",
                DocumentRefusal.Origin,
                refusal(PIN, origin = origin),
            )
        }
        assertEquals(
            DocumentIntent.Pin(ID_LINK),
            (
                DocumentChannel.read("${DocumentOrigin.ORIGIN}/", true, PIN, TOKEN)
                    as DocumentEvent.Raised
            ).intent,
        )
    }

    @Test
    fun onlyTheMainFrameIsHeard() {
        assertEquals(DocumentRefusal.Frame, refusal(PIN, mainFrame = false))
    }

    @Test
    fun onlyTheMountedDocumentIsHeard() {
        assertEquals(DocumentRefusal.Binding, refusal(PIN, token = null))
        assertEquals(DocumentRefusal.Binding, refusal(PIN, token = OTHER_TOKEN))
        assertEquals(
            DocumentRefusal.Binding,
            refusal("""{"token":"$OTHER_TOKEN","verb":"pin","link":$ID_LINK_JSON}"""),
        )
        assertEquals(DocumentRefusal.Binding, refusal("""{"token":"","verb":"dismiss"}"""))
    }

    @Test
    fun aMessageThisChannelDoesNotCarryIsRefused() {
        val malformed =
            listOf(
                null,
                "",
                "not json at all",
                "[]",
                """{"verb":"pin","link":$ID_LINK_JSON}""",
                """{"token":"$TOKEN"}""",
                """{"token":"$TOKEN","verb":"pin","link":$ID_LINK_JSON,"script":"alert(1)"}""",
                """{"token":"$TOKEN","verb":"evaluate","link":$ID_LINK_JSON}""",
                """{"token":"$TOKEN","verb":"open","link":$ID_LINK_JSON}""",
                """{"token":"$TOKEN","verb":"pin","link":{"target":"a"}}""",
                """{"token":"$TOKEN","verb":"pin","link":{"target":"","reference":""}}""",
                """{"token":"$TOKEN","verb":"pin","link":{"id":"","target":"a","reference":"a"}}""",
                """{"token":"$TOKEN","verb":"pin"}""",
                """{"token":"$TOKEN","verb":"glance","link":$ID_LINK_JSON}""",
                """{"token":"$TOKEN","verb":"glance","link":$ID_LINK_JSON,"gesture":"drag"}""",
                """{"token":"$TOKEN","verb":"glance","gesture":"touch"}""",
                """{"token":"$TOKEN","verb":"pin","link":$ID_LINK_JSON,"gesture":"touch"}""",
                """{"token":"$TOKEN","verb":"dismiss","link":$ID_LINK_JSON}""",
                """{"token":"$TOKEN","verb":"dismiss","gesture":"touch"}""",
            )
        for (message in malformed) {
            assertEquals("$message was carried", DocumentRefusal.Shape, refusal(message))
        }
    }

    @Test
    fun aMessageLongerThanAGestureIsRefused() {
        val target = "a".repeat(DocumentChannel.MESSAGE_LIMIT)
        val flood =
            """{"token":"$TOKEN","verb":"pin","link":""" +
                """{"id":null,"target":"$target","reference":"$target"}}"""
        assertEquals(DocumentRefusal.Shape, refusal(flood))
    }

    private fun raised(message: String): DocumentIntent =
        (DocumentChannel.read(DocumentOrigin.ORIGIN, true, message, TOKEN) as DocumentEvent.Raised)
            .intent

    private fun refusal(
        message: String?,
        origin: String? = DocumentOrigin.ORIGIN,
        mainFrame: Boolean = true,
        token: String? = TOKEN,
    ): DocumentRefusal =
        (DocumentChannel.read(origin, mainFrame, message, token) as DocumentEvent.Refused).reason

    private companion object {
        const val TOKEN = "3f2a9c81-4d5e-4f60-9a1b-0c2d3e4f5061"
        const val OTHER_TOKEN = "8b7c6d5e-1a2b-4c3d-9e8f-0a1b2c3d4e5f"
        const val ID = "5c4b3a29-1d0e-4f5a-8b7c-6d5e4f3a2b19"

        val ID_LINK = DocumentLink(id = ID, target = "id:$ID", reference = "id:$ID")
        const val ID_LINK_JSON =
            """{"id":"$ID","target":"id:$ID","reference":"id:$ID"}"""

        val FILE_LINK =
            DocumentLink(
                id = null,
                target = "figures/diagram.png",
                reference = "figures/diagram.png",
            )
        const val FILE_LINK_JSON =
            """{"id":null,"target":"figures/diagram.png","reference":"figures/diagram.png"}"""

        const val PIN = """{"token":"$TOKEN","verb":"pin","link":$ID_LINK_JSON}"""
    }
}
