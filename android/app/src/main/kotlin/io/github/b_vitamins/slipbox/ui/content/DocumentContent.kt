/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import androidx.compose.runtime.Immutable

/** Exact source generation and document identity for later asset lookups. */
@Immutable
internal data class DocumentBinding(
    val source: String,
    val generation: String,
    val id: String,
    val filePath: String,
)

/** Org input; changing any field retires the mount token. */
@Immutable
internal data class DocumentSource(
    val source: String,
    val generation: String,
    val id: String,
    val filePath: String,
    val org: String,
    val baseLevel: Int = DEFAULT_BASE_LEVEL,
) {
    init {
        require(source.isNotBlank()) { "a document source carries no source identity" }
        require(generation.isNotBlank()) { "a document source carries no generation" }
        require(id.isNotBlank()) { "a document source carries no id" }
        require(filePath.isNotBlank()) { "a document source carries no file path" }
        require(baseLevel in HIGHEST_LEVEL..LOWEST_LEVEL) {
            "a document base level outside $HIGHEST_LEVEL..$LOWEST_LEVEL: $baseLevel"
        }
    }

    val binding: DocumentBinding get() = DocumentBinding(source, generation, id, filePath)

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

/** Stable rendered block plus bounded fallbacks for changed document geometry. */
@Immutable
internal data class DocumentPosition(
    val mark: String = "",
    val progress: Float = 0f,
    val offset: Float = 0f,
)

@Immutable
internal data class DocumentHeadingRequest(
    val index: Int,
    val serial: Long,
) {
    init {
        require(index in 0..10_000 && serial >= 0) { "heading request must be canonical" }
    }
}

/** Navigation requests only; the caller owns destinations and previews. */
internal sealed interface DocumentIntent {
    data class Glance(
        val link: DocumentLink,
        val gesture: DocumentGesture,
        val progress: Float = 0f,
        val origin: String,
        val position: DocumentPosition? = null,
    ) : DocumentIntent

    data class Pin(
        val link: DocumentLink,
        val progress: Float = 0f,
        val position: DocumentPosition? = null,
    ) : DocumentIntent

    data class Go(
        val link: DocumentLink,
        val progress: Float = 0f,
        val position: DocumentPosition? = null,
    ) : DocumentIntent

    data class Position(val position: DocumentPosition) : DocumentIntent

    data object Dismiss : DocumentIntent
}

/** One renderer-issued origin to refocus after its native preview closes. */
@Immutable
internal data class DocumentFocusRequest(val origin: String)

internal class DocumentAsset(val mimeType: String, val bytes: ByteArray)

/**
 * Resolves the verbatim Org target on the loader thread. The resolver must enforce
 * containment in [DocumentBinding]'s store; returning null reports a missing asset.
 */
internal fun interface DocumentAssetResolver {
    fun resolve(binding: DocumentBinding, target: String): DocumentAsset?
}
