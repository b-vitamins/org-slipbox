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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.NotePlaceNeighbor
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion

@Composable
internal fun ReaderContextSheet(
    document: ReaderDocument?,
    kind: ReaderSurfaceKind = ReaderSurfaceKind.Note,
    visible: Boolean,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    onHeading: (Int) -> Unit,
    onNeighbor: (NotePlaceNeighbor) -> Unit,
    onExplore: () -> Unit,
    onRelations: () -> Unit,
    onAppearance: () -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    ContextualSheet(
        title =
            stringResource(
                if (kind == ReaderSurfaceKind.GlossaryTerm) {
                    R.string.glossary_term_details
                } else {
                    R.string.reader_note_details
                },
            ),
        visible = visible,
        motion = motion,
        onDismiss = onDismiss,
        restoreFocusTo = restoreFocusTo,
    ) {
        val note = document ?: return@ContextualSheet
        Text(
            text = note.anchor.title,
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.fillMaxWidth().semantics { heading() },
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingHorizontal),
        ) {
            if (kind == ReaderSurfaceKind.Note) {
                TextControl(
                    label = stringResource(R.string.reader_explore),
                    onClick = onExplore,
                )
                TextControl(
                    label = stringResource(R.string.reader_relations),
                    onClick = onRelations,
                )
            }
            TextControl(
                label = stringResource(R.string.action_appearance),
                onClick = onAppearance,
            )
        }

        if (kind == ReaderSurfaceKind.GlossaryTerm) {
            ContextSectionTitle(stringResource(R.string.glossary_identity))
            MetadataRow(stringResource(R.string.glossary_canonical_term), note.anchor.title)
            if (note.anchor.aliases.isNotEmpty()) {
                MetadataRow(
                    stringResource(R.string.glossary_alias_label),
                    note.anchor.aliases.joinToString(" · "),
                )
            }
            MetadataRow(
                label = stringResource(R.string.glossary_node_key),
                value = note.anchor.nodeKey,
                monospace = true,
            )
        }

        val outline =
            if (kind == ReaderSurfaceKind.Note) {
                note.outline.withIndex().filter { (_, anchor) ->
                    anchor.nodeKey != note.anchor.nodeKey
                }
            } else {
                emptyList()
            }
        if (outline.isNotEmpty()) {
            ContextSectionTitle(stringResource(R.string.reader_outline))
            val baseLevel = outline.minOf { (_, anchor) -> anchor.level }
            outline.forEach { (index, anchor) ->
                OutlineRow(
                    anchor = anchor,
                    depth = (anchor.level - baseLevel).toInt().coerceIn(0, 4),
                    current = anchor.nodeKey == note.addressedAnchor.nodeKey,
                    onClick = { onHeading(index) },
                )
            }
        }

        val place = note.place.takeIf { kind == ReaderSurfaceKind.Note }
        val neighbors = listOfNotNull(place?.earlier, place?.later)
        if (place != null && place.total > 1 && neighbors.isNotEmpty()) {
            ContextSectionTitle(
                stringResource(R.string.reader_filed_position, place.ordinal, place.total),
            )
            place.earlier?.let { neighbor ->
                NeighborRow(
                    side = stringResource(R.string.reader_filed_before),
                    neighbor = neighbor,
                    onClick = { onNeighbor(neighbor) },
                )
            }
            place.later?.let { neighbor ->
                NeighborRow(
                    side = stringResource(R.string.reader_filed_after),
                    neighbor = neighbor,
                    onClick = { onNeighbor(neighbor) },
                )
            }
        }

        ContextSectionTitle(stringResource(R.string.reader_source_details))
        MetadataRow(stringResource(R.string.reader_source), note.sourceName)
        if (kind == ReaderSurfaceKind.GlossaryTerm) {
            MetadataRow(
                label = stringResource(R.string.glossary_generation),
                value = note.source.binding.generation,
                monospace = true,
            )
        }
        MetadataRow(stringResource(R.string.reader_file), note.anchor.filePath)
        note.anchor.outlinePath.takeIf(String::isNotBlank)?.let { path ->
            MetadataRow(stringResource(R.string.reader_outline_path), path)
        }
        MetadataRow(
            label = stringResource(R.string.reader_revision),
            value = note.revision,
            monospace = true,
        )
    }
}

@Composable
private fun ContextSectionTitle(title: String) {
    HorizontalDivider(
        modifier = Modifier.padding(top = SlipboxDimensions.readingPadding),
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onBackground,
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(top = SlipboxDimensions.headerPaddingVertical)
                .semantics { heading() },
    )
}

@Composable
private fun OutlineRow(
    anchor: NodeRecord,
    depth: Int,
    current: Boolean,
    onClick: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(role = Role.Button, onClick = onClick)
                .padding(start = (depth * 16).dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = anchor.title,
            style = MaterialTheme.typography.bodyLarge,
            color =
                if (current) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onBackground
                },
            modifier = Modifier.weight(1f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (current) {
            Text(
                text = stringResource(R.string.reader_current_heading),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
            )
        }
    }
}

@Composable
private fun NeighborRow(
    side: String,
    neighbor: NotePlaceNeighbor,
    onClick: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(role = Role.Button, onClick = onClick)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        Text(
            text = side,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            text = neighbor.title,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun MetadataRow(label: String, value: String, monospace: Boolean = false) {
    if (value.isBlank()) return
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(top = SlipboxDimensions.headerPaddingVertical),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onBackground,
            fontFamily = if (monospace) FontFamily.Monospace else FontFamily.Default,
        )
    }
}
