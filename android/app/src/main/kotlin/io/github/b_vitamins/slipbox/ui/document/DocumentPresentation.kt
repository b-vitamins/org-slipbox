/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.document

import androidx.compose.runtime.Immutable
import java.util.Locale

internal enum class DocumentTheme(val option: String) {
    Light("light"),
    Dark("dark"),
}

/**
 * CSS-pixel metrics with platform font scaling already applied. The host must map
 * one CSS pixel to one density-independent pixel without further text scaling.
 */
@Immutable
internal data class DocumentTypography(
    val bodySize: Float,
    val bodyLine: Float,
    val h1Size: Float,
    val h1Line: Float,
    val h2Size: Float,
    val h2Line: Float,
    val codeSize: Float,
    val codeLine: Float,
    val chromeSize: Float,
    val tableSize: Float,
    val letterSpacingEm: Float,
)

/** The document's metrics, in CSS pixels. */
@Immutable
internal data class DocumentGeometry(
    val columnWidth: Float,
    val padding: Float,
    val touchTarget: Float,
    val availableWidth: Float,
)

/** Millisecond durations with platform animation scaling already applied. */
@Immutable
internal data class DocumentMotion(
    val columnMs: Int,
    val shadowMs: Int,
    val opacityMs: Int,
    val crossfadeMs: Int,
    val reduced: Boolean,
)

/** Per-mount presentation; apply [cssVariables] to its container, not the host page. */
@Immutable
internal data class DocumentPresentation(
    val theme: DocumentTheme,
    val typography: DocumentTypography,
    val geometry: DocumentGeometry,
    val motion: DocumentMotion,
) {
    fun cssVariables(): Map<String, String> =
        linkedMapOf(
            "--text-size" to px(typography.bodySize),
            "--text-line" to px(typography.bodyLine),
            "--text-letter-spacing" to em(typography.letterSpacingEm),
            "--h1-size" to px(typography.h1Size),
            "--h1-line" to px(typography.h1Line),
            "--h2-size" to px(typography.h2Size),
            "--h2-line" to px(typography.h2Line),
            "--code-size" to px(typography.codeSize),
            "--code-line" to px(typography.codeLine),
            // The renderer's own fixed sizes, which no shared token carries.
            "--slipbox-chrome-size" to px(typography.chromeSize),
            "--slipbox-table-size" to px(typography.tableSize),
            "--column-width" to px(geometry.columnWidth),
            "--note-padding" to px(geometry.padding),
            "--touch-target" to px(geometry.touchTarget),
            "--dur-column" to ms(motion.columnMs),
            "--dur-shadow" to ms(motion.shadowMs),
            "--dur-opacity" to ms(motion.opacityMs),
            "--dur-crossfade" to ms(motion.crossfadeMs),
        )

    fun toJson(): String =
        buildString {
            append("{\"theme\":\"").append(theme.option).append("\",")
            append("\"type\":{")
            append("\"bodySize\":").append(number(typography.bodySize)).append(',')
            append("\"bodyLine\":").append(number(typography.bodyLine)).append(',')
            append("\"h1Size\":").append(number(typography.h1Size)).append(',')
            append("\"h1Line\":").append(number(typography.h1Line)).append(',')
            append("\"h2Size\":").append(number(typography.h2Size)).append(',')
            append("\"h2Line\":").append(number(typography.h2Line)).append(',')
            append("\"codeSize\":").append(number(typography.codeSize)).append(',')
            append("\"codeLine\":").append(number(typography.codeLine)).append(',')
            append("\"chromeSize\":").append(number(typography.chromeSize)).append(',')
            append("\"tableSize\":").append(number(typography.tableSize)).append(',')
            append("\"letterSpacingEm\":").append(fine(typography.letterSpacingEm))
            append("},\"geometry\":{")
            append("\"columnWidth\":").append(number(geometry.columnWidth)).append(',')
            append("\"padding\":").append(number(geometry.padding)).append(',')
            append("\"touchTarget\":").append(number(geometry.touchTarget)).append(',')
            append("\"availableWidth\":").append(number(geometry.availableWidth))
            append("},\"motion\":{")
            append("\"columnMs\":").append(motion.columnMs).append(',')
            append("\"shadowMs\":").append(motion.shadowMs).append(',')
            append("\"opacityMs\":").append(motion.opacityMs).append(',')
            append("\"crossfadeMs\":").append(motion.crossfadeMs).append(',')
            append("\"reduced\":").append(motion.reduced)
            append("},\"css\":{")
            cssVariables().entries.forEachIndexed { index, (name, value) ->
                if (index > 0) append(',')
                append('"').append(name).append("\":\"").append(value).append('"')
            }
            append("}}")
        }

    private companion object {
        fun number(value: Float): String = String.format(Locale.ROOT, "%.2f", value)

        fun fine(value: Float): String = String.format(Locale.ROOT, "%.4f", value)

        fun px(value: Float): String = number(value) + "px"

        fun em(value: Float): String = fine(value) + "em"

        fun ms(value: Int): String = "${value}ms"
    }
}
