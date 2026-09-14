/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable

internal fun interface DestinationAvailability {
    fun presents(surface: SlipboxSurface): Boolean
}

internal typealias SlipboxDestination =
    @Composable (route: SlipboxRoute, backStack: SlipboxBackStack) -> Unit

/** Availability comes from registered handlers, not a separate advertised inventory. */
@Stable
internal class SlipboxDestinations private constructor(
    private val handlers: Map<SlipboxSurface, SlipboxDestination>,
) : DestinationAvailability {

    init {
        require(SlipboxSurface.Library in handlers) { "the start surface has no destination" }
    }

    override fun presents(surface: SlipboxSurface): Boolean = surface in handlers

    @Composable
    fun Present(route: SlipboxRoute, backStack: SlipboxBackStack) {
        val handler = checkNotNull(handlers[route.surface]) {
            "${route.surface} has no destination"
        }
        handler(route, backStack)
    }

    internal class Builder {
        private val handlers = mutableMapOf<SlipboxSurface, SlipboxDestination>()

        fun surface(surface: SlipboxSurface, destination: SlipboxDestination) {
            require(handlers.put(surface, destination) == null) {
                "$surface already has a destination"
            }
        }

        fun build(): SlipboxDestinations = SlipboxDestinations(handlers.toMap())
    }
}

internal fun slipboxDestinations(
    declare: SlipboxDestinations.Builder.() -> Unit,
): SlipboxDestinations = SlipboxDestinations.Builder().apply(declare).build()
