/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WebTokenParityTest {

    @Test
    fun thePaletteIsTheSurfacesPalette() {
        val css = webTokens()
        assertPair(
            css,
            "paper",
            SlipboxTokens.Palette.PAPER_LIGHT,
            SlipboxTokens.Palette.PAPER_DARK,
        )
        assertPair(
            css,
            "surface",
            SlipboxTokens.Palette.SURFACE_LIGHT,
            SlipboxTokens.Palette.SURFACE_DARK,
        )
        assertPair(css, "ink", SlipboxTokens.Palette.INK_LIGHT, SlipboxTokens.Palette.INK_DARK)
        assertPair(
            css,
            "muted",
            SlipboxTokens.Palette.MUTED_LIGHT,
            SlipboxTokens.Palette.MUTED_DARK,
        )
        assertPair(
            css,
            "muted-2",
            SlipboxTokens.Palette.MUTED_2_LIGHT,
            SlipboxTokens.Palette.MUTED_2_DARK,
        )
        assertPair(
            css,
            "muted-3",
            SlipboxTokens.Palette.MUTED_3_LIGHT,
            SlipboxTokens.Palette.MUTED_3_DARK,
        )
        assertPair(
            css,
            "hairline",
            SlipboxTokens.Palette.HAIRLINE_LIGHT,
            SlipboxTokens.Palette.HAIRLINE_DARK,
        )
        assertPair(css, "link", SlipboxTokens.Palette.LINK_LIGHT, SlipboxTokens.Palette.LINK_DARK)
        assertPair(
            css,
            "link-visited",
            SlipboxTokens.Palette.LINK_VISITED_LIGHT,
            SlipboxTokens.Palette.LINK_VISITED_DARK,
        )
        assertPair(
            css,
            "math-error",
            SlipboxTokens.Palette.MATH_ERROR_LIGHT,
            SlipboxTokens.Palette.MATH_ERROR_DARK,
        )
    }

    /** The wash under a reveal is the tone the surface shades an edge with. */
    @Test
    fun theScrimIsTheSurfacesEdgeShadow() {
        val declared = value(webTokens(), "edge-shadow")
        val match =
            EDGE_SHADOW.matchEntire(declared) ?: error("--edge-shadow is not a pair: $declared")
        assertScrim("light", SlipboxTokens.Palette.SCRIM_LIGHT, match.groupValues[1])
        assertScrim("dark", SlipboxTokens.Palette.SCRIM_DARK, match.groupValues[2])
    }

    @Test
    fun theTypeScaleIsTheSurfacesTypeScale() {
        val css = webTokens()
        assertLength(css, "text-size", SlipboxTokens.Type.BODY_SIZE_SP)
        assertLength(css, "text-line", SlipboxTokens.Type.BODY_LINE_SP)
        assertLength(css, "h1-size", SlipboxTokens.Type.H1_SIZE_SP)
        assertLength(css, "h1-line", SlipboxTokens.Type.H1_LINE_SP)
        assertLength(css, "h2-size", SlipboxTokens.Type.H2_SIZE_SP)
        assertLength(css, "h2-line", SlipboxTokens.Type.H2_LINE_SP)
        assertLength(css, "code-size", SlipboxTokens.Type.CODE_SIZE_SP)
        assertLength(css, "code-line", SlipboxTokens.Type.CODE_LINE_SP)
        assertEquals(
            "letter spacing",
            SlipboxTokens.Type.LETTER_SPACING_EM,
            value(css, "text-letter-spacing").removeSuffix("em").toFloat(),
            0f,
        )
    }

    @Test
    fun theGeometryIsTheSurfacesGeometry() {
        val css = webTokens()
        assertLength(css, "column-width", SlipboxTokens.Geometry.READING_MEASURE_DP)
        assertLength(css, "note-padding-mobile", SlipboxTokens.Geometry.READING_PADDING_DP)
        assertLength(css, "header-padding-y", SlipboxTokens.Geometry.HEADER_PADDING_Y_DP)
        assertLength(css, "header-padding-x", SlipboxTokens.Geometry.HEADER_PADDING_X_DP)
    }

    /** The one metric that departs: a native control answers to the platform's floor. */
    @Test
    fun theTouchFloorIsRaisedAboveTheSurfacesOwn() {
        val coarse = value(webTokens(), "touch-target").removeSuffix("px").toFloat()
        assertEquals("--touch-target", 44f, coarse, 0f)
        assertTrue(
            "the native floor is at least the surface's",
            SlipboxTokens.Geometry.TOUCH_TARGET_DP >= coarse,
        )
    }

    /**
     * Each duration stands at its declared length, and the surface re-declares every
     * one of them as none for a reader who asks for less motion.
     */
    @Test
    fun theMotionIsTheSurfacesMotion() {
        val css = webTokens()
        val durations =
            mapOf(
                "dur-column" to SlipboxTokens.Motion.COLUMN_MS,
                "dur-shadow" to SlipboxTokens.Motion.SHADOW_MS,
                "dur-opacity" to SlipboxTokens.Motion.OPACITY_MS,
                "dur-crossfade" to SlipboxTokens.Motion.CROSSFADE_MS,
            )
        for ((token, expected) in durations) {
            assertEquals("--$token", expected, standing(css, token).removeSuffix("ms").toInt())
            assertEquals("--$token under a refusal of motion", "0ms", value(css, token))
        }
    }

    @Test
    fun theSettleIsTheSurfacesEasing() {
        val declared = value(webTokens(), "ease-settle")
        val match =
            BEZIER.matchEntire(declared) ?: error("--ease-settle is not a bezier: $declared")
        val points =
            listOf(
                SlipboxTokens.Motion.SETTLE_X1,
                SlipboxTokens.Motion.SETTLE_Y1,
                SlipboxTokens.Motion.SETTLE_X2,
                SlipboxTokens.Motion.SETTLE_Y2,
            )
        points.forEachIndexed { index, expected ->
            assertEquals(
                "--ease-settle point ${index + 1}",
                expected,
                match.groupValues[index + 1].toFloat(),
                0f,
            )
        }
    }

    @Test
    fun theCodeToneIsTheOneTheRendererFallsBackTo() {
        val css = orgStyles()
        for (selector in listOf(".org-verbatim", ".org-src", ".org-example")) {
            assertEquals(
                "$selector background",
                "var(--code-surface, var(--paper))",
                declaration(css, selector, "background"),
            )
        }
        for (sheet in listOf(webTokens(), css)) {
            assertFalse(
                "--code-surface is declared, so the fallback would not be what applies",
                CODE_SURFACE.containsMatchIn(sheet.replace(COMMENT, "")),
            )
        }
        assertPair(
            webTokens(),
            "paper",
            SlipboxTokens.Palette.PAPER_LIGHT,
            SlipboxTokens.Palette.PAPER_DARK,
        )
    }

    /**
     * The sizes and corner the renderer sets in its own stylesheet rather than in a
     * shared token, which a host must therefore carry itself to scale them.
     */
    @Test
    fun theRenderersOwnSizesAreTheOnesItSets() {
        val css = orgStyles()
        assertPixels(css, ".org-src__lang", "font-size", SlipboxTokens.Type.CHROME_SIZE_SP)
        assertPixels(css, ".org-src__copy", "font-size", SlipboxTokens.Type.CHROME_SIZE_SP)
        assertPixels(css, ".org-table", "font-size", SlipboxTokens.Type.TABLE_SIZE_SP)
        assertPixels(css, ".org-src", "border-radius", SlipboxTokens.Geometry.BLOCK_CORNER_DP)
    }

    private fun assertPair(css: String, token: String, light: Long, dark: Long) {
        val declared = value(css, token)
        val match =
            LIGHT_DARK.matchEntire(declared)
                ?: error("--$token is not a light-dark pair: $declared")
        assertEquals("--$token light", light, argb(match.groupValues[1]))
        assertEquals("--$token dark", dark, argb(match.groupValues[2]))
    }

    private fun assertScrim(scheme: String, expected: Long, declared: String) {
        val parts =
            declared.removePrefix("rgba(").removeSuffix(")").split(',').map { part -> part.trim() }
        check(parts.size == 4) { "$declared is not an rgba colour" }
        assertEquals("$scheme scrim red", expected shr 16 and 0xFF, parts[0].toLong())
        assertEquals("$scheme scrim green", expected shr 8 and 0xFF, parts[1].toLong())
        assertEquals("$scheme scrim blue", expected and 0xFF, parts[2].toLong())
        // An eight-bit channel cannot hold every fraction the surface can declare.
        assertEquals(
            "$scheme scrim alpha",
            parts[3].toFloat(),
            (expected shr 24 and 0xFF) / 255f,
            1f / 255f,
        )
    }

    private fun assertLength(css: String, token: String, expected: Float) {
        assertEquals("--$token", expected, value(css, token).removeSuffix("px").toFloat(), 0f)
    }

    private fun assertPixels(css: String, selector: String, property: String, expected: Float) {
        val declared = declaration(css, selector, property)
        assertEquals("$selector $property", expected, declared.removeSuffix("px").toFloat(), 0f)
    }

    /**
     * The one declaration of [property] among the rules [selector] names. Two of them
     * would leave which one applies to cascade order, so both are refused.
     */
    private fun declaration(css: String, selector: String, property: String): String {
        val declarations =
            RULE.findAll(css.replace(COMMENT, ""))
                .filter { rule ->
                    rule.groupValues[1].split(',').any { part -> part.trim() == selector }
                }
                .mapNotNull { rule ->
                    Regex("(?:^|;)\\s*$property\\s*:([^;]*)")
                        .find(rule.groupValues[2])
                        ?.groupValues
                        ?.get(1)
                        ?.trim()
                }
                .toList()
        check(declarations.size == 1) {
            "$selector declares $property ${declarations.size} time(s)"
        }
        return declarations.single()
    }

    /** The last declaration of the token, which is what any query re-declaring it leaves. */
    private fun value(css: String, token: String): String = values(css, token).last()

    /** What the token stands at before any query re-declares it. */
    private fun standing(css: String, token: String): String = values(css, token).first()

    private fun values(css: String, token: String): List<String> {
        val declarations = Regex("--$token:([^;]*);").findAll(css).toList()
        check(declarations.isNotEmpty()) { "--$token is not declared" }
        return declarations.map { it.groupValues[1].replace(Regex("\\s+"), " ").trim() }
    }

    private fun argb(hex: String): Long = 0xFF000000L or hex.removePrefix("#").toLong(16)

    private fun webTokens(): String = webSource(WEB_TOKENS)

    private fun orgStyles(): String = webSource(ORG_STYLES)

    private fun webSource(path: String): String {
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
        const val WEB_TOKENS = "crates/slipbox-web/client/src/styles/tokens.css"
        const val ORG_STYLES = "crates/slipbox-web/client/src/org/org.css"
        const val HEX = "(#[0-9a-fA-F]{6})"
        const val NUMBER = "(-?[0-9.]+)"
        val LIGHT_DARK = Regex("light-dark\\(\\s*$HEX\\s*,\\s*$HEX\\s*\\)")
        const val RGBA = "(rgba\\([^)]*\\))"
        val EDGE_SHADOW = Regex("light-dark\\(\\s*$RGBA\\s*,\\s*$RGBA\\s*\\)")
        val BEZIER =
            Regex(
                "cubic-bezier\\(\\s*$NUMBER\\s*,\\s*$NUMBER\\s*,\\s*$NUMBER\\s*,\\s*$NUMBER\\s*\\)",
            )
        val COMMENT = Regex("/\\*.*?\\*/", RegexOption.DOT_MATCHES_ALL)
        val RULE = Regex("([^{}]+)\\{([^{}]*)\\}")
        val CODE_SURFACE = Regex("--code-surface\\s*:")
    }
}
