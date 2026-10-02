/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.DirectedRelationDirection
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion

@Composable
internal fun DirectedRelationsSheet(
    visible: Boolean,
    phase: DirectedRelationsPhase,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    onRetry: () -> Unit,
    onLoadMore: () -> Unit,
    onPreview: (DirectedRelationRecord) -> Unit,
    onOpen: (DirectedRelationRecord) -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    ContextualSheet(
        title = stringResource(R.string.reader_relations),
        visible = visible,
        motion = motion,
        onDismiss = onDismiss,
        restoreFocusTo = restoreFocusTo,
    ) {
        when (phase) {
            DirectedRelationsPhase.Idle,
            DirectedRelationsPhase.Loading,
            -> SheetNotice(stringResource(R.string.reader_relations_loading))

            DirectedRelationsPhase.Empty ->
                SheetNotice(stringResource(R.string.reader_relations_empty))

            DirectedRelationsPhase.Failed -> {
                SheetNotice(stringResource(R.string.reader_relations_unavailable), problem = true)
                TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
            }

            is DirectedRelationsPhase.Ready -> {
                Text(
                    text =
                        pluralStringResource(
                            R.plurals.reader_related_notes,
                            phase.total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                            phase.total,
                        ),
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.fillMaxWidth().semantics { heading() },
                )
                Text(
                    text =
                        stringResource(
                            R.string.reader_relation_counts,
                            pluralStringResource(
                                R.plurals.reader_incoming_relations,
                                phase.incomingTotal.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                                phase.incomingTotal,
                            ),
                            pluralStringResource(
                                R.plurals.reader_outgoing_relations,
                                phase.outgoingTotal.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                                phase.outgoingTotal,
                            ),
                        ),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth(),
                )
                phase.relations.forEach { relation ->
                    DirectedRelationRow(
                        relation = relation,
                        onPreview = { onPreview(relation) },
                        onOpen = { onOpen(relation) },
                    )
                }
                when {
                    phase.loadingMore ->
                        SheetNotice(stringResource(R.string.reader_relations_loading_more))
                    phase.continuationFailed -> {
                        SheetNotice(
                            stringResource(R.string.reader_relations_more_unavailable),
                            problem = true,
                        )
                        TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                    }
                    phase.hasMore ->
                        TextControl(
                            label = stringResource(R.string.action_show_more_relations),
                            onClick = onLoadMore,
                        )
                }
            }
        }
    }
}

@Composable
private fun DirectedRelationRow(
    relation: DirectedRelationRecord,
    onPreview: () -> Unit,
    onOpen: () -> Unit,
) {
    val direction =
        stringResource(
            when (relation.direction) {
                DirectedRelationDirection.INCOMING -> R.string.reader_relation_incoming
                DirectedRelationDirection.OUTGOING -> R.string.reader_relation_outgoing
                DirectedRelationDirection.BIDIRECTIONAL -> R.string.reader_relation_bidirectional
            },
        )
    val location =
        when (relation.note.kind) {
            NodeKind.FILE -> relation.note.filePath
            NodeKind.HEADING -> "${relation.note.filePath} · ${relation.note.outlinePath}"
        }
    val previewLabel = stringResource(R.string.reader_relation_preview, relation.note.title)
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = SlipboxDimensions.touchTarget),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .clickable(onClickLabel = previewLabel, onClick = onPreview)
                    .padding(vertical = SlipboxDimensions.headerPaddingVertical),
        ) {
            Text(
                text = relation.note.title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = direction,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
            )
            if (relation.preview.isNotBlank()) {
                Text(
                    text = relation.preview,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                text = location,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        TextControl(label = stringResource(R.string.action_open), onClick = onOpen)
    }
    HorizontalDivider(
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
}

@Composable
private fun SheetNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color =
            if (problem) {
                MaterialTheme.colorScheme.error
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
        modifier = Modifier.fillMaxWidth(),
    )
}
