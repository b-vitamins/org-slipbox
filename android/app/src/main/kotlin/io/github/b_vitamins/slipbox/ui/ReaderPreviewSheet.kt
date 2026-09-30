/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.zIndex
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.content.DocumentContentView
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion

internal const val READER_PREVIEW_SHEET_TAG = "reader-preview-sheet"
internal const val READER_PREVIEW_DOCUMENT_TAG = "reader-preview-document"

/** A bounded native reveal around the same document renderer used by the reader. */
@Composable
internal fun ReaderPreviewSheet(
    phase: ReaderPreviewPhase,
    presentation: DocumentPresentation,
    motion: SlipboxMotion,
    documentHeight: Dp,
    onDismiss: () -> Unit,
    onOpen: () -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    val preview = (phase as? ReaderPreviewPhase.Ready)?.preview
    val paneTitle =
        when {
            preview?.anchor?.glossary == true -> stringResource(R.string.reader_preview_glossary)
            preview != null -> stringResource(R.string.reader_preview_note)
            else -> stringResource(R.string.reader_preview_loading)
        }
    ContextualSheet(
        title = paneTitle,
        visible = phase != ReaderPreviewPhase.Hidden,
        motion = motion,
        onDismiss = onDismiss,
        modifier = Modifier.testTag(READER_PREVIEW_SHEET_TAG),
        restoreFocusTo = restoreFocusTo,
    ) {
        when (phase) {
            ReaderPreviewPhase.Hidden -> Unit
            is ReaderPreviewPhase.Loading ->
                Text(
                    text = stringResource(R.string.reader_preview_loading),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            is ReaderPreviewPhase.Ready -> {
                Text(
                    text = phase.preview.anchor.title,
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.onBackground,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .zIndex(1f)
                            .semantics { heading() },
                )
                DocumentContentView(
                    source = phase.preview.source,
                    presentation = presentation,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .height(documentHeight)
                            .padding(top = SlipboxDimensions.headerPaddingVertical)
                            .testTag(READER_PREVIEW_DOCUMENT_TAG),
                )
                if (phase.preview.shortened) {
                    Text(
                        text = stringResource(R.string.reader_preview_shortened),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement =
                        Arrangement.spacedBy(
                            SlipboxDimensions.headerPaddingHorizontal,
                            Alignment.End,
                        ),
                ) {
                    TextControl(
                        label = stringResource(R.string.action_dismiss),
                        onClick = onDismiss,
                    )
                    TextControl(
                        label =
                            stringResource(
                                if (phase.preview.anchor.glossary) {
                                    R.string.reader_preview_open_term
                                } else {
                                    R.string.reader_preview_open_note
                                },
                            ),
                        onClick = onOpen,
                    )
                }
            }
        }
    }
}
