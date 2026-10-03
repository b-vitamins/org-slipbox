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
import io.github.b_vitamins.slipbox.engine.NodeRecord

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
    private val trails: ReadingTrailSink = ReadingTrailSink.None,
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
        (bound as? SlipboxRoute.Reader)?.let { persist(it.note.binding.source) }
        return true
    }

    /** Persist the live library query while retaining the same back-stack entry. */
    fun rememberLibrarySearch(origin: SlipboxRoute.Library, query: String): Boolean {
        if (!attached || !isSavedText(query)) return false
        val presented = routes.lastOrNull() as? SlipboxRoute.Library ?: return false
        if (presented.place != origin.place) return false
        if (presented.query == query) return true
        routes[routes.lastIndex] = presented.copy(query = query)
        return true
    }

    /** Persist the glossary mode and due filter without adding a history entry. */
    fun rememberGlossaryView(
        origin: SlipboxRoute.Glossary,
        review: Boolean,
        query: String,
    ): Boolean {
        if (!attached || !isSavedText(query)) return false
        val presented = routes.lastOrNull() as? SlipboxRoute.Glossary ?: return false
        if (presented.term != null || !presented.samePlace(origin) || presented.binding != origin.binding) {
            return false
        }
        if (presented.review == review && presented.query == query) return true
        routes[routes.lastIndex] = presented.copy(review = review, query = query)
        return true
    }

    /** Save the live reader position if [origin] is still the presented route. */
    fun rememberReadingPlace(origin: SlipboxRoute.Reader, anchor: ReadingAnchor): Boolean {
        if (!attached || !anchor.isCanonical()) return false
        val presented = routes.lastOrNull() as? SlipboxRoute.Reader ?: return false
        if (!presented.samePlace(origin) || presented.note.binding != origin.note.binding) {
            return false
        }
        if (presented.anchor == anchor) return true
        routes[routes.lastIndex] = presented.copy(anchor = anchor)
        persist(presented.note.binding.source)
        return true
    }

    /** Replace a restored hint with the identity answered by the current generation. */
    fun reconcileReadingNote(origin: SlipboxRoute.Reader, resolved: NodeRecord): Boolean {
        if (!attached) return false
        val presented = routes.lastOrNull() as? SlipboxRoute.Reader ?: return false
        if (!presented.samePlace(origin) || presented.note.binding != origin.note.binding) {
            return false
        }
        val note = presented.note.resolvedBy(resolved)
        if (note == presented.note) return true
        routes[routes.lastIndex] = presented.copy(note = note)
        persist(note.binding.source)
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
                route.samePlace(target) && route.reads == target.reads
            }
        if (existing >= 0) {
            while (routes.lastIndex > existing) routes.removeAt(routes.lastIndex)
        } else {
            routes.add(target)
        }
        persist(origin.note.binding.source)
        return true
    }

    fun back(): Boolean {
        if (!attached || routes.size <= 1) return false
        val removed = routes.removeAt(routes.lastIndex)
        (removed as? SlipboxRoute.Reader)?.let { persist(it.note.binding.source) }
        return true
    }

    /** Drop reads and unbound search state that belonged to the prior active source. */
    fun switchSource(to: GenerationBinding): Boolean {
        if (!attached || !to.isCanonical()) return false
        routes
            .filterIsInstance<SlipboxRoute.Reader>()
            .map { it.note.binding.source }
            .filter { it != to.source }
            .distinct()
            .forEach(::persist)
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
        val removed = routes.retainAll { entry ->
            !entry.names(source) || (keepSettings && entry is SlipboxRoute.SourceSettings)
        }
        if (removed) trails.save(source, emptyList())
        return removed || changed
    }

    internal fun detach() {
        routes
            .filterIsInstance<SlipboxRoute.Reader>()
            .map { it.note.binding.source }
            .distinct()
            .forEach(::persist)
        attached = false
    }

    private fun persist(source: String) {
        trails.save(
            source,
            routes.filterIsInstance<SlipboxRoute.Reader>().filter { it.note.binding.source == source },
        )
    }
}

/** Providers are captured for this owner's lifetime. */
@Composable
internal fun rememberSlipboxBackStack(
    availability: DestinationAvailability,
    generations: SourceGenerations = SourceGenerations.None,
    restored: List<SlipboxRoute> = emptyList(),
    trails: ReadingTrailSink = ReadingTrailSink.None,
): SlipboxBackStack {
    val backStack = rememberSaveable(
        saver = listSaver<SlipboxBackStack, String>(
            save = { saveRoutes(it.entries) },
            restore = {
                SlipboxBackStack(
                    restored = restoreRoutes(it, availability, generations),
                    availability = availability,
                    generations = generations,
                    trails = trails,
                )
            },
        ),
    ) {
        SlipboxBackStack(
            restoreRoutes(saveRoutes(restored), availability, generations),
            availability,
            generations,
            trails,
        )
    }
    DisposableEffect(backStack) { onDispose(backStack::detach) }
    return backStack
}
