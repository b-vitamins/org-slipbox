/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.CorpusSearchEntity
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

internal const val CORPUS_SEARCH_FIELD_TAG = "corpus-search-field"
internal const val CORPUS_SEARCH_LIST_TAG = "corpus-search-list"

@Composable
internal fun CorpusSearchField(
    input: TextFieldValue,
    onChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = input,
        onValueChange = onChange,
        modifier = modifier.fillMaxWidth().testTag(CORPUS_SEARCH_FIELD_TAG),
        placeholder = { Text(stringResource(R.string.search_placeholder)) },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge,
        trailingIcon =
            if (input.text.isEmpty()) {
                null
            } else {
                {
                    TextControl(
                        label = stringResource(R.string.action_clear_search),
                        onClick = { onChange(TextFieldValue()) },
                    )
                }
            },
    )
}

@Composable
internal fun CorpusSearchResults(
    phase: CorpusSearchPhase,
    selectedNodeKey: String?,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpen: (CorpusSearchHit) -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth().testTag(CORPUS_SEARCH_LIST_TAG),
    ) {
        when (phase) {
            CorpusSearchPhase.Dormant -> Unit
            is CorpusSearchPhase.Searching -> item(key = "searching:${phase.query}") {
                SearchNotice(stringResource(R.string.search_loading))
            }
            is CorpusSearchPhase.Empty -> item(key = "search-empty:${phase.query}") {
                SearchNotice(stringResource(R.string.search_empty))
            }
            is CorpusSearchPhase.Failed -> item(key = "search-failed:${phase.query}") {
                SearchNotice(stringResource(R.string.search_unavailable), problem = true)
                TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
            }
            is CorpusSearchPhase.Ready -> {
                item(key = "search-count:${phase.query}") {
                    val count = phase.hits.size
                    Text(
                        text =
                            if (phase.queryTruncated) {
                                pluralStringResource(
                                    R.plurals.search_results_bounded,
                                    phase.queryBound.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                                    count,
                                    phase.queryBound,
                                )
                            } else {
                                pluralStringResource(
                                    R.plurals.search_results_loaded,
                                    phase.total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
                                    count,
                                    phase.total,
                                )
                            },
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onBackground,
                        modifier = Modifier.semantics { heading() },
                    )
                }
                items(items = phase.hits, key = { it.node.nodeKey }) { hit ->
                    CorpusSearchRow(
                        hit = hit,
                        selected = hit.node.nodeKey == selectedNodeKey,
                        onClick = { onOpen(hit) },
                    )
                    HorizontalDivider(
                        thickness = SlipboxDimensions.hairline,
                        color = MaterialTheme.colorScheme.outline,
                    )
                }
                if (phase.hasMore) {
                    item(key = "search-continuation:${phase.nextPosition}") {
                        if (!phase.loadingMore && !phase.continuationFailed) {
                            LaunchedEffect(phase.nextPosition) { onLoadMore() }
                        }
                        if (phase.continuationFailed) {
                            SearchNotice(
                                stringResource(R.string.search_more_unavailable),
                                problem = true,
                            )
                            TextControl(
                                label = stringResource(R.string.action_retry),
                                onClick = onRetry,
                            )
                        } else {
                            SearchNotice(stringResource(R.string.search_loading_more))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CorpusSearchRow(
    hit: CorpusSearchHit,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val selectedColor = MaterialTheme.colorScheme.tertiaryContainer.copy(alpha = 0.28f)
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .background(if (selected) selectedColor else MaterialTheme.colorScheme.surface)
                .semantics { this.selected = selected }
                .clickable(onClick = onClick)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
    ) {
        Row(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = highlighted(hit.title.segments),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                text =
                    stringResource(
                        when (hit.entity) {
                            CorpusSearchEntity.NOTE -> R.string.search_kind_note
                            CorpusSearchEntity.GLOSSARY -> R.string.search_kind_glossary
                        },
                    ),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(start = SlipboxDimensions.headerPaddingHorizontal),
            )
        }
        val aliases = hit.aliases.segments.takeIf(List<ContentSegment>::hasMatch)
        val excerpt = hit.excerpt.segments.takeIf(List<ContentSegment>::hasMatch)
        when {
            aliases != null -> SearchContext(highlighted(aliases))
            excerpt != null -> SearchContext(highlighted(excerpt))
            else -> SearchContext(AnnotatedString(hit.node.outlinePath.ifBlank { hit.node.filePath }))
        }
    }
}

@Composable
private fun SearchContext(text: AnnotatedString) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 3,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.fillMaxWidth().padding(top = SlipboxDimensions.headerPaddingVertical / 2),
    )
}

@Composable
private fun highlighted(segments: List<ContentSegment>): AnnotatedString {
    val mark =
        SpanStyle(
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            background = MaterialTheme.colorScheme.tertiaryContainer,
            fontWeight = FontWeight.Medium,
        )
    return buildAnnotatedString {
        segments.forEach { segment ->
            if (segment.matched) {
                withStyle(mark) { append(segment.text) }
            } else {
                append(segment.text)
            }
        }
    }
}

private fun List<ContentSegment>.hasMatch(): Boolean = any(ContentSegment::matched)

@Composable
private fun SearchNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
