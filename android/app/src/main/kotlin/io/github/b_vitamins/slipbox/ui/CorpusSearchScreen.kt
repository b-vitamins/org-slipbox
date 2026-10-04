/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.ui.content.CorpusSearchContentView
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation

internal const val CORPUS_SEARCH_FIELD_TAG = "corpus-search-field"
internal const val CORPUS_SEARCH_LIST_TAG = "corpus-search-list"

@Composable
internal fun CorpusSearchField(
    input: TextFieldValue,
    onChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(13.dp)
    Row(
        modifier =
            modifier
                .fillMaxWidth()
                .height(48.dp)
                .clip(shape)
                .background(MaterialTheme.colorScheme.surface)
                .border(
                    width = 1.dp,
                    color = MaterialTheme.colorScheme.outline,
                    shape = shape,
                )
                .padding(start = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_search),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(18.dp),
        )
        Spacer(Modifier.width(9.dp))
        BasicTextField(
            value = input,
            onValueChange = onChange,
            modifier =
                Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .testTag(CORPUS_SEARCH_FIELD_TAG),
            singleLine = true,
            textStyle =
                MaterialTheme.typography.bodyLarge.copy(
                    color = MaterialTheme.colorScheme.onSurface,
                ),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            decorationBox = { field ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (input.text.isEmpty()) {
                        Text(
                            text = stringResource(R.string.search_placeholder),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    field()
                }
            },
        )
        if (input.text.isNotEmpty()) {
            IconControl(
                icon = painterResource(R.drawable.ic_close),
                label = stringResource(R.string.action_clear_search),
                onClick = { onChange(TextFieldValue()) },
            )
        } else {
            Spacer(Modifier.width(12.dp))
        }
    }
}

@Composable
internal fun CorpusSearchResults(
    phase: CorpusSearchPhase,
    presentation: DocumentPresentation,
    onRetry: () -> Unit,
    onOpen: (CorpusSearchHit) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (phase is CorpusSearchPhase.Ready) {
        CorpusSearchContentView(
            hits = phase.hits,
            presentation = presentation,
            onOpen = onOpen,
            modifier = modifier.fillMaxWidth().testTag(CORPUS_SEARCH_LIST_TAG),
        )
        return
    }
    LazyColumn(
        modifier = modifier.fillMaxWidth().testTag(CORPUS_SEARCH_LIST_TAG),
        contentPadding = PaddingValues(top = 12.dp, bottom = 32.dp),
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
            is CorpusSearchPhase.Ready -> Unit
        }
    }
}

@Composable
private fun SearchNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(vertical = 8.dp),
    )
}
