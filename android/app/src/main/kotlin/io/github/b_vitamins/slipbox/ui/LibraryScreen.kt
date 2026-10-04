/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.navigation.ReadingReturn
import io.github.b_vitamins.slipbox.navigation.ReadingReturnsSnapshot
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

@Composable
@Suppress("UNUSED_PARAMETER")
internal fun LibraryScreen(
    onOpenAbout: () -> Unit,
    modifier: Modifier = Modifier,
    phase: SourceLibraryPhase = SourceLibraryPhase.Empty(0),
    hasSources: Boolean = phase is SourceLibraryPhase.Ready,
    onConnect: () -> Unit = {},
    onManageSources: () -> Unit = {},
    onRetry: () -> Unit = {},
    inventory: NotesInventoryPhase? = null,
    onLoadMore: () -> Unit = {},
    onRetryInventory: () -> Unit = {},
    onOpenNote: (NodeRecord) -> Unit = {},
    selectedNoteNodeKey: String? = null,
    onOpenGlossary: () -> Unit = {},
    searchInput: TextFieldValue = TextFieldValue(),
    searchPhase: CorpusSearchPhase = CorpusSearchPhase.Dormant,
    onSearchChange: (TextFieldValue) -> Unit = {},
    onRetrySearch: () -> Unit = {},
    onOpenSearchHit: (CorpusSearchHit) -> Unit = {},
    readingReturns: ReadingReturnsSnapshot? = null,
    onOpenReadingReturn: (ReadingReturn) -> Unit = {},
    onRemoveBookmark: (ReadingReturn) -> Unit = {},
    onRemoveRecent: (ReadingReturn) -> Unit = {},
    onClearBookmarks: () -> Unit = {},
    onClearRecents: () -> Unit = {},
    appearance: SlipboxAppearance = SlipboxAppearance.System,
    reduceMotion: Boolean = false,
    onSelectAppearance: (SlipboxAppearance) -> Unit = {},
    onSelectReduceMotion: (Boolean) -> Unit = {},
    randomPhase: RandomNotePhase = RandomNotePhase.Idle,
    onSurpriseMe: () -> Unit = {},
) {
    EntryDrawer(
        appearance = appearance,
        reduceMotion = reduceMotion,
        onSelectAppearance = onSelectAppearance,
        onSelectReduceMotion = onSelectReduceMotion,
        onOpenSources = if (hasSources) onManageSources else onConnect,
        onOpenAbout = onOpenAbout,
        modifier = modifier,
    ) {
        openMenu ->
        ReadingSurface(
            title = stringResource(R.string.entry_notes),
            scrollable = phase !is SourceLibraryPhase.Ready,
            contentBackground = MaterialTheme.colorScheme.background,
            topBar = {
                EntryTopBar(
                    selected = EntrySection.Notes,
                    onOpenMenu = openMenu,
                    onShowNotes = {},
                    onShowGlossary = onOpenGlossary,
                    glossaryEnabled = phase is SourceLibraryPhase.Ready,
                )
            },
        ) {
            when (phase) {
                SourceLibraryPhase.Loading -> LibraryNotice(stringResource(R.string.library_loading))
                is SourceLibraryPhase.Empty -> {
                    LibraryNotice(
                        stringResource(
                            if (hasSources) R.string.library_no_selection else R.string.library_empty,
                        ),
                    )
                    TextControl(
                        label =
                            stringResource(
                                if (hasSources) R.string.action_sources else R.string.action_connect,
                            ),
                        onClick = if (hasSources) onManageSources else onConnect,
                    )
                }
                is SourceLibraryPhase.Ready -> {
                    val landing = searchInput.text.isBlank()
                    if (landing) Spacer(Modifier.weight(1f))
                    CorpusSearchField(input = searchInput, onChange = onSearchChange)
                    if (landing) {
                        LandingActions(
                            randomPhase = randomPhase,
                            onSurpriseMe = onSurpriseMe,
                        )
                        Spacer(Modifier.weight(1f))
                        // Extra lower space puts the field just above the visual midpoint.
                        Spacer(Modifier.height(32.dp))
                    } else {
                        CorpusSearchResults(
                            phase = searchPhase,
                            presentation =
                                rememberDocumentPresentation(
                                    appearance = appearance,
                                    reduceMotion = reduceMotion,
                                    availableWidth = SlipboxDimensions.readingMeasure,
                                ),
                            onRetry = onRetrySearch,
                            onOpen = onOpenSearchHit,
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
                is SourceLibraryPhase.Failed -> {
                    LibraryNotice(stringResource(R.string.library_unavailable), problem = true)
                    TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                }
            }
        }
    }
}

@Composable
private fun LandingActions(
    randomPhase: RandomNotePhase,
    onSurpriseMe: () -> Unit,
) {
    Spacer(Modifier.height(8.dp))
    Box(modifier = Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        SurpriseControl(
            pending = randomPhase == RandomNotePhase.Loading,
            onClick = onSurpriseMe,
        )
    }
    val notice =
        when (randomPhase) {
            RandomNotePhase.Empty -> R.string.search_surprise_empty
            RandomNotePhase.Failed -> R.string.search_surprise_unavailable
            RandomNotePhase.Idle,
            RandomNotePhase.Loading,
            -> null
        }
    if (notice != null) {
        Text(
            text = stringResource(notice),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun SurpriseControl(pending: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier =
            Modifier
                .widthIn(min = 124.dp)
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(enabled = !pending, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier =
                Modifier
                    .widthIn(min = 116.dp)
                    .height(34.dp)
                    .clip(shape)
                    .background(MaterialTheme.colorScheme.surface)
                    .border(
                        width = SlipboxDimensions.hairline,
                        color = MaterialTheme.colorScheme.outline,
                        shape = shape,
                    )
                    .padding(horizontal = 16.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text =
                    stringResource(
                        if (pending) {
                            R.string.search_surprise_loading
                        } else {
                            R.string.search_surprise
                        },
                    ),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun LibraryNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
