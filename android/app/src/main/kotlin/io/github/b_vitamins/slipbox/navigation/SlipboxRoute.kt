/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */
@file:OptIn(ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

/** Saved route discriminator; keep stable across releases. */
internal const val ROUTE_DISCRIMINATOR = "route"

internal enum class SlipboxSurface {
    Library,
    Reader,
    Glossary,
    SourceSettings,
    Connection,
    About,
}

/** Source-bound destinations and restorable reading/search state. */
@Serializable
@JsonClassDiscriminator(ROUTE_DISCRIMINATOR)
internal sealed interface SlipboxRoute {

    val surface: SlipboxSurface

    /** Presentation identity, excluding reading position and search text. */
    val place: String

        val reads: GenerationBinding?
        get() = null

        @Serializable
    @SerialName("library")
    data class Library(val query: String = "") : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.Library
        override val place: String get() = "library"
    }

        @Serializable
    @SerialName("reader")
    data class Reader(
        val note: BoundNote,
        val anchor: ReadingAnchor = ReadingAnchor.Start,
    ) : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.Reader
        override val place: String get() = "reader:${note.reference}"
        override val reads: GenerationBinding get() = note.binding
    }

        @Serializable
    @SerialName("glossary")
    data class Glossary(
        val binding: GenerationBinding,
        val term: String? = null,
        val query: String = "",
    ) : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.Glossary
        override val place: String get() = "glossary:${binding.source}:${term.orEmpty()}"
        override val reads: GenerationBinding get() = binding
    }

        @Serializable
    @SerialName("source-settings")
    data class SourceSettings(val source: String) : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.SourceSettings
        override val place: String get() = "source-settings:$source"
    }

        @Serializable
    @SerialName("connection")
    data class Connection(val source: String? = null) : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.Connection
        override val place: String get() = "connection:${source.orEmpty()}"
    }

    @Serializable
    @SerialName("about")
    data object About : SlipboxRoute {
        override val surface: SlipboxSurface get() = SlipboxSurface.About
        override val place: String get() = "about"
    }

    companion object {
                val Start: SlipboxRoute get() = Library()
    }
}

internal fun SlipboxRoute.isCanonical(): Boolean =
    when (this) {
        is SlipboxRoute.Library -> isSavedText(query)
        is SlipboxRoute.Reader ->
            note.binding.isCanonical() && isKeyText(note.nodeKey) && anchor.isCanonical()

        is SlipboxRoute.Glossary ->
            binding.isCanonical() && (term == null || isKeyText(term)) && isSavedText(query)

        is SlipboxRoute.SourceSettings -> isSourceIdentity(source)
        is SlipboxRoute.Connection -> source == null || isSourceIdentity(source)
        is SlipboxRoute.About -> true
    }

internal fun SlipboxRoute.names(source: String): Boolean =
    when (this) {
        is SlipboxRoute.Reader -> note.binding.source == source
        is SlipboxRoute.Glossary -> binding.source == source
        is SlipboxRoute.SourceSettings -> this.source == source
        is SlipboxRoute.Connection -> this.source == source
        is SlipboxRoute.Library, is SlipboxRoute.About -> false
    }
