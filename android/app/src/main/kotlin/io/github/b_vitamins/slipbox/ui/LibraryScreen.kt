/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

@Composable
internal fun LibraryScreen(
    onOpenAbout: () -> Unit,
    modifier: Modifier = Modifier,
    phase: SourceLibraryPhase = SourceLibraryPhase.Empty(0),
    onConnect: () -> Unit = {},
    onRetry: () -> Unit = {},
) {
    ReadingSurface(
        title = stringResource(R.string.app_name),
        modifier = modifier,
        trailing = {
            Row(horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingHorizontal)) {
                if (phase is SourceLibraryPhase.Ready) {
                    TextControl(label = stringResource(R.string.action_add_source), onClick = onConnect)
                }
                TextControl(label = stringResource(R.string.action_about), onClick = onOpenAbout)
            }
        },
    ) {
        when (phase) {
            SourceLibraryPhase.Loading -> LibraryNotice(stringResource(R.string.library_loading))
            is SourceLibraryPhase.Empty -> {
                LibraryNotice(stringResource(R.string.library_empty))
                TextControl(label = stringResource(R.string.action_connect), onClick = onConnect)
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
            }
            is SourceLibraryPhase.Failed -> {
                LibraryNotice(stringResource(R.string.library_unavailable), problem = true)
                TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
            }
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
