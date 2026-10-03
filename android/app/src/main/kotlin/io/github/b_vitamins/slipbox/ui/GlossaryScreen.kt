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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

internal const val GLOSSARY_LIST_TAG = "glossary-list"

@Composable
internal fun GlossaryScreen(
    phase: GlossaryInventoryPhase,
    onBack: () -> Unit,
    onActivate: () -> Unit,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenTerm: (NodeRecord) -> Unit,
    modifier: Modifier = Modifier,
) {
    LaunchedEffect(Unit) { onActivate() }
    val listState = rememberLazyListState()
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
                        GlossaryTermRow(term, onClick = { onOpenTerm(term) })
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
    }
}

@Composable
private fun GlossaryTermRow(term: NodeRecord, onClick: () -> Unit) {
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
        val context =
            if (term.aliases.isEmpty()) {
                term.outlinePath.ifBlank { term.filePath }
            } else {
                stringResource(R.string.glossary_aliases, term.aliases.joinToString(" · "))
            }
        Text(
            text = context,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun GlossaryNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
