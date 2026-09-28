/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import io.github.b_vitamins.slipbox.engine.GenerationBinding

/** Ready generations bind new/restored routes; existing entries keep their binding. */
internal fun interface SourceGenerations {
    fun readyGeneration(source: String): String?

    companion object {
        val None = SourceGenerations { null }
    }
}

/** Shared history for destination callbacks, system Back and predictive Back. */
@Stable
internal class SlipboxBackStack(
    restored: List<SlipboxRoute>,
    private val availability: DestinationAvailability,
    private val generations: SourceGenerations,
) {
    private val routes = mutableStateListOf(SlipboxRoute.Start).apply {
        for (route in restored) appendRoute(route)
    }

    private var attached = true

    val entries: List<SlipboxRoute> get() = routes

    val current: SlipboxRoute get() = routes.last()

    fun open(route: SlipboxRoute): Boolean {
        if (!attached) return false
        val bound = bindRoute(route, availability, generations) ?: return false
        routes.appendRoute(bound)
        return true
    }

    /** Save the live reader position if [origin] is still the presented route. */
    fun rememberReadingPlace(origin: SlipboxRoute.Reader, anchor: ReadingAnchor): Boolean {
        if (!attached || !anchor.isCanonical()) return false
        val presented = routes.lastOrNull() as? SlipboxRoute.Reader ?: return false
        if (presented.place != origin.place || presented.note.binding != origin.note.binding) {
            return false
        }
        routes[routes.lastIndex] = presented.copy(anchor = anchor)
        return true
    }

    /** Follow within the origin's immutable generation and rewind an existing path entry. */
    fun follow(
        origin: SlipboxRoute.Reader,
        targetNodeKey: String,
        originAnchor: ReadingAnchor,
    ): Boolean {
        if (!rememberReadingPlace(origin, originAnchor)) return false
        if (!availability.presents(SlipboxSurface.Reader)) return false
        val target =
            SlipboxRoute.Reader(
                note = BoundNote(origin.note.binding, targetNodeKey),
                anchor = ReadingAnchor.Start,
            )
        if (!target.isCanonical()) return false

        val existing =
            routes.indexOfLast { route ->
                route.place == target.place && route.reads == target.reads
            }
        if (existing >= 0) {
            while (routes.lastIndex > existing) routes.removeAt(routes.lastIndex)
        } else {
            routes.add(target)
        }
        return true
    }

    fun back(): Boolean {
        if (!attached || routes.size <= 1) return false
        routes.removeAt(routes.lastIndex)
        return true
    }

    /** Drop reads and unbound search state that belonged to the prior active source. */
    fun switchSource(to: GenerationBinding): Boolean {
        if (!attached || !to.isCanonical()) return false
        var changed = false
        for (index in routes.indices) {
            val entry = routes[index]
            if (entry is SlipboxRoute.Library && entry.query.isNotEmpty()) {
                routes[index] = entry.copy(query = "")
                changed = true
            }
        }
        return routes.retainAll { entry ->
            val read = entry.reads
            read == null || read.source == to.source
        } || changed
    }

    /** Remove reading/search state and, unless retained to report cleanup, source settings. */
    fun removeSource(
        source: String,
        keepSettings: Boolean = false,
        clearSearch: Boolean = true,
    ): Boolean {
        if (!attached) return false
        var changed = false
        if (clearSearch) {
            for (index in routes.indices) {
                val entry = routes[index]
                if (entry is SlipboxRoute.Library && entry.query.isNotEmpty()) {
                    routes[index] = entry.copy(query = "")
                    changed = true
                }
            }
        }
        return routes.retainAll { entry ->
            !entry.names(source) || (keepSettings && entry is SlipboxRoute.SourceSettings)
        } || changed
    }

    internal fun detach() {
        attached = false
    }
}

/** Providers are captured for this owner's lifetime. */
@Composable
internal fun rememberSlipboxBackStack(
    availability: DestinationAvailability,
    generations: SourceGenerations = SourceGenerations.None,
): SlipboxBackStack {
    val backStack = rememberSaveable(
        saver = listSaver<SlipboxBackStack, String>(
            save = { saveRoutes(it.entries) },
            restore = {
                SlipboxBackStack(
                    restored = restoreRoutes(it, availability, generations),
                    availability = availability,
                    generations = generations,
                )
            },
        ),
    ) {
        SlipboxBackStack(emptyList(), availability, generations)
    }
    DisposableEffect(backStack) { onDispose(backStack::detach) }
    return backStack
}
