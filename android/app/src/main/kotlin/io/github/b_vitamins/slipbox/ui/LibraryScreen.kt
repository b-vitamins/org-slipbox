/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

@Composable
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
) {
    ReadingSurface(
        title = stringResource(R.string.app_name),
        modifier = modifier,
        scrollable = phase !is SourceLibraryPhase.Ready,
        trailing = {
            Row(horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingHorizontal)) {
                if (hasSources) {
                    TextControl(label = stringResource(R.string.action_sources), onClick = onManageSources)
                }
                if (phase is SourceLibraryPhase.Ready || hasSources) {
                    TextControl(label = stringResource(R.string.action_add_source), onClick = onConnect)
                }
                TextControl(label = stringResource(R.string.action_about), onClick = onOpenAbout)
            }
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
                Text(
                    text = phase.source.source.displayName,
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onBackground,
                )
                LibraryNotice(
                    stringResource(
                        R.string.library_ready,
                        pluralStringResource(
                            R.plurals.library_files,
                            phase.source.stats.filesIndexed.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                            phase.source.stats.filesIndexed,
                        ),
                        pluralStringResource(
                            R.plurals.library_nodes,
                            phase.source.stats.nodesIndexed.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                            phase.source.stats.nodesIndexed,
                        ),
                        phase.source.revision.take(10),
                    ),
                )
                NotesInventory(
                    phase = inventory ?: NotesInventoryPhase.Loading,
                    onLoadMore = onLoadMore,
                    onRetry = onRetryInventory,
                    onOpenNote = onOpenNote,
                    modifier = Modifier.weight(1f),
                )
            }
            is SourceLibraryPhase.Failed -> {
                LibraryNotice(stringResource(R.string.library_unavailable), problem = true)
                TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
            }
        }
    }
}

@Composable
private fun NotesInventory(
    phase: NotesInventoryPhase,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenNote: (NodeRecord) -> Unit,
    modifier: Modifier = Modifier,
) {
    when (phase) {
        NotesInventoryPhase.Loading ->
            LibraryNotice(stringResource(R.string.notes_loading))
        NotesInventoryPhase.Empty ->
            LibraryNotice(stringResource(R.string.notes_empty))
        NotesInventoryPhase.Failed -> {
            LibraryNotice(stringResource(R.string.notes_unavailable), problem = true)
            TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
        }
        is NotesInventoryPhase.Ready -> {
            Text(
                text =
                    pluralStringResource(
                        R.plurals.library_notes,
                        phase.total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                        phase.total,
                    ),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onBackground,
                modifier = Modifier.semantics { heading() },
            )
            LazyColumn(modifier = modifier.fillMaxWidth()) {
                items(items = phase.notes, key = NodeRecord::nodeKey) { note ->
                    NoteInventoryRow(note, onClick = { onOpenNote(note) })
                    HorizontalDivider(
                        thickness = SlipboxDimensions.hairline,
                        color = MaterialTheme.colorScheme.outline,
                    )
                }
                if (phase.hasMore) {
                    item(key = "continuation:${phase.nextPosition}") {
                        if (!phase.loadingMore && !phase.continuationFailed) {
                            LaunchedEffect(phase.nextPosition) { onLoadMore() }
                        }
                        if (phase.continuationFailed) {
                            LibraryNotice(
                                stringResource(R.string.notes_more_unavailable),
                                problem = true,
                            )
                            TextControl(
                                label = stringResource(R.string.action_retry),
                                onClick = onRetry,
                            )
                        } else {
                            LibraryNotice(stringResource(R.string.notes_loading_more))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun NoteInventoryRow(note: NodeRecord, onClick: () -> Unit) {
    val location =
        when (note.kind) {
            NodeKind.FILE -> note.filePath
            NodeKind.HEADING -> "${note.filePath} · ${note.outlinePath}"
        }
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(onClick = onClick)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        Text(
            text = note.title,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
        Text(
            text = location,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
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
