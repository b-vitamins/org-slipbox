/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentHostPageTest {

    @Test
    fun thePagePermitsItsOwnBundleAndNothingElse() {
        val declared = policy()
        assertEquals("default-src", listOf("'none'"), declared["default-src"])
        assertEquals("script-src", listOf("'self'"), declared["script-src"])
        assertEquals("font-src", listOf("'self'"), declared["font-src"])
        assertEquals("img-src", listOf("'self'"), declared["img-src"])
        assertEquals("connect-src", listOf("'none'"), declared["connect-src"])
        assertEquals("base-uri", listOf("'none'"), declared["base-uri"])
        assertEquals("form-action", listOf("'none'"), declared["form-action"])
        assertEquals("style-src", listOf("'self'", "'unsafe-inline'"), declared["style-src"])
    }

    @Test
    fun thePageLoadsOnlyWhatIsPackagedBesideIt() {
        val page = source(PAGE)
        val referenced = REFERENCE.findAll(page).map { it.groupValues[2] }.toList()
        assertEquals(
            listOf("./document.css", "./host.css", "./host.js"),
            referenced.sorted(),
        )
        assertFalse("the page names the renderer's own host page", page.contains("host.html"))
        assertFalse("the page names an absolute URL", ABSOLUTE.containsMatchIn(withoutPolicy(page)))
        assertTrue(
            "a script element carries a body",
            SCRIPT.findAll(page).all { it.groupValues[1].isBlank() },
        )
    }

    @Test
    fun theHostScriptEvaluatesNothing() {
        val script = source(SCRIPT_FILE)
        for (construct in
            listOf(
                "eval(",
                "new Function",
                "innerHTML",
                "outerHTML",
                "insertAdjacentHTML",
                "document.write",
                "setTimeout(\"",
                "javascript:",
            )
        ) {
            assertFalse("the host script uses $construct", script.contains(construct))
        }
        assertEquals(
            listOf("./document.js"),
            IMPORT.findAll(script).map { it.groupValues[1] }.toList(),
        )
        assertFalse("the host script names an absolute URL", ABSOLUTE.containsMatchIn(script))
    }

    @Test
    fun theHostStylesheetFetchesNothing() {
        val css = source(STYLE_FILE)
        assertFalse("the host stylesheet imports", css.contains("@import"))
        assertFalse("the host stylesheet names an absolute URL", ABSOLUTE.containsMatchIn(css))
    }

    private fun policy(): Map<String, List<String>> {
        val page = source(PAGE)
        val match = checkNotNull(POLICY.find(page)) { "the page declares no content policy" }
        return match
            .groupValues[1]
            .split(';')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
            .associate { directive ->
                val parts = directive.split(Regex("\\s+"))
                parts.first() to parts.drop(1)
            }
    }

    private fun withoutPolicy(page: String): String = POLICY.replace(page, "")

    private fun source(path: String): String {
        val here = File("").absolutePath
        return checkNotNull(above(path)) { "$path was not found above $here" }.readText()
    }

    private fun above(path: String): File? {
        var directory: File? = File("").absoluteFile
        while (directory != null) {
            val candidate = File(directory, path)
            if (candidate.isFile) return candidate
            directory = directory.parentFile
        }
        return null
    }

    private companion object {
        const val ASSETS = "android/app/src/main/assets/document"
        const val PAGE = "$ASSETS/index.html"
        const val SCRIPT_FILE = "$ASSETS/host.js"
        const val STYLE_FILE = "$ASSETS/host.css"

        val POLICY =
            Regex(
                "<meta\\s+http-equiv=\"Content-Security-Policy\"\\s+content=\"([^\"]*)\"\\s*/?>",
            )
        val REFERENCE = Regex("(href|src)=\"([^\"]*)\"")
        val SCRIPT = Regex("<script[^>]*>(.*?)</script>", RegexOption.DOT_MATCHES_ALL)
        val IMPORT = Regex("import\\(\"([^\"]*)\"\\)")
        val ABSOLUTE = Regex("(https?:)?//[a-zA-Z0-9]")
    }
}
