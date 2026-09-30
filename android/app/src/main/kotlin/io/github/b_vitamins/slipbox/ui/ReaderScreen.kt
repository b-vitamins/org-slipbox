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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
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
import io.github.b_vitamins.slipbox.navigation.ReadingAnchor
import io.github.b_vitamins.slipbox.ui.content.DocumentAssetResolver
import io.github.b_vitamins.slipbox.ui.content.DocumentContentView
import io.github.b_vitamins.slipbox.ui.content.DocumentFocusRequest
import io.github.b_vitamins.slipbox.ui.content.DocumentIntent
import io.github.b_vitamins.slipbox.ui.content.DocumentPosition
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

internal const val READER_DOCUMENT_TAG = "reader-document"

private val NoReaderAssets = DocumentAssetResolver { _, _ -> null }

@Composable
internal fun ReaderScreen(
    phase: DocumentReaderPhase,
    settings: ReadingSettings,
    onBack: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
    linkPhase: ReaderLinkPhase = ReaderLinkPhase.Idle,
    previewPhase: ReaderPreviewPhase = ReaderPreviewPhase.Hidden,
    focusRequest: DocumentFocusRequest? = null,
    initialAnchor: ReadingAnchor = ReadingAnchor.Start,
    onIntent: (DocumentIntent) -> Unit = {},
    resolveAsset: DocumentAssetResolver = NoReaderAssets,
    onDismissPreview: () -> Unit = {},
    onOpenPreview: () -> Unit = {},
) {
    val document = (phase as? DocumentReaderPhase.Ready)?.document
    var appearanceVisible by rememberSaveable { mutableStateOf(false) }
    val appearanceControl = remember { FocusRequester() }
    val documentControl = remember { FocusRequester() }
    var deliveredFocus by remember { mutableStateOf<DocumentFocusRequest?>(null) }
    LaunchedEffect(focusRequest) {
        if (focusRequest == null) {
            deliveredFocus = null
            return@LaunchedEffect
        }
        // Let the reveal release native focus before addressing its DOM origin.
        withFrameNanos {}
        deliveredFocus = focusRequest
    }
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val readingWidth = minOf(maxWidth, SlipboxDimensions.readingMeasure)
        val excerptLines =
            ((previewPhase as? ReaderPreviewPhase.Ready)?.preview?.excerptLines ?: 1)
                .coerceIn(1, 12)
        val naturalPreviewHeight = (96 + excerptLines * 24).dp
        val previewHeight =
            minOf(
                maxHeight * 0.42f,
                300.dp,
                maxOf(160.dp, naturalPreviewHeight),
            )
        val previewVisible = previewPhase != ReaderPreviewPhase.Hidden
        val presentation =
            rememberDocumentPresentation(
                appearance = settings.preferences.appearance,
                reduceMotion = settings.preferences.reduceMotion,
                availableWidth = readingWidth,
            )
        ReadingSurface(
            title = document?.anchor?.title ?: stringResource(R.string.reader_title),
            obscured = appearanceVisible || previewVisible,
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
                ReaderPreviewSheet(
                    phase = previewPhase,
                    presentation = presentation,
                    motion = motion,
                    documentHeight = previewHeight,
                    onDismiss = onDismissPreview,
                    onOpen = onOpenPreview,
                    resolveAsset = resolveAsset,
                    restoreFocusTo = documentControl,
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
                    ReaderLinkNotice(linkPhase)
                    DocumentContentView(
                        source = phase.document.source,
                        presentation = presentation,
                        initialPosition =
                            DocumentPosition(
                                mark = initialAnchor.mark,
                                progress = initialAnchor.progress,
                                offset = initialAnchor.offset,
                            ),
                        restoreFocus = deliveredFocus,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .weight(1f)
                                .focusRequester(documentControl)
                                .testTag(READER_DOCUMENT_TAG),
                        onIntent = onIntent,
                        resolveAsset = resolveAsset,
                    )
                }
            }
        }
    }
}

@Composable
private fun ReaderLinkNotice(phase: ReaderLinkPhase) {
    val text =
        when (phase) {
            ReaderLinkPhase.Idle -> return
            ReaderLinkPhase.Resolving -> stringResource(R.string.reader_link_resolving)
            is ReaderLinkPhase.Missing -> stringResource(R.string.reader_link_missing, phase.target)
            is ReaderLinkPhase.Unsupported ->
                stringResource(R.string.reader_link_unsupported, phase.target)
            is ReaderLinkPhase.Failed -> stringResource(R.string.reader_link_failed, phase.target)
            ReaderLinkPhase.ExternalUnavailable ->
                stringResource(R.string.reader_link_browser_unavailable)
            is ReaderLinkPhase.AssetMissing ->
                stringResource(R.string.reader_asset_missing, phase.label)
            is ReaderLinkPhase.AssetUnsupported ->
                stringResource(R.string.reader_asset_unsupported, phase.label)
            is ReaderLinkPhase.AssetOversized ->
                stringResource(
                    R.string.reader_asset_too_large,
                    phase.label,
                    phase.maxBytes / (1024L * 1024L),
                )
            is ReaderLinkPhase.AssetViewerUnavailable ->
                stringResource(R.string.reader_asset_viewer_unavailable, phase.label)
            is ReaderLinkPhase.AssetFailed ->
                stringResource(R.string.reader_asset_failed, phase.label)
        }
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color =
            if (phase == ReaderLinkPhase.Resolving) {
                MaterialTheme.colorScheme.onSurfaceVariant
            } else {
                MaterialTheme.colorScheme.error
            },
        modifier =
            Modifier.fillMaxWidth().padding(
                horizontal = SlipboxDimensions.readingPadding,
                vertical = SlipboxDimensions.headerPaddingVertical,
            ),
    )
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
