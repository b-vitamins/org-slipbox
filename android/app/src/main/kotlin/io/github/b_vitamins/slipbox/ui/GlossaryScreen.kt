/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SingleChoiceSegmentedButtonRowScope
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

internal const val GLOSSARY_LIST_TAG = "glossary-list"
internal const val GLOSSARY_DUE_FIELD_TAG = "glossary-due-field"

@Composable
internal fun GlossaryScreen(
    phase: GlossaryInventoryPhase,
    onBack: () -> Unit,
    onActivate: () -> Unit,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenTerm: (NodeRecord) -> Unit,
    modifier: Modifier = Modifier,
    review: Boolean = false,
    reviewInput: TextFieldValue = TextFieldValue(),
    reviewPhase: GlossaryReviewPhase = GlossaryReviewPhase.Dormant,
    onShowAll: () -> Unit = {},
    onShowDue: () -> Unit = {},
    onActivateReview: () -> Unit = {},
    onReviewQueryChange: (TextFieldValue) -> Unit = {},
    onLoadMoreReview: () -> Unit = {},
    onRetryReview: () -> Unit = {},
) {
    val listState = rememberLazyListState()
    val presentedReview = remember { mutableStateOf(review) }
    LaunchedEffect(review) {
        if (presentedReview.value != review) listState.scrollToItem(0)
        presentedReview.value = review
        if (review) onActivateReview() else onActivate()
    }
    ReadingSurface(
        title = stringResource(R.string.glossary_title),
        modifier = modifier,
        scrollable = false,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = onBack,
            )
        },
    ) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxWidth().weight(1f).testTag(GLOSSARY_LIST_TAG),
        ) {
            item(key = "glossary-mode") {
                SingleChoiceSegmentedButtonRow(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .padding(bottom = SlipboxDimensions.readingPadding),
                ) {
                    GlossaryModeControl(
                        label = stringResource(R.string.glossary_all_terms),
                        selected = !review,
                        onClick = onShowAll,
                        index = 0,
                    )
                    GlossaryModeControl(
                        label = stringResource(R.string.glossary_due_terms),
                        selected = review,
                        onClick = onShowDue,
                        index = 1,
                    )
                }
            }
            if (review) {
                item(key = "glossary-due-search") {
                    OutlinedTextField(
                        value = reviewInput,
                        onValueChange = onReviewQueryChange,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .padding(bottom = SlipboxDimensions.readingPadding)
                                .testTag(GLOSSARY_DUE_FIELD_TAG),
                        placeholder = { Text(stringResource(R.string.glossary_due_search)) },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyLarge,
                        trailingIcon =
                            if (reviewInput.text.isEmpty()) {
                                null
                            } else {
                                {
                                    TextControl(
                                        label = stringResource(R.string.action_clear_search),
                                        onClick = { onReviewQueryChange(TextFieldValue()) },
                                    )
                                }
                            },
                    )
                }
                glossaryReviewItems(
                    phase = reviewPhase,
                    onLoadMore = onLoadMoreReview,
                    onRetry = onRetryReview,
                    onOpenTerm = onOpenTerm,
                )
            } else {
                glossaryInventoryItems(phase, onLoadMore, onRetry, onOpenTerm)
            }
        }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.glossaryInventoryItems(
    phase: GlossaryInventoryPhase,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenTerm: (NodeRecord) -> Unit,
) {
    when (phase) {
        GlossaryInventoryPhase.Dormant,
        GlossaryInventoryPhase.Loading,
        -> item(key = "glossary-loading") {
            GlossaryNotice(stringResource(R.string.glossary_loading))
        }

        GlossaryInventoryPhase.Empty -> item(key = "glossary-empty") {
            GlossaryNotice(stringResource(R.string.glossary_empty))
        }

        GlossaryInventoryPhase.Failed -> item(key = "glossary-failed") {
            GlossaryNotice(stringResource(R.string.glossary_unavailable), problem = true)
            TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
        }

        is GlossaryInventoryPhase.Ready -> {
            item(key = "glossary-heading") {
                Text(
                    text =
                        pluralStringResource(
                            R.plurals.glossary_terms,
                            phase.total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                            phase.total,
                        ),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier = Modifier.semantics { heading() },
                )
            }
            items(items = phase.terms, key = NodeRecord::nodeKey) { term ->
                GlossaryTermRow(term, review = false, onClick = { onOpenTerm(term) })
                HorizontalDivider(
                    thickness = SlipboxDimensions.hairline,
                    color = MaterialTheme.colorScheme.outline,
                )
            }
            if (phase.hasMore) {
                item(key = "glossary-continuation:${phase.nextPosition}") {
                    if (!phase.loadingMore && !phase.continuationFailed) {
                        LaunchedEffect(phase.nextPosition) { onLoadMore() }
                    }
                    if (phase.continuationFailed) {
                        GlossaryNotice(
                            stringResource(R.string.glossary_more_unavailable),
                            problem = true,
                        )
                        TextControl(
                            label = stringResource(R.string.action_retry),
                            onClick = onRetry,
                        )
                    } else {
                        GlossaryNotice(stringResource(R.string.glossary_loading_more))
                    }
                }
            }
        }
    }
}

@Composable
private fun SingleChoiceSegmentedButtonRowScope.GlossaryModeControl(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    index: Int,
) {
    SegmentedButton(
        selected = selected,
        onClick = onClick,
        shape = SegmentedButtonDefaults.itemShape(index = index, count = 2),
        label = { Text(label, style = MaterialTheme.typography.labelLarge) },
        modifier = Modifier.semantics { this.selected = selected },
    )
}

private fun androidx.compose.foundation.lazy.LazyListScope.glossaryReviewItems(
    phase: GlossaryReviewPhase,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenTerm: (NodeRecord) -> Unit,
) {
    when (phase) {
        GlossaryReviewPhase.Dormant,
        is GlossaryReviewPhase.Loading,
        -> item(key = "glossary-review-loading") {
            GlossaryNotice(stringResource(R.string.glossary_due_loading))
        }

        is GlossaryReviewPhase.Empty -> item(key = "glossary-review-empty:${phase.query}") {
            GlossaryNotice(
                stringResource(
                    if (phase.query.isBlank()) {
                        R.string.glossary_due_empty
                    } else {
                        R.string.glossary_due_search_empty
                    },
                ),
            )
            GlossaryReferenceDate(phase.referenceDate)
        }

        is GlossaryReviewPhase.Failed -> item(key = "glossary-review-failed:${phase.query}") {
            GlossaryNotice(stringResource(R.string.glossary_due_unavailable), problem = true)
            TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
        }

        is GlossaryReviewPhase.Ready -> {
            item(key = "glossary-review-heading:${phase.query}") {
                Text(
                    text =
                        pluralStringResource(
                            R.plurals.glossary_due_count,
                            phase.total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                            phase.total,
                            phase.referenceDate,
                        ),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier =
                        Modifier
                            .padding(bottom = SlipboxDimensions.headerPaddingVertical)
                            .semantics { heading() },
                )
            }
            items(items = phase.terms, key = NodeRecord::nodeKey) { term ->
                GlossaryTermRow(term, review = true, onClick = { onOpenTerm(term) })
                HorizontalDivider(
                    thickness = SlipboxDimensions.hairline,
                    color = MaterialTheme.colorScheme.outline,
                )
            }
            if (phase.hasMore) {
                item(key = "glossary-review-continuation:${phase.nextPosition}") {
                    if (!phase.loadingMore && !phase.continuationFailed) {
                        LaunchedEffect(phase.nextPosition) { onLoadMore() }
                    }
                    if (phase.continuationFailed) {
                        GlossaryNotice(
                            stringResource(R.string.glossary_due_more_unavailable),
                            problem = true,
                        )
                        TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                    } else {
                        GlossaryNotice(stringResource(R.string.glossary_due_loading_more))
                    }
                }
            }
        }
    }
}

@Composable
private fun GlossaryReferenceDate(referenceDate: String) {
    Text(
        text = stringResource(R.string.glossary_due_reference_date, referenceDate),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun GlossaryTermRow(term: NodeRecord, review: Boolean, onClick: () -> Unit) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .clickable(onClick = onClick)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        Text(
            text = term.title,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
        val context = if (review) dueStanding(term) else glossaryContext(term)
        Text(
            text = context,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun glossaryContext(term: NodeRecord): String =
    if (term.aliases.isEmpty()) {
        term.outlinePath.ifBlank { term.filePath }
    } else {
        stringResource(R.string.glossary_aliases, term.aliases.joinToString(" · "))
    }

@Composable
private fun dueStanding(term: NodeRecord): String =
    when {
        term.reviewFields().all(String?::isNullOrBlank) ->
            stringResource(R.string.glossary_due_never_reviewed)
        term.srDue.isNullOrBlank() -> stringResource(R.string.glossary_due_without_date)
        else -> stringResource(R.string.glossary_due_on, term.srDue)
    }

internal fun NodeRecord.reviewFields(): List<String?> =
    listOf(srDue, srInterval, srReps, srEase, srLast)

@Composable
private fun GlossaryNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
