/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import kotlinx.serialization.Serializable

private const val SOURCE_IDENTITY_CHARS = 32

private const val MAX_GENERATION_CHARS = 64

private const val MAX_KEY_CHARS = 1024

private const val READING_REFERENCE = "reading_reference"

/** A source-scoped note key bound to the generation that answered for it. */
@Serializable
internal data class BoundNote(val binding: GenerationBinding, val nodeKey: String) {

    /** Reading-state identity, stable across generation replacement. */
    val reference: String
        get() = "${binding.source}:$READING_REFERENCE:$nodeKey"
}

/** Restorable mark and normalized viewport position, without note content. */
@Serializable
internal data class ReadingAnchor(
    /** An empty mark names the beginning of the note. */
    val mark: String = "",
    /** Normalized viewport position, from zero to one. */
    val progress: Float = 0f,
) {
    internal companion object {
        val Start = ReadingAnchor()
    }
}

internal fun isSourceIdentity(value: String): Boolean =
    value.length == SOURCE_IDENTITY_CHARS &&
        value.all { it in '0'..'9' || it in 'a'..'f' }

internal fun isGenerationIdentity(value: String): Boolean =
    value.length in 1..MAX_GENERATION_CHARS &&
        value.all { it.isAsciiAlphanumeric() || it == '.' || it == '_' || it == '-' }

internal fun GenerationBinding.isCanonical(): Boolean =
    isSourceIdentity(source) && isGenerationIdentity(generation)

internal fun isKeyText(value: String): Boolean = value.isNotEmpty() && isSavedText(value)

internal fun isSavedText(value: String): Boolean =
    value.length <= MAX_KEY_CHARS && value.isPrintable()

internal fun ReadingAnchor.isCanonical(): Boolean =
    isSavedText(mark) && progress in 0f..1f

private fun Char.isAsciiAlphanumeric(): Boolean =
    this in '0'..'9' || this in 'a'..'z' || this in 'A'..'Z'

private fun String.isPrintable(): Boolean =
    none { it.isISOControl() } &&
        none { it.isWhitespace() && it != ' ' } &&
        trim() == this
