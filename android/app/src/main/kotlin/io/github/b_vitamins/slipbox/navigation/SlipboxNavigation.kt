/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.runtime.Composable
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.ui.NavDisplay
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTransitions

@Composable
internal fun SlipboxNavigation(
    destinations: SlipboxDestinations,
    motion: SlipboxMotion,
    generations: SourceGenerations = SourceGenerations.None,
) {
    val backStack = rememberSlipboxBackStack(destinations, generations)
    NavDisplay(
        backStack = backStack.entries,
        onBack = { backStack.back() },
        transitionSpec = SlipboxTransitions.exchange(motion),
        popTransitionSpec = SlipboxTransitions.exchange(motion),
        predictivePopTransitionSpec = SlipboxTransitions.draggedExchange(motion),
        entryProvider = { route ->
            // Position/query changes retain destination state.
            NavEntry(route, contentKey = route.place) { presented ->
                destinations.Present(presented, backStack)
            }
        },
    )
}
