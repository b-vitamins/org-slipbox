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

        fun back(): Boolean {
        if (!attached || routes.size <= 1) return false
        routes.removeAt(routes.lastIndex)
        return true
    }

    /** Drop reads from other sources; retain unbound routes and the library search. */
    fun switchSource(to: GenerationBinding): Boolean {
        if (!attached || !to.isCanonical()) return false
        return routes.retainAll { entry ->
            val read = entry.reads
            read == null || read.source == to.source
        }
    }

    /** Remove both reading and configuration entries for this source. */
    fun removeSource(source: String): Boolean {
        if (!attached) return false
        return routes.retainAll { !it.names(source) }
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
