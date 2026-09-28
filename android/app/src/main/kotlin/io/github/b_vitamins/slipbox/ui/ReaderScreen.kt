/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.ui.content.DocumentContentView
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

internal const val READER_DOCUMENT_TAG = "reader-document"

@Composable
internal fun ReaderScreen(
    phase: DocumentReaderPhase,
    settings: ReadingSettings,
    onBack: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val document = (phase as? DocumentReaderPhase.Ready)?.document
    var appearanceVisible by rememberSaveable { mutableStateOf(false) }
    val appearanceControl = remember { FocusRequester() }
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val readingWidth = minOf(maxWidth, SlipboxDimensions.readingMeasure)
        val presentation =
            rememberDocumentPresentation(
                appearance = settings.preferences.appearance,
                reduceMotion = settings.preferences.reduceMotion,
                availableWidth = readingWidth,
            )
        ReadingSurface(
            title = document?.anchor?.title ?: stringResource(R.string.reader_title),
            obscured = appearanceVisible,
            scrollable = document == null,
            contentPadding =
                if (document == null) {
                    PaddingValues(SlipboxDimensions.readingPadding)
                } else {
                    PaddingValues(0.dp)
                },
            leading = {
                IconControl(
                    icon = painterResource(R.drawable.ic_back),
                    label = stringResource(R.string.action_back),
                    onClick = onBack,
                )
            },
            trailing = {
                TextControl(
                    label = stringResource(R.string.action_appearance),
                    onClick = { appearanceVisible = true },
                    modifier = Modifier.focusRequester(appearanceControl),
                )
            },
            overlay = {
                AppearanceSheet(
                    visible = appearanceVisible,
                    settings = settings,
                    motion = motion,
                    onDismiss = { appearanceVisible = false },
                    restoreFocusTo = appearanceControl,
                )
            },
        ) {
            when (phase) {
                DocumentReaderPhase.Loading ->
                    ReaderNotice(stringResource(R.string.reader_loading))

                DocumentReaderPhase.NotFound ->
                    ReaderNotice(stringResource(R.string.reader_not_found), problem = true)

                DocumentReaderPhase.SourceUnavailable ->
                    ReaderNotice(stringResource(R.string.reader_source_unavailable), problem = true)

                is DocumentReaderPhase.UnsupportedSize ->
                    ReaderNotice(
                        text =
                            if (phase.maxLines == null) {
                                stringResource(R.string.reader_too_large)
                            } else {
                                pluralStringResource(
                                    R.plurals.reader_too_many_lines,
                                    phase.maxLines,
                                    phase.maxLines,
                                )
                            },
                        problem = true,
                    )

                DocumentReaderPhase.Failed -> {
                    ReaderNotice(stringResource(R.string.reader_unavailable), problem = true)
                    TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                }

                is DocumentReaderPhase.Ready -> {
                    ReaderMetadata(phase.document.anchor)
                    DocumentContentView(
                        source = phase.document.source,
                        presentation = presentation,
                        modifier = Modifier.fillMaxWidth().weight(1f).testTag(READER_DOCUMENT_TAG),
                    )
                }
            }
        }
    }
}

@Composable
private fun ReaderMetadata(anchor: NodeRecord) {
    val location =
        when (anchor.kind) {
            NodeKind.FILE -> anchor.filePath
            NodeKind.HEADING -> "${anchor.filePath} · ${anchor.outlinePath}"
        }
    val backlinks =
        pluralStringResource(
            R.plurals.reader_backlinks,
            anchor.backlinkCount.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
            anchor.backlinkCount,
        )
    val links =
        pluralStringResource(
            R.plurals.reader_links,
            anchor.forwardLinkCount.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
            anchor.forwardLinkCount,
        )
    val description = stringResource(R.string.reader_metadata_description, location, backlinks, links)
    Text(
        text = "$location · $backlinks · $links",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(
                    horizontal = SlipboxDimensions.readingPadding,
                    vertical = SlipboxDimensions.headerPaddingVertical,
                )
                .semantics { contentDescription = description },
    )
}

@Composable
private fun ReaderNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
