/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.graphics.Bitmap
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.PixelMap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.MainTestClock
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntRect
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.Locale
import kotlin.math.max
import kotlin.math.min

/**
 * Where a device run leaves what it painted and measured: under the output directory
 * this run was given, which is the one taken off the device when the run ends.
 */
internal object Evidence {

    val directory: File by lazy {
        val arguments = InstrumentationRegistry.getArguments()
        val given =
            checkNotNull(arguments.getString(OUTPUT_ARGUMENT)) {
                "$OUTPUT_ARGUMENT is not among ${arguments.keySet().sorted()}"
            }
        File(given, "design").apply { mkdirs() }
    }

    fun image(name: String, image: ImageBitmap) {
        val bitmap = image.asAndroidBitmap()
        File(directory, "$name.png").outputStream().use { out ->
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)) {
                "$name could not be encoded"
            }
        }
    }

    fun record(name: String, record: Record) {
        File(directory, "$name.json").writeText(record.json())
    }

    /** The directory the instrumentation is told to leave its own output in. */
    private const val OUTPUT_ARGUMENT = "additionalTestOutputDir"
}

/** One ordered JSON object of measurements, written as one evidence file. */
internal class Record {

    private val fields = StringBuilder()

    fun text(name: String, value: String): Record = put(name, quote(value))

    fun size(name: String, value: Float): Record = put(name, number(value))

    fun count(name: String, value: Int): Record = put(name, value.toString())

    fun flag(name: String, value: Boolean): Record = put(name, value.toString())

    fun colour(name: String, value: Color): Record = put(name, quote(hex(value)))

    fun nested(name: String, value: Record): Record = put(name, value.json())

    /** A value already in JSON, such as an export a production type wrote itself. */
    fun raw(name: String, value: String): Record = put(name, value)

    fun json(): String = "{$fields}"

    private fun put(name: String, value: String): Record {
        if (fields.isNotEmpty()) fields.append(',')
        fields.append(quote(name)).append(':').append(value)
        return this
    }

    private companion object {
        fun quote(value: String): String =
            "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

        fun number(value: Float): String = String.format(Locale.ROOT, "%.2f", value)
    }
}

internal fun hex(colour: Color): String = String.format(Locale.ROOT, "#%08X", colour.toArgb())

/** A length in the pixels of one density: the unit a node and a capture measure in. */
internal fun Dp.pixels(density: Density): Float = value * density.density

/**
 * A node's own bounds, in pixels. A test that forces a density reads them here
 * rather than in the window's own density-independent pixels.
 */
internal data class Bounds(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
) {
    val width: Float
        get() = right - left

    val height: Float
        get() = bottom - top

    val middle: Float
        get() = (top + bottom) / 2

    /** The same bounds in the density-independent pixels they were laid out in. */
    fun dp(density: Density): Bounds =
        Bounds(
            left = left / density.density,
            top = top / density.density,
            right = right / density.density,
            bottom = bottom / density.density,
        )
}

/** Advance a caller-paused clock until [settled] or [limit] frames. */
internal fun MainTestClock.frames(limit: Int = FRAMES, settled: () -> Boolean): Int {
    var advanced = 0
    while (advanced < limit && !settled()) {
        advanceTimeByFrame()
        advanced++
    }
    return advanced
}

internal const val FRAMES = 32

internal fun SemanticsNodeInteraction.bounds(): Bounds {
    val node = fetchSemanticsNode()
    val position = node.positionInRoot
    return Bounds(
        left = position.x,
        top = position.y,
        right = position.x + node.size.width,
        bottom = position.y + node.size.height,
    )
}

/** The WCAG contrast ratio between two tones, as a reader meets them. */
internal fun contrast(one: Color, other: Color): Float {
    val first = one.luminance()
    val second = other.luminance()
    return (max(first, second) + 0.05f) / (min(first, second) + 0.05f)
}

internal fun ImageBitmap.pixels(): PixelMap = toPixelMap()

/** Whether two captures painted the same thing, pixel for pixel. */
internal fun PixelMap.matches(other: PixelMap): Boolean {
    if (width != other.width || height != other.height) return false
    for (y in 0 until height) {
        for (x in 0 until width) {
            if (this[x, y] != other[x, y]) return false
        }
    }
    return true
}

/**
 * The runs of pixels down column [x], as far as row [until], that are not [background]:
 * a rule crossing the column is one run, and its length is the thickness that was
 * actually painted.
 */
internal fun PixelMap.runsDown(x: Int, background: Color, until: Int = height): List<IntRange> {
    val runs = mutableListOf<IntRange>()
    var start = -1
    for (y in 0 until until) {
        val differs = this[x, y] != background
        if (differs && start < 0) start = y
        if (!differs && start >= 0) {
            runs += start until y
            start = -1
        }
    }
    if (start >= 0) runs += start until until
    return runs
}

/**
 * The bounds of everything painted over [background], which for one control is the
 * visual bounds of its mark rather than the bounds of the target around it.
 */
internal fun PixelMap.markBounds(background: Color): IntRect {
    var left = width
    var top = height
    var right = 0
    var bottom = 0
    for (y in 0 until height) {
        for (x in 0 until width) {
            if (this[x, y] == background) continue
            left = min(left, x)
            top = min(top, y)
            right = max(right, x + 1)
            bottom = max(bottom, y + 1)
        }
    }
    check(right > left && bottom > top) { "nothing was painted over the background" }
    return IntRect(left, top, right, bottom)
}

/** The darkest pixel inside a region, which is the ink of the text set in it. */
internal fun PixelMap.darkest(left: Int, top: Int, right: Int, bottom: Int): Color {
    var found = this[left, top]
    for (y in top until bottom) {
        for (x in left until right) {
            val candidate = this[x, y]
            if (candidate.luminance() < found.luminance()) found = candidate
        }
    }
    return found
}

/** The lightest pixel inside a region, which is the canvas the text is set on. */
internal fun PixelMap.lightest(left: Int, top: Int, right: Int, bottom: Int): Color {
    var found = this[left, top]
    for (y in top until bottom) {
        for (x in left until right) {
            val candidate = this[x, y]
            if (candidate.luminance() > found.luminance()) found = candidate
        }
    }
    return found
}
