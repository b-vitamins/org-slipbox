/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.navigation.BackNavigationBehavior
import androidx.compose.material3.adaptive.navigation3.ListDetailSceneStrategy
import androidx.compose.material3.adaptive.navigation3.rememberListDetailSceneStrategy
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.ui.NavDisplay
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTransitions

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
internal fun SlipboxNavigation(
    destinations: SlipboxDestinations,
    motion: SlipboxMotion,
    generations: SourceGenerations = SourceGenerations.None,
    restored: List<SlipboxRoute> = emptyList(),
    trails: ReadingTrailSink = ReadingTrailSink.None,
) {
    val backStack = rememberSlipboxBackStack(destinations, generations, restored, trails)
    val listDetail =
        rememberListDetailSceneStrategy<SlipboxRoute>(
            shouldHandleSinglePaneLayout = true,
            // One Back action always means one route, at every window size.
            backNavigationBehavior = BackNavigationBehavior.PopLatest,
        )
    NavDisplay(
        backStack = backStack.entries,
        modifier =
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .windowInsetsPadding(WindowInsets.safeDrawing),
        onBack = { backStack.back() },
        sceneStrategies = listOf(listDetail),
        transitionSpec = SlipboxTransitions.exchange(motion),
        popTransitionSpec = SlipboxTransitions.exchange(motion),
        predictivePopTransitionSpec = SlipboxTransitions.draggedExchange(motion),
        entryProvider = { route ->
            // Position/query changes retain destination state.
            NavEntry(
                route,
                contentKey = route.place,
                metadata = adaptivePaneMetadata(route, backStack.entries),
            ) { presented ->
                destinations.Present(presented, backStack)
            }
        },
    )
}

/** The nearest list route owns detail presentation without changing route semantics. */
internal fun listDetailSceneKey(
    route: SlipboxRoute,
    history: List<SlipboxRoute>,
): String? {
    if (route.isAdaptiveList()) return route.place
    if (!route.isAdaptiveDetail()) return null

    val index = history.indexOfLast { it == route }.let { if (it < 0) history.size else it }
    return history.take(index).lastOrNull(SlipboxRoute::isAdaptiveList)?.place
}

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
private fun adaptivePaneMetadata(
    route: SlipboxRoute,
    history: List<SlipboxRoute>,
): Map<String, Any> =
    when {
        route is SlipboxRoute.Library ->
            ListDetailSceneStrategy.listPane(sceneKey = route.place) {
                AdaptiveDetailPlaceholder(R.string.adaptive_select_reading)
            }
        route is SlipboxRoute.Glossary && route.term == null ->
            ListDetailSceneStrategy.listPane(sceneKey = route.place) {
                AdaptiveDetailPlaceholder(R.string.adaptive_select_term)
            }
        route.isAdaptiveDetail() ->
            listDetailSceneKey(route, history)
                ?.let { ListDetailSceneStrategy.detailPane(it) }
                .orEmpty()
        else -> emptyMap()
    }

private fun SlipboxRoute.isAdaptiveList(): Boolean =
    this is SlipboxRoute.Library || (this is SlipboxRoute.Glossary && term == null)

private fun SlipboxRoute.isAdaptiveDetail(): Boolean =
    this is SlipboxRoute.Reader || (this is SlipboxRoute.Glossary && term != null)

@Composable
private fun AdaptiveDetailPlaceholder(message: Int) {
    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface)
                .safeDrawingPadding()
                .padding(SlipboxDimensions.readingPadding * 2),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = stringResource(message),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}
