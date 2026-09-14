/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.DocumentTheme
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentPayloadTest {

    @Test
    fun aSourceThatSpellsScriptCrossesAsText() {
        val payload = DocumentPayload.of(mount(HOSTILE), presentation())
        assertFalse("a script element closes inside the payload", payload.contains("</script>"))
        assertFalse("a bare < crosses", payload.contains("<"))
        assertFalse("a line ends inside the payload", payload.contains("\n"))
        assertFalse("a carriage return crosses", payload.contains("\r"))
        assertFalse("U+2028 ends a line for an older engine", payload.contains("\u2028"))
        assertFalse("U+2029 ends a line for an older engine", payload.contains("\u2029"))
        assertEquals(HOSTILE, sourceOf(payload))
    }

    @Test
    fun aSourceCrossesUnchangedToTheLastCharacter() {
        for (source in
            listOf(
                HOSTILE,
                " ",
                "* Heading\n\nA \\emph{TeX} backslash and a \"quote\".",
                "#+begin_src js\nconst end = \"</script>\";\n#+end_src\n",
                "\u2028\u2029\uD83D\uDE42",
            )
        ) {
            val payload = DocumentPayload.of(mount(source), presentation())
            assertEquals(source, sourceOf(payload))
        }
    }

    @Test
    fun aPresentationCallIsOneStatementOnOneLine() {
        val call = DocumentPayload.present(mount(HOSTILE), presentation())
        assertTrue(call.startsWith("if (window.slipboxHost) window.slipboxHost.present({"))
        assertTrue(call.endsWith("});"))
        assertEquals("the call spans more than one line", -1, call.indexOf('\n'))
    }

    @Test
    fun theAssetBaseIsBoundToTheMountThatIssuedIt() {
        val mount = mount(HOSTILE)
        val payload = decode(DocumentPayload.of(mount, presentation()))
        assertEquals(mount.token, payload["token"]?.jsonPrimitive?.content)
        assertEquals(
            DocumentOrigin.assetBase(mount.token),
            payload["assetBase"]?.jsonPrimitive?.content,
        )
        val source = payload["source"]?.jsonObject
        assertEquals(mount.source.id, source?.get("id")?.jsonPrimitive?.content)
        assertEquals("7", source?.get("generation")?.jsonPrimitive?.content)
        assertEquals("2", source?.get("baseLevel")?.jsonPrimitive?.content)
    }

    @Test
    fun thePresentationCrossesWithTheSource() {
        val presentation = presentation(dark = true, fontScale = 2f)
        val declared =
            decode(DocumentPayload.of(mount(HOSTILE), presentation))["presentation"]?.jsonObject
        assertEquals("dark", declared?.get("theme")?.jsonPrimitive?.content)
        val css = declared?.get("css")?.jsonObject
        for ((name, value) in presentation.cssVariables()) {
            assertEquals(name, value, css?.get(name)?.jsonPrimitive?.content)
        }
    }

    private fun decode(payload: String) = Json.parseToJsonElement(payload).jsonObject

    private fun sourceOf(payload: String): String =
        decode(payload)["source"]?.jsonObject?.get("org")?.jsonPrimitive?.content.orEmpty()

    private fun mount(source: String): DocumentMount =
        DocumentMount(
            token = "3f2a9c81-4d5e-4f60-9a1b-0c2d3e4f5061",
            source = DocumentSource(id = "note-1", generation = 7, org = source),
        )

    private fun presentation(dark: Boolean = false, fontScale: Float = 1f): DocumentPresentation =
        documentPresentation(
            density = Density(3f, fontScale),
            dark = dark,
            motion = SlipboxMotion(),
            availableWidth = 411.dp,
        ).also {
            assertEquals(if (dark) DocumentTheme.Dark else DocumentTheme.Light, it.theme)
        }

    private companion object {
        const val HOSTILE =
            "* A heading with </script><script>fetch(\"https://elsewhere.invalid\")</script>\n" +
                "A \"quoted\" \\backslash, a tab\there, a \u0007 bell.\u2028\u2029"
    }
}
