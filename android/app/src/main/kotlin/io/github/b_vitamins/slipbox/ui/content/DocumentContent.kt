/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import androidx.compose.runtime.Immutable

/** Caller-provided source-scoped document identity and generation for asset lookups. */
@Immutable
internal data class DocumentBinding(val id: String, val generation: Long)

/** Org input; changing any field retires the mount token. */
@Immutable
internal data class DocumentSource(
    val id: String,
    val generation: Long,
    val org: String,
    val baseLevel: Int = DEFAULT_BASE_LEVEL,
) {
    init {
        require(id.isNotBlank()) { "a document source carries no id" }
        require(generation >= 0) { "a document source generation is negative: $generation" }
        require(baseLevel in HIGHEST_LEVEL..LOWEST_LEVEL) {
            "a document base level outside $HIGHEST_LEVEL..$LOWEST_LEVEL: $baseLevel"
        }
    }

    val binding: DocumentBinding get() = DocumentBinding(id, generation)

    internal companion object {
        const val DEFAULT_BASE_LEVEL = 2
        const val HIGHEST_LEVEL = 1
        const val LOWEST_LEVEL = 6
    }
}

/** Link fields retain the renderer's spelling; [reference] resolves an ID when present. */
@Immutable
internal data class DocumentLink(
    val id: String?,
    val target: String,
    val reference: String,
)

internal enum class DocumentGesture(val verb: String) {
    Hover("hover"),
    Focus("focus"),
    Touch("touch"),
    ;

    internal companion object {
        fun of(verb: String?): DocumentGesture? = entries.firstOrNull { it.verb == verb }
    }
}

/** Navigation requests only; the caller owns destinations and previews. */
internal sealed interface DocumentIntent {
    data class Glance(val link: DocumentLink, val gesture: DocumentGesture) : DocumentIntent

    data class Pin(val link: DocumentLink) : DocumentIntent

    data class Go(val link: DocumentLink) : DocumentIntent

    data object Dismiss : DocumentIntent
}

internal class DocumentAsset(val mimeType: String, val bytes: ByteArray)

/**
 * Resolves the verbatim Org target on the loader thread. The resolver must enforce
 * containment in [DocumentBinding]'s store; returning null reports a missing asset.
 */
internal fun interface DocumentAssetResolver {
    fun resolve(binding: DocumentBinding, target: String): DocumentAsset?
}
