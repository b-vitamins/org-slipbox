/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import kotlinx.coroutines.launch

internal enum class EntrySection {
    Notes,
    Glossary,
}

@Composable
internal fun EntryDrawer(
    appearance: SlipboxAppearance,
    reduceMotion: Boolean,
    onSelectAppearance: (SlipboxAppearance) -> Unit,
    onSelectReduceMotion: (Boolean) -> Unit,
    onOpenSources: () -> Unit,
    onOpenAbout: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable (openMenu: () -> Unit) -> Unit,
) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    fun leave(action: () -> Unit) {
        scope.launch {
            drawer.close()
            action()
        }
    }
    ModalNavigationDrawer(
        drawerState = drawer,
        modifier = modifier,
        drawerContent = {
            ModalDrawerSheet(
                drawerContainerColor = MaterialTheme.colorScheme.surface,
                drawerContentColor = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.width(288.dp).fillMaxHeight(),
            ) {
                Column(
                    modifier =
                        Modifier
                            .fillMaxHeight()
                            .verticalScroll(rememberScrollState())
                            .padding(
                                horizontal = SlipboxDimensions.readingPadding,
                                vertical = 24.dp,
                            ),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = stringResource(R.string.menu_title),
                        style =
                            MaterialTheme.typography.titleLarge.copy(
                                fontWeight = FontWeight.Medium,
                            ),
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp).semantics {
                            heading()
                        },
                    )
                    NavigationDrawerItem(
                        label = { Text(stringResource(R.string.action_sources)) },
                        selected = false,
                        onClick = { leave(onOpenSources) },
                    )
                    NavigationDrawerItem(
                        label = { Text(stringResource(R.string.action_about)) },
                        selected = false,
                        onClick = { leave(onOpenAbout) },
                    )
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 12.dp),
                        thickness = SlipboxDimensions.hairline,
                        color = MaterialTheme.colorScheme.outline,
                    )
                    Text(
                        text = stringResource(R.string.appearance_theme),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 12.dp),
                    )
                    RestrainedSegmentedControl(
                        labels = SlipboxAppearance.entries.map { stringResource(it.shortLabel()) },
                        selectedIndex = SlipboxAppearance.entries.indexOf(appearance),
                        onSelect = { onSelectAppearance(SlipboxAppearance.entries[it]) },
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp),
                    )
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = SlipboxDimensions.touchTarget)
                                .clickable(
                                    role = Role.Switch,
                                    onClick = { onSelectReduceMotion(!reduceMotion) },
                                )
                                .padding(horizontal = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = stringResource(R.string.appearance_reduce_motion),
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                        Switch(
                            checked = reduceMotion,
                            onCheckedChange = null,
                        )
                    }
                }
            }
        },
    ) {
        content { scope.launch { drawer.open() } }
    }
}

@Composable
internal fun EntryTopBar(
    selected: EntrySection,
    onOpenMenu: () -> Unit,
    onShowNotes: () -> Unit,
    onShowGlossary: () -> Unit,
    glossaryEnabled: Boolean = true,
) {
    Box(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.headerMinHeight)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        val menuLabel = stringResource(R.string.action_menu)
        Box(
            modifier =
                Modifier
                    .align(Alignment.CenterStart)
                    .padding(start = 8.dp)
                    .size(SlipboxDimensions.touchTarget)
                    .clickable(role = Role.Button, onClick = onOpenMenu)
                    .semantics { this.contentDescription = menuLabel },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_menu),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(SlipboxDimensions.glyph),
            )
        }
        CompactEntrySwitcher(
            selected = selected,
            onShowNotes = onShowNotes,
            onShowGlossary = onShowGlossary,
            glossaryEnabled = glossaryEnabled,
            modifier = Modifier.align(Alignment.Center),
        )
    }
}

@Composable
private fun CompactEntrySwitcher(
    selected: EntrySection,
    onShowNotes: () -> Unit,
    onShowGlossary: () -> Unit,
    glossaryEnabled: Boolean,
    modifier: Modifier = Modifier,
) {
    val labels = listOf(stringResource(R.string.entry_notes), stringResource(R.string.glossary_title))
    val selectedIndex = if (selected == EntrySection.Notes) 0 else 1
    val enabled = listOf(true, glossaryEnabled)
    val shape = RoundedCornerShape(10.dp)
    Box(
        modifier = modifier.width(176.dp).height(SlipboxDimensions.touchTarget),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .height(32.dp)
                    .clip(shape)
                    .background(SlipboxTheme.colors.surface)
                    .border(SlipboxDimensions.hairline, SlipboxTheme.colors.hairline, shape),
        ) {
            labels.indices.forEach { index ->
                if (index > 0) {
                    Box(
                        Modifier
                            .width(SlipboxDimensions.hairline)
                            .fillMaxHeight()
                            .padding(vertical = 6.dp)
                            .background(SlipboxTheme.colors.hairline),
                    )
                }
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .background(
                            if (index == selectedIndex) {
                                SlipboxTheme.colors.rule.copy(alpha = 0.58f)
                            } else {
                                SlipboxTheme.colors.surface
                            },
                        ),
                )
            }
        }
        Row(Modifier.fillMaxSize()) {
            labels.forEachIndexed { index, label ->
                val isSelected = index == selectedIndex
                Box(
                    modifier =
                        Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .selectable(
                                selected = isSelected,
                                enabled = enabled[index],
                                role = Role.Tab,
                                onClick = if (index == 0) onShowNotes else onShowGlossary,
                            ),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = label,
                        style = MaterialTheme.typography.labelMedium,
                        color =
                            when {
                                !enabled[index] -> SlipboxTheme.colors.muted.copy(alpha = 0.45f)
                                isSelected -> SlipboxTheme.colors.ink
                                else -> SlipboxTheme.colors.muted
                            },
                        fontWeight = if (isSelected) FontWeight.Medium else FontWeight.Normal,
                    )
                }
            }
        }
    }
}

@Composable
internal fun RestrainedSegmentedControl(
    labels: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    enabled: List<Boolean> = List(labels.size) { true },
) {
    require(labels.isNotEmpty())
    require(selectedIndex in labels.indices)
    require(enabled.size == labels.size)
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier =
            modifier
                .height(38.dp)
                .clip(shape)
                .background(SlipboxTheme.colors.surface)
                .border(SlipboxDimensions.hairline, SlipboxTheme.colors.hairline, shape),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        labels.forEachIndexed { index, label ->
            if (index > 0) {
                Box(
                    Modifier
                        .width(SlipboxDimensions.hairline)
                        .height(22.dp)
                        .background(SlipboxTheme.colors.hairline),
                )
            }
            val selected = index == selectedIndex
            Box(
                modifier =
                    Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .background(
                            if (selected) {
                                SlipboxTheme.colors.rule.copy(alpha = 0.58f)
                            } else {
                                SlipboxTheme.colors.surface
                            },
                        )
                        .selectable(
                            selected = selected,
                            enabled = enabled[index],
                            role = Role.Tab,
                            onClick = { onSelect(index) },
                        ),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelLarge,
                    color =
                        when {
                            !enabled[index] -> SlipboxTheme.colors.muted.copy(alpha = 0.45f)
                            selected -> SlipboxTheme.colors.ink
                            else -> SlipboxTheme.colors.muted
                        },
                    fontWeight = if (selected) FontWeight.Medium else FontWeight.Normal,
                )
            }
        }
    }
}

private fun SlipboxAppearance.shortLabel(): Int =
    when (this) {
        SlipboxAppearance.System -> R.string.appearance_system_short
        SlipboxAppearance.Light -> R.string.appearance_light
        SlipboxAppearance.Dark -> R.string.appearance_dark
    }
