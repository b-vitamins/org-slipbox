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
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.TextFieldValue
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.navigation.ReadingReturn
import io.github.b_vitamins.slipbox.navigation.ReadingReturnAvailability
import io.github.b_vitamins.slipbox.navigation.ReadingReturnsSnapshot
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import kotlin.math.roundToInt

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
    onOpenGlossary: () -> Unit = {},
    searchInput: TextFieldValue = TextFieldValue(),
    searchPhase: CorpusSearchPhase = CorpusSearchPhase.Dormant,
    selectedSearchNodeKey: String? = null,
    onSearchChange: (TextFieldValue) -> Unit = {},
    onLoadMoreSearch: () -> Unit = {},
    onRetrySearch: () -> Unit = {},
    onOpenSearchHit: (CorpusSearchHit) -> Unit = {},
    readingReturns: ReadingReturnsSnapshot? = null,
    onOpenReadingReturn: (ReadingReturn) -> Unit = {},
    onRemoveBookmark: (ReadingReturn) -> Unit = {},
    onRemoveRecent: (ReadingReturn) -> Unit = {},
    onClearBookmarks: () -> Unit = {},
    onClearRecents: () -> Unit = {},
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
                CorpusSearchField(input = searchInput, onChange = onSearchChange)
                if (searchInput.text.isBlank()) {
                    NotesInventory(
                        phase = inventory ?: NotesInventoryPhase.Loading,
                        onLoadMore = onLoadMore,
                        onRetry = onRetryInventory,
                        onOpenNote = onOpenNote,
                        onOpenGlossary = onOpenGlossary,
                        readingReturns = readingReturns,
                        onOpenReadingReturn = onOpenReadingReturn,
                        onRemoveBookmark = onRemoveBookmark,
                        onRemoveRecent = onRemoveRecent,
                        onClearBookmarks = onClearBookmarks,
                        onClearRecents = onClearRecents,
                        modifier = Modifier.weight(1f),
                    )
                } else {
                    CorpusSearchResults(
                        phase = searchPhase,
                        selectedNodeKey = selectedSearchNodeKey,
                        onLoadMore = onLoadMoreSearch,
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

@Composable
private fun NotesInventory(
    phase: NotesInventoryPhase,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenNote: (NodeRecord) -> Unit,
    onOpenGlossary: () -> Unit,
    readingReturns: ReadingReturnsSnapshot?,
    onOpenReadingReturn: (ReadingReturn) -> Unit,
    onRemoveBookmark: (ReadingReturn) -> Unit,
    onRemoveRecent: (ReadingReturn) -> Unit,
    onClearBookmarks: () -> Unit,
    onClearRecents: () -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(modifier = modifier.fillMaxWidth()) {
        item(key = "open-glossary") {
            LibraryDestinationRow(
                title = stringResource(R.string.glossary_title),
                detail = stringResource(R.string.glossary_library_detail),
                onClick = onOpenGlossary,
            )
            HorizontalDivider(
                modifier = Modifier.padding(bottom = SlipboxDimensions.headerPaddingVertical),
                thickness = SlipboxDimensions.hairline,
                color = MaterialTheme.colorScheme.outline,
            )
        }
        readingReturnItems(
            snapshot = readingReturns,
            onOpen = onOpenReadingReturn,
            onRemoveBookmark = onRemoveBookmark,
            onRemoveRecent = onRemoveRecent,
            onClearBookmarks = onClearBookmarks,
            onClearRecents = onClearRecents,
        )
        when (phase) {
            NotesInventoryPhase.Loading ->
                item(key = "notes-loading") {
                    LibraryNotice(stringResource(R.string.notes_loading))
                }
            NotesInventoryPhase.Empty ->
                item(key = "notes-empty") {
                    LibraryNotice(stringResource(R.string.notes_empty))
                }
            NotesInventoryPhase.Failed -> {
                item(key = "notes-failed") {
                    LibraryNotice(stringResource(R.string.notes_unavailable), problem = true)
                    TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                }
            }
            is NotesInventoryPhase.Ready -> {
                item(key = "notes-heading") {
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
                }
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
private fun LibraryDestinationRow(title: String, detail: String, onClick: () -> Unit) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(onClick = onClick)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            text = detail,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun LazyListScope.readingReturnItems(
    snapshot: ReadingReturnsSnapshot?,
    onOpen: (ReadingReturn) -> Unit,
    onRemoveBookmark: (ReadingReturn) -> Unit,
    onRemoveRecent: (ReadingReturn) -> Unit,
    onClearBookmarks: () -> Unit,
    onClearRecents: () -> Unit,
) {
    snapshot ?: return
    val continuing = snapshot.recents.firstOrNull()
    if (continuing != null) {
        item(key = "reading-continue-heading") {
            ReadingSectionHeading(
                title = stringResource(R.string.reading_continue),
                action = stringResource(R.string.action_clear_recents),
                onAction = onClearRecents,
            )
        }
        item(key = "reading-continue:${continuing.note.reference}") {
            ReadingReturnRow(
                entry = continuing,
                onOpen = { onOpen(continuing) },
                onRemove = { onRemoveRecent(continuing) },
            )
        }
    }
    if (snapshot.bookmarks.isNotEmpty()) {
        item(key = "reading-bookmarks-heading") {
            ReadingSectionHeading(
                title = stringResource(R.string.reading_bookmarks),
                action = stringResource(R.string.action_clear_bookmarks),
                onAction = onClearBookmarks,
            )
        }
        items(
            items = snapshot.bookmarks,
            key = { "bookmark:${it.note.reference}" },
        ) { entry ->
            ReadingReturnRow(
                entry = entry,
                onOpen = { onOpen(entry) },
                onRemove = { onRemoveBookmark(entry) },
            )
        }
    }
    val earlier = snapshot.recents.drop(1)
    if (earlier.isNotEmpty()) {
        item(key = "reading-recents-heading") {
            ReadingSectionHeading(title = stringResource(R.string.reading_recent))
        }
        items(
            items = earlier,
            key = { "recent:${it.note.reference}" },
        ) { entry ->
            ReadingReturnRow(
                entry = entry,
                onOpen = { onOpen(entry) },
                onRemove = { onRemoveRecent(entry) },
            )
        }
    }
    if (continuing != null || snapshot.bookmarks.isNotEmpty()) {
        item(key = "reading-divider") {
            HorizontalDivider(
                modifier = Modifier.padding(vertical = SlipboxDimensions.headerPaddingVertical),
                thickness = SlipboxDimensions.hairline,
                color = MaterialTheme.colorScheme.outline,
            )
        }
    }
}

@Composable
private fun ReadingSectionHeading(
    title: String,
    action: String? = null,
    onAction: () -> Unit = {},
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        if (action != null) TextControl(label = action, onClick = onAction)
    }
}

@Composable
private fun ReadingReturnRow(
    entry: ReadingReturn,
    onOpen: () -> Unit,
    onRemove: () -> Unit,
) {
    val available = entry.availability == ReadingReturnAvailability.Available
    val detail =
        when (entry.availability) {
            ReadingReturnAvailability.Checking -> stringResource(R.string.reading_checking)
            ReadingReturnAvailability.Missing -> stringResource(R.string.reading_missing)
            ReadingReturnAvailability.Unavailable -> stringResource(R.string.reading_unavailable)
            ReadingReturnAvailability.Available -> {
                val progress = (entry.anchor.progress * 100).roundToInt()
                val location = entry.note.filePath.ifEmpty { entry.note.nodeKey }
                if (progress > 0) {
                    stringResource(R.string.reading_location_progress, location, progress)
                } else {
                    location
                }
            }
        }
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = SlipboxDimensions.touchTarget),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .clickable(enabled = available, onClick = onOpen)
                    .padding(vertical = SlipboxDimensions.headerPaddingVertical),
        ) {
            Text(
                text = entry.title,
                style = MaterialTheme.typography.bodyLarge,
                color =
                    if (available) {
                        MaterialTheme.colorScheme.onBackground
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
            )
            Text(
                text = detail,
                style = MaterialTheme.typography.bodySmall,
                color =
                    if (entry.availability == ReadingReturnAvailability.Missing) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
            )
        }
        val removeDescription = stringResource(R.string.reading_remove_description, entry.title)
        TextControl(
            label = stringResource(R.string.action_remove),
            onClick = onRemove,
            modifier = Modifier.semantics { contentDescription = removeDescription },
        )
    }
    HorizontalDivider(
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
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
