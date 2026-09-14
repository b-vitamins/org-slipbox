/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.onAllNodesWithText
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.ui.IconControl
import io.github.b_vitamins.slipbox.ui.ReadingSurface
import io.github.b_vitamins.slipbox.ui.SpecimenNote
import io.github.b_vitamins.slipbox.ui.TextControl
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

internal object Synthetic {
    const val ALPHA = "0123456789abcdef0123456789abcdef"
    const val BETA = "fedcba9876543210fedcba9876543210"
    const val GENERATION = "2026-09-14T10-15-00"
    const val KEY = "20260914T090000"
    const val MARK = "figure-2"
    const val TERM = "monad"
    const val SEARCHED = "kant"

    const val LIBRARY = "Synthetic library"
    const val GLOSSARY = "Synthetic glossary"
    const val NO_SEARCH = "No search"
    const val OPEN = "Open"
    const val SEARCH = "Search"

    fun note(source: String = ALPHA, key: String = KEY, mark: String = "") =
        SlipboxRoute.Reader(
            note = BoundNote(GenerationBinding(source, GENERATION), key),
            anchor = ReadingAnchor(mark),
        )

    fun glossary(source: String = ALPHA, term: String = TERM) =
        SlipboxRoute.Glossary(GenerationBinding(source, GENERATION), term = term)
}

internal fun readyGenerations(vararg sources: String): SourceGenerations {
    val ready = sources.toSet()
    return SourceGenerations { if (it in ready) Synthetic.GENERATION else null }
}

@Composable
internal fun SyntheticHost(
    settings: ReadingSettings,
    generations: SourceGenerations = readyGenerations(Synthetic.ALPHA, Synthetic.BETA),
    onOwner: (SlipboxBackStack) -> Unit = {},
) {
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    val destinations = remember(settings) { syntheticDestinations(settings, onOwner) }
    SlipboxTheme(appearance = settings.preferences.appearance) {
        SlipboxNavigation(destinations = destinations, motion = motion, generations = generations)
    }
}

internal fun ComposeTestRule.showing(text: String): Boolean =
    onAllNodesWithText(text).fetchSemanticsNodes().isNotEmpty()

internal fun ComposeTestRule.revealing(paneTitle: String): Boolean =
    onAllNodes(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle))
        .fetchSemanticsNodes()
        .isNotEmpty()

private fun syntheticDestinations(
    settings: ReadingSettings,
    onOwner: (SlipboxBackStack) -> Unit,
): SlipboxDestinations =
    slipboxDestinations {
        reported(SlipboxSurface.Library, onOwner) { route, stack ->
            SyntheticLibrary(route as SlipboxRoute.Library, stack)
        }
        reported(SlipboxSurface.Reader, onOwner) { _, stack ->
            SpecimenNote(settings = settings, onBack = { stack.back() })
        }
        reported(SlipboxSurface.Glossary, onOwner) { route, stack ->
            SyntheticGlossary(route as SlipboxRoute.Glossary, stack)
        }
    }

private fun SlipboxDestinations.Builder.reported(
    presented: SlipboxSurface,
    onOwner: (SlipboxBackStack) -> Unit,
    destination: SlipboxDestination,
) {
    surface(presented) { route, stack ->
        SideEffect { onOwner(stack) }
        destination(route, stack)
    }
}

@Composable
private fun SyntheticLibrary(route: SlipboxRoute.Library, stack: SlipboxBackStack) {
    ReadingSurface(
        title = Synthetic.LIBRARY,
        trailing = {
            TextControl(label = Synthetic.OPEN, onClick = { stack.open(Synthetic.note()) })
        },
    ) {
        Text(
            text = route.query.ifEmpty { Synthetic.NO_SEARCH },
            style = MaterialTheme.typography.bodyLarge,
            color = SlipboxTheme.colors.ink,
        )
        TextControl(
            label = Synthetic.SEARCH,
            onClick = { stack.open(route.copy(query = Synthetic.SEARCHED)) },
        )
    }
}

@Composable
private fun SyntheticGlossary(route: SlipboxRoute.Glossary, stack: SlipboxBackStack) {
    ReadingSurface(
        title = Synthetic.GLOSSARY,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = { stack.back() },
            )
        },
    ) {
        Text(
            text = route.term.orEmpty(),
            style = MaterialTheme.typography.bodyLarge,
            color = SlipboxTheme.colors.ink,
        )
    }
}
