/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox

import androidx.compose.runtime.Composable
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.ui.NavDisplay
import io.github.b_vitamins.slipbox.navigation.SlipboxDestination
import io.github.b_vitamins.slipbox.navigation.popDestination
import io.github.b_vitamins.slipbox.navigation.pushDestination
import io.github.b_vitamins.slipbox.navigation.rememberSlipboxBackStack
import io.github.b_vitamins.slipbox.ui.AboutScreen
import io.github.b_vitamins.slipbox.ui.LibraryScreen
import io.github.b_vitamins.slipbox.ui.settings.rememberReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTransitions
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

@Composable
fun SlipboxApp() {
    val backStack = rememberSlipboxBackStack()
    val settings = rememberReadingSettings()
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    SlipboxTheme(appearance = settings.preferences.appearance) {
        NavDisplay(
            backStack = backStack,
            onBack = { backStack.popDestination() },
            transitionSpec = SlipboxTransitions.exchange(motion),
            popTransitionSpec = SlipboxTransitions.exchange(motion),
            predictivePopTransitionSpec = SlipboxTransitions.draggedExchange(motion),
            entryProvider = { destination ->
                NavEntry(destination) { key ->
                    when (key) {
                        is SlipboxDestination.Library ->
                            LibraryScreen(
                                onOpenAbout = { backStack.pushDestination(SlipboxDestination.About) },
                            )

                        is SlipboxDestination.About ->
                            AboutScreen(
                                onBack = { backStack.popDestination() },
                                settings = settings,
                            )
                    }
                }
            },
        )
    }
}
