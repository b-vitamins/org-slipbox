/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

private val ROUTE_FORMAT = Json {
    encodeDefaults = false
    ignoreUnknownKeys = false
}

/** Decode entries independently so an invalid entry cannot discard valid neighbours. */
internal fun saveRoutes(routes: List<SlipboxRoute>): List<String> =
    routes.map { ROUTE_FORMAT.encodeToString(SlipboxRoute.serializer(), it) }

internal fun restoreRoutes(
    saved: List<String>,
    availability: DestinationAvailability,
    generations: SourceGenerations,
): List<SlipboxRoute> {
    val restored = mutableListOf<SlipboxRoute>()
    for (entry in saved) {
        val decoded = decodeRoute(entry) ?: continue
        restored.appendRoute(bindRoute(decoded, availability, generations) ?: continue)
    }
    return restored
}

internal fun bindRoute(
    route: SlipboxRoute,
    availability: DestinationAvailability,
    generations: SourceGenerations,
): SlipboxRoute? {
    if (!route.isCanonical()) return null
    if (!availability.presents(route.surface)) return null
    val reads = route.reads ?: return route
    val ready = generations.readyGeneration(reads.source) ?: return null
    if (!isGenerationIdentity(ready)) return null
    return route.readAt(GenerationBinding(reads.source, ready))
}

internal fun MutableList<SlipboxRoute>.appendRoute(route: SlipboxRoute) {
    if (isNotEmpty() && last().place == route.place) set(lastIndex, route) else add(route)
}

private fun decodeRoute(saved: String): SlipboxRoute? =
    try {
        ROUTE_FORMAT.decodeFromString(SlipboxRoute.serializer(), saved)
    } catch (_: SerializationException) {
        null
    }

private fun SlipboxRoute.readAt(binding: GenerationBinding): SlipboxRoute =
    when (this) {
        is SlipboxRoute.Reader -> copy(note = note.copy(binding = binding))
        is SlipboxRoute.Glossary -> copy(binding = binding)
        is SlipboxRoute.Library, is SlipboxRoute.SourceSettings -> this
        is SlipboxRoute.Connection, is SlipboxRoute.About -> this
    }
