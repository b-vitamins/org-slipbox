/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.ExplorationEntry
import io.github.b_vitamins.slipbox.engine.ExplorationExplanation
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.ExplorationSectionKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.PlanningField
import io.github.b_vitamins.slipbox.engine.PlanningRelationRecord
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion

@Composable
internal fun ReaderExplorationSheet(
    visible: Boolean,
    phase: ReaderExplorationPhase,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    onSelect: (ExplorationLens) -> Unit,
    onRefresh: () -> Unit,
    onPreview: (NodeRecord) -> Unit,
    onOpen: (NodeRecord) -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    ContextualSheet(
        title = stringResource(R.string.reader_explore),
        visible = visible,
        motion = motion,
        onDismiss = onDismiss,
        restoreFocusTo = restoreFocusTo,
    ) {
        ExplorationLensRow(selected = phase.lens(), onSelect = onSelect)
        HorizontalDivider(
            modifier = Modifier.padding(bottom = SlipboxDimensions.headerPaddingVertical),
            thickness = SlipboxDimensions.hairline,
            color = MaterialTheme.colorScheme.outline,
        )
        when (phase) {
            ReaderExplorationPhase.AwaitingLens ->
                SheetNotice(stringResource(R.string.reader_explore_choose_lens))
            is ReaderExplorationPhase.Loading ->
                SheetNotice(
                    stringResource(
                        R.string.reader_explore_loading,
                        explorationLensLabel(phase.lens),
                    ),
                )
            is ReaderExplorationPhase.NoSubstrate -> {
                SheetNotice(noSubstrateLabel(phase.lens))
                RefreshControl(onRefresh)
            }
            is ReaderExplorationPhase.Empty -> {
                SheetNotice(stringResource(R.string.reader_explore_empty))
                RefreshControl(onRefresh)
            }
            is ReaderExplorationPhase.Failed -> {
                SheetNotice(
                    stringResource(
                        R.string.reader_explore_unavailable,
                        explorationLensLabel(phase.lens),
                    ),
                    problem = true,
                )
                RefreshControl(onRefresh)
            }
            is ReaderExplorationPhase.Ready -> {
                phase.result.sections.forEach { section ->
                    Text(
                        text = explorationSectionLabel(section.kind),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .padding(top = SlipboxDimensions.headerPaddingVertical)
                                .semantics { heading() },
                    )
                    if (section.entries.isEmpty()) {
                        SheetNotice(stringResource(R.string.reader_explore_section_empty))
                    } else {
                        section.entries.forEach { entry ->
                            ExplorationEntryRow(
                                entry = entry,
                                onPreview = { onPreview(entry.target()) },
                                onOpen = { onOpen(entry.target()) },
                            )
                        }
                    }
                }
                if (phase.result.sections.any { it.entries.size >= EXPLORATION_QUERY_LIMIT }) {
                    SheetNotice(
                        pluralStringResource(
                            R.plurals.reader_explore_query_bound,
                            EXPLORATION_QUERY_LIMIT,
                            EXPLORATION_QUERY_LIMIT,
                        ),
                    )
                }
                RefreshControl(onRefresh)
            }
        }
    }
}

@Composable
private fun ExplorationLensRow(
    selected: ExplorationLens?,
    onSelect: (ExplorationLens) -> Unit,
) {
    FlowRow(
        modifier =
            Modifier
                .fillMaxWidth()
                .selectableGroup(),
        horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingVertical),
        verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingVertical),
        maxItemsInEachRow = 4,
    ) {
        ExplorationLens.entries.forEach { lens ->
            val marked = lens == selected
            Box(
                modifier =
                    Modifier
                        .defaultMinSize(minHeight = SlipboxDimensions.touchTarget)
                        .background(
                            color =
                                if (marked) {
                                    MaterialTheme.colorScheme.primaryContainer
                                } else {
                                    MaterialTheme.colorScheme.surface
                                },
                            shape = MaterialTheme.shapes.small,
                        )
                        .selectable(
                            selected = marked,
                            role = Role.RadioButton,
                            onClick = { onSelect(lens) },
                        )
                        .padding(
                            horizontal = SlipboxDimensions.headerPaddingHorizontal / 2,
                        ),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = explorationLensLabel(lens),
                    style = MaterialTheme.typography.labelLarge,
                    color =
                        if (marked) {
                            MaterialTheme.colorScheme.onPrimaryContainer
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                )
            }
        }
    }
}

@Composable
private fun ExplorationEntryRow(
    entry: ExplorationEntry,
    onPreview: () -> Unit,
    onOpen: () -> Unit,
) {
    val target = entry.target()
    val previewLabel = stringResource(R.string.reader_relation_preview, target.title)
    Row(
        modifier = Modifier.fillMaxWidth(),
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
                text = target.title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = explorationExplanation(entry.explanation()),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        TextControl(label = stringResource(R.string.action_open), onClick = onOpen)
    }
    HorizontalDivider(
        thickness = SlipboxDimensions.hairline,
        color = MaterialTheme.colorScheme.outline,
    )
}

private fun ExplorationEntry.target(): NodeRecord =
    when (this) {
        is ExplorationEntry.Backlink -> record.sourceAnchor ?: record.sourceNote
        is ExplorationEntry.ForwardLink -> record.destinationNote
        is ExplorationEntry.Reflink -> record.sourceAnchor
        is ExplorationEntry.UnlinkedReference -> record.sourceAnchor
        is ExplorationEntry.Anchor -> record.anchor
    }

private fun ExplorationEntry.explanation(): ExplorationExplanation =
    when (this) {
        is ExplorationEntry.Backlink -> record.explanation
        is ExplorationEntry.ForwardLink -> record.explanation
        is ExplorationEntry.Reflink -> record.explanation
        is ExplorationEntry.UnlinkedReference -> record.explanation
        is ExplorationEntry.Anchor -> record.explanation
    }

@Composable
private fun explorationExplanation(explanation: ExplorationExplanation): String =
    when (explanation) {
        ExplorationExplanation.Backlink -> stringResource(R.string.reader_explore_backlink)
        ExplorationExplanation.ForwardLink -> stringResource(R.string.reader_explore_forward_link)
        is ExplorationExplanation.SharedReference ->
            stringResource(R.string.reader_explore_shared_reference, explanation.reference)
        is ExplorationExplanation.UnlinkedReference ->
            stringResource(R.string.reader_explore_unlinked_reference)
        is ExplorationExplanation.TimeNeighbor -> planningRelationsLabel(explanation.relations)
        is ExplorationExplanation.TaskNeighbor -> {
            val details = mutableListOf<String>()
            explanation.sharedTodoKeyword?.let {
                details += stringResource(R.string.reader_explore_task_state, it)
            }
            for (relation in explanation.planningRelations) {
                details += planningRelationLabel(relation)
            }
            details.joinToString(" · ")
        }
        is ExplorationExplanation.BridgeCandidate ->
            evidenceLabel(explanation.references, explanation.viaNotes.map { it.title })
        is ExplorationExplanation.DormantSharedReference ->
            listOf(
                    stringResource(R.string.reader_explore_older_source),
                    evidenceLabel(explanation.references, explanation.viaNotes.map { it.title }),
                )
                .filter { it.isNotBlank() }
                .joinToString(" · ")
        is ExplorationExplanation.UnresolvedSharedReference ->
            listOf(
                    stringResource(
                        R.string.reader_explore_task_state,
                        explanation.todoKeyword,
                    ),
                    referencesLabel(explanation.references),
                )
                .filter { it.isNotBlank() }
                .joinToString(" · ")
        is ExplorationExplanation.WeaklyIntegratedSharedReference ->
            listOf(
                    pluralStringResource(
                        R.plurals.reader_explore_structural_links,
                        explanation.structuralLinkCount.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                        explanation.structuralLinkCount,
                    ),
                    evidenceLabel(explanation.references, explanation.viaNotes.map { it.title }),
                )
                .filter { it.isNotBlank() }
                .joinToString(" · ")
    }

@Composable
private fun evidenceLabel(references: List<String>, via: List<String>): String =
    listOf(referencesLabel(references), viaLabel(via)).filter { it.isNotBlank() }.joinToString(" · ")

@Composable
private fun referencesLabel(references: List<String>): String =
    references
        .takeIf { it.isNotEmpty() }
        ?.joinToString(", ")
        ?.let { stringResource(R.string.reader_explore_references, it) }
        .orEmpty()

@Composable
private fun viaLabel(via: List<String>): String =
    via
        .takeIf { it.isNotEmpty() }
        ?.joinToString(", ")
        ?.let { stringResource(R.string.reader_explore_via, it) }
        .orEmpty()

@Composable
private fun planningRelationLabel(relation: PlanningRelationRecord): String =
    stringResource(
        R.string.reader_explore_planning_relation,
        planningFieldLabel(relation.sourceField),
        planningFieldLabel(relation.candidateField),
        relation.date,
    )

@Composable
private fun planningRelationsLabel(relations: List<PlanningRelationRecord>): String {
    val labels = mutableListOf<String>()
    for (relation in relations) labels += planningRelationLabel(relation)
    return labels.joinToString("; ")
}

@Composable
private fun planningFieldLabel(field: PlanningField): String =
    stringResource(
        when (field) {
            PlanningField.SCHEDULED -> R.string.reader_explore_scheduled
            PlanningField.DEADLINE -> R.string.reader_explore_deadline
        },
    )

@Composable
private fun explorationLensLabel(lens: ExplorationLens): String =
    stringResource(
        when (lens) {
            ExplorationLens.STRUCTURE -> R.string.reader_explore_lens_structure
            ExplorationLens.REFS -> R.string.reader_explore_lens_refs
            ExplorationLens.TIME -> R.string.reader_explore_lens_time
            ExplorationLens.TASKS -> R.string.reader_explore_lens_tasks
            ExplorationLens.BRIDGES -> R.string.reader_explore_lens_bridges
            ExplorationLens.DORMANT -> R.string.reader_explore_lens_dormant
            ExplorationLens.UNRESOLVED -> R.string.reader_explore_lens_unresolved
        },
    )

@Composable
private fun explorationSectionLabel(kind: ExplorationSectionKind): String =
    stringResource(
        when (kind) {
            ExplorationSectionKind.BACKLINKS -> R.string.reader_explore_section_backlinks
            ExplorationSectionKind.FORWARD_LINKS -> R.string.reader_explore_section_forward_links
            ExplorationSectionKind.REFLINKS -> R.string.reader_explore_section_reflinks
            ExplorationSectionKind.UNLINKED_REFERENCES ->
                R.string.reader_explore_section_unlinked_references
            ExplorationSectionKind.TIME_NEIGHBORS -> R.string.reader_explore_section_time_neighbors
            ExplorationSectionKind.TASK_NEIGHBORS -> R.string.reader_explore_section_task_neighbors
            ExplorationSectionKind.BRIDGE_CANDIDATES ->
                R.string.reader_explore_section_bridge_candidates
            ExplorationSectionKind.DORMANT_NOTES -> R.string.reader_explore_section_dormant_notes
            ExplorationSectionKind.UNRESOLVED_TASKS ->
                R.string.reader_explore_section_unresolved_tasks
            ExplorationSectionKind.WEAKLY_INTEGRATED_NOTES ->
                R.string.reader_explore_section_weakly_integrated_notes
        },
    )

@Composable
private fun noSubstrateLabel(lens: ExplorationLens): String =
    stringResource(
        when (lens) {
            ExplorationLens.TIME -> R.string.reader_explore_no_time_substrate
            ExplorationLens.TASKS -> R.string.reader_explore_no_task_substrate
            ExplorationLens.BRIDGES -> R.string.reader_explore_no_relation_substrate
            ExplorationLens.DORMANT -> R.string.reader_explore_no_dormant_substrate
            ExplorationLens.UNRESOLVED -> R.string.reader_explore_no_relation_substrate
            ExplorationLens.STRUCTURE,
            ExplorationLens.REFS,
            -> R.string.reader_explore_empty
        },
    )
