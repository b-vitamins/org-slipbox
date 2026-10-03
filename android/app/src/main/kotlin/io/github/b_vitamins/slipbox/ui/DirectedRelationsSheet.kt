/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.DirectedRelationDirection
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion

@Composable
internal fun DirectedRelationsSheet(
    noteKey: String?,
    visible: Boolean,
    phase: DirectedRelationsPhase,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    onRetry: () -> Unit,
    onLoadMore: () -> Unit,
    onPreview: (DirectedRelationRecord) -> Unit,
    onOpen: (DirectedRelationRecord) -> Unit,
    relatedPhase: RelatedDiscoveryPhase,
    mentionPhase: MentionDiscoveryPhase,
    onRevealRelated: () -> Unit,
    onRefreshRelated: () -> Unit,
    onShowAllRelated: () -> Unit,
    onRevealMentions: () -> Unit,
    onRefreshMentions: () -> Unit,
    onShowAllMentions: () -> Unit,
    onPreviewDiscovered: (NodeRecord) -> Unit,
    onOpenDiscovered: (NodeRecord) -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    var relatedExpanded by rememberSaveable(noteKey) { mutableStateOf(false) }
    var mentionsExpanded by rememberSaveable(noteKey) { mutableStateOf(false) }
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
        val listed =
            (phase as? DirectedRelationsPhase.Ready)
                ?.relations
                ?.mapTo(mutableSetOf()) { it.note.nodeKey }
                .orEmpty()
        DiscoverySection(
            title = stringResource(R.string.reader_related_discovery),
            expanded = relatedExpanded,
            onToggle = {
                relatedExpanded = !relatedExpanded
                if (relatedExpanded) onRevealRelated()
            },
        ) {
            RelatedDiscoveryContent(
                phase = relatedPhase,
                listed = listed,
                onRefresh = onRefreshRelated,
                onShowAll = onShowAllRelated,
                onPreview = onPreviewDiscovered,
                onOpen = onOpenDiscovered,
            )
        }
        DiscoverySection(
            title = stringResource(R.string.reader_unlinked_mentions),
            expanded = mentionsExpanded,
            onToggle = {
                mentionsExpanded = !mentionsExpanded
                if (mentionsExpanded) onRevealMentions()
            },
        ) {
            MentionDiscoveryContent(
                phase = mentionPhase,
                listed = listed,
                onRefresh = onRefreshMentions,
                onShowAll = onShowAllMentions,
                onPreview = onPreviewDiscovered,
                onOpen = onOpenDiscovered,
            )
        }
    }
}

@Composable
private fun DiscoverySection(
    title: String,
    expanded: Boolean,
    onToggle: () -> Unit,
    content: @Composable () -> Unit,
) {
    HorizontalDivider(
        modifier = Modifier.padding(top = SlipboxDimensions.readingPadding),
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .toggleable(value = expanded, role = Role.Button, onValueChange = { onToggle() })
                .padding(vertical = SlipboxDimensions.headerPaddingVertical)
                .semantics { heading() },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f),
        )
        Text(
            text =
                stringResource(
                    if (expanded) R.string.action_hide_section else R.string.action_show_section,
                ),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
        )
    }
    if (expanded) content()
}

@Composable
private fun RelatedDiscoveryContent(
    phase: RelatedDiscoveryPhase,
    listed: Set<String>,
    onRefresh: () -> Unit,
    onShowAll: () -> Unit,
    onPreview: (NodeRecord) -> Unit,
    onOpen: (NodeRecord) -> Unit,
) {
    when (phase) {
        RelatedDiscoveryPhase.Idle,
        RelatedDiscoveryPhase.Loading,
        -> SheetNotice(stringResource(R.string.reader_related_loading))

        RelatedDiscoveryPhase.Empty -> {
            SheetNotice(stringResource(R.string.reader_related_empty))
            RefreshControl(onRefresh)
        }

        RelatedDiscoveryPhase.Failed -> {
            SheetNotice(stringResource(R.string.reader_related_unavailable), problem = true)
            RefreshControl(onRefresh)
        }

        is RelatedDiscoveryPhase.Ready -> {
            val discovery = relatedDiscovery(phase.result, listed)
            if (discovery.noteCount == 0) {
                SheetNotice(stringResource(R.string.reader_related_empty))
            } else {
                val groups =
                    if (phase.showAll) discovery.groups else discovery.bounded(RELATED_DISPLAY_LIMIT)
                groups.forEach { group ->
                    Text(
                        text = stringResource(R.string.reader_related_via, group.connector.title),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    group.notes.forEach { related ->
                        DiscoveredNoteRow(
                            note = related.note,
                            detail =
                                related.explanation.viaNotes.size.takeIf { it > 1 }?.let { count ->
                                    pluralStringResource(
                                        R.plurals.reader_related_connectors,
                                        count,
                                        count,
                                    )
                                },
                            onPreview = { onPreview(related.note) },
                            onOpen = { onOpen(related.note) },
                        )
                    }
                }
                val hidden = discovery.noteCount - groups.sumOf { it.notes.size }
                if (hidden > 0) {
                    TextControl(
                        label =
                            pluralStringResource(
                                R.plurals.reader_show_more_related,
                                hidden,
                                hidden,
                            ),
                        onClick = onShowAll,
                    )
                }
                if (discovery.rankedCount >= RELATED_QUERY_LIMIT) {
                    SheetNotice(
                        pluralStringResource(
                            R.plurals.reader_related_query_bound,
                            RELATED_QUERY_LIMIT,
                            RELATED_QUERY_LIMIT,
                        ),
                    )
                }
            }
            RefreshControl(onRefresh)
        }
    }
}

@Composable
private fun MentionDiscoveryContent(
    phase: MentionDiscoveryPhase,
    listed: Set<String>,
    onRefresh: () -> Unit,
    onShowAll: () -> Unit,
    onPreview: (NodeRecord) -> Unit,
    onOpen: (NodeRecord) -> Unit,
) {
    when (phase) {
        MentionDiscoveryPhase.Idle,
        MentionDiscoveryPhase.Loading,
        -> SheetNotice(stringResource(R.string.reader_mentions_loading))

        MentionDiscoveryPhase.Empty -> {
            SheetNotice(stringResource(R.string.reader_mentions_empty))
            RefreshControl(onRefresh)
        }

        MentionDiscoveryPhase.Failed -> {
            SheetNotice(stringResource(R.string.reader_mentions_unavailable), problem = true)
            RefreshControl(onRefresh)
        }

        is MentionDiscoveryPhase.Ready -> {
            val groups = mentionGroups(phase.result, listed)
            if (groups.isEmpty()) {
                SheetNotice(stringResource(R.string.reader_mentions_empty))
            } else {
                val shown = if (phase.showAll) groups else groups.take(MENTIONS_DISPLAY_LIMIT)
                shown.forEach { group ->
                    MentionGroupRow(
                        group = group,
                        onPreview = { onPreview(group.note) },
                        onOpen = { onOpen(group.note) },
                    )
                }
                val hidden = groups.size - shown.size
                if (hidden > 0) {
                    TextControl(
                        label =
                            pluralStringResource(
                                R.plurals.reader_show_more_mention_sources,
                                hidden,
                                hidden,
                            ),
                        onClick = onShowAll,
                    )
                }
                if (phase.result.unlinkedReferences.size >= MENTIONS_QUERY_LIMIT) {
                    SheetNotice(
                        pluralStringResource(
                            R.plurals.reader_mentions_query_bound,
                            MENTIONS_QUERY_LIMIT,
                            MENTIONS_QUERY_LIMIT,
                        ),
                    )
                }
            }
            RefreshControl(onRefresh)
        }
    }
}

@Composable
internal fun RefreshControl(onRefresh: () -> Unit) {
    TextControl(label = stringResource(R.string.action_refresh), onClick = onRefresh)
}

@Composable
private fun DiscoveredNoteRow(
    note: NodeRecord,
    detail: String?,
    onPreview: () -> Unit,
    onOpen: () -> Unit,
) {
    val previewLabel = stringResource(R.string.reader_relation_preview, note.title)
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
                text = note.title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (detail != null) {
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        TextControl(label = stringResource(R.string.action_open), onClick = onOpen)
    }
}

@Composable
private fun MentionGroupRow(
    group: MentionGroup,
    onPreview: () -> Unit,
    onOpen: () -> Unit,
) {
    val previewLabel = stringResource(R.string.reader_relation_preview, group.note.title)
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = SlipboxDimensions.touchTarget),
        verticalAlignment = Alignment.Top,
    ) {
        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .clickable(onClickLabel = previewLabel, onClick = onPreview)
                    .padding(vertical = SlipboxDimensions.headerPaddingVertical),
        ) {
            Text(
                text = group.note.title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Text(
                text =
                    pluralStringResource(
                        R.plurals.reader_mentions_in_note,
                        group.occurrences.size,
                        group.occurrences.size,
                    ),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
            )
            group.occurrences.forEach { occurrence -> HighlightedExcerpt(occurrence.excerpt) }
        }
        TextControl(label = stringResource(R.string.action_open), onClick = onOpen)
    }
    HorizontalDivider(
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
}

@Composable
internal fun HighlightedExcerpt(segments: List<ContentSegment>) {
    val match =
        SpanStyle(
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            background = MaterialTheme.colorScheme.tertiaryContainer,
            fontWeight = FontWeight.Medium,
        )
    Text(
        text =
            buildAnnotatedString {
                segments.forEach { segment ->
                    if (segment.matched) {
                        withStyle(match) { append(segment.text) }
                    } else {
                        append(segment.text)
                    }
                }
            },
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(top = SlipboxDimensions.headerPaddingVertical / 2),
    )
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
internal fun SheetNotice(text: String, problem: Boolean = false) {
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
