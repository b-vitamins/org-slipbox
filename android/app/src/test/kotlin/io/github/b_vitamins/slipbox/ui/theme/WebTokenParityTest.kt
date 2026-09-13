/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import java.io.File
import org.junit.Assert.assertEquals
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
            "math-error",
            SlipboxTokens.Palette.MATH_ERROR_LIGHT,
            SlipboxTokens.Palette.MATH_ERROR_DARK,
        )
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

    private fun assertPair(css: String, token: String, light: Long, dark: Long) {
        val declared = value(css, token)
        val match =
            LIGHT_DARK.matchEntire(declared)
                ?: error("--$token is not a light-dark pair: $declared")
        assertEquals("--$token light", light, argb(match.groupValues[1]))
        assertEquals("--$token dark", dark, argb(match.groupValues[2]))
    }

    private fun assertLength(css: String, token: String, expected: Float) {
        assertEquals("--$token", expected, value(css, token).removeSuffix("px").toFloat(), 0f)
    }

    private fun value(css: String, token: String): String {
        val declarations = Regex("--$token:([^;]*);").findAll(css).toList()
        check(declarations.isNotEmpty()) { "--$token is not declared" }
        return declarations.last().groupValues[1].replace(Regex("\\s+"), " ").trim()
    }

    private fun argb(hex: String): Long = 0xFF000000L or hex.removePrefix("#").toLong(16)

    private fun webTokens(): String {
        val file =
            checkNotNull(webTokensFile()) {
                "$WEB_TOKENS was not found above ${File("").absolutePath}"
            }
        return file.readText()
    }

    private fun webTokensFile(): File? {
        var directory: File? = File("").absoluteFile
        while (directory != null) {
            val candidate = File(directory, WEB_TOKENS)
            if (candidate.isFile) return candidate
            directory = directory.parentFile
        }
        return null
    }

    private companion object {
        const val WEB_TOKENS = "crates/slipbox-web/client/src/styles/tokens.css"
        const val HEX = "(#[0-9a-fA-F]{6})"
        val LIGHT_DARK = Regex("light-dark\\(\\s*$HEX\\s*,\\s*$HEX\\s*\\)")
    }
}
