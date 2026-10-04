/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.engine.DirectedRelationRecord
import io.github.b_vitamins.slipbox.engine.ExplorationLens
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.NotePlaceNeighbor
import io.github.b_vitamins.slipbox.navigation.ReadingAnchor
import io.github.b_vitamins.slipbox.ui.content.DocumentAssetResolver
import io.github.b_vitamins.slipbox.ui.content.DocumentContentView
import io.github.b_vitamins.slipbox.ui.content.DocumentFocusRequest
import io.github.b_vitamins.slipbox.ui.content.DocumentHeadingRequest
import io.github.b_vitamins.slipbox.ui.content.DocumentIntent
import io.github.b_vitamins.slipbox.ui.content.DocumentPosition
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxSettle
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale
import kotlinx.coroutines.delay

internal const val READER_DOCUMENT_TAG = "reader-document"

private val NoReaderAssets = DocumentAssetResolver { _, _ -> null }

internal enum class ReaderSurfaceKind {
    Note,
    GlossaryTerm,
}

@Composable
internal fun ReaderScreen(
    phase: DocumentReaderPhase,
    settings: ReadingSettings,
    onBack: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
    kind: ReaderSurfaceKind = ReaderSurfaceKind.Note,
    linkPhase: ReaderLinkPhase = ReaderLinkPhase.Idle,
    previewPhase: ReaderPreviewPhase = ReaderPreviewPhase.Hidden,
    focusRequest: DocumentFocusRequest? = null,
    initialAnchor: ReadingAnchor = ReadingAnchor.Start,
    onIntent: (DocumentIntent) -> Unit = {},
    resolveAsset: DocumentAssetResolver = NoReaderAssets,
    onDismissPreview: () -> Unit = {},
    onDismissLinkNotice: () -> Unit = {},
    onOpenPreview: () -> Unit = {},
    bookmarked: Boolean = false,
    onToggleBookmark: () -> Unit = {},
    onOpenFilingNeighbor: (NotePlaceNeighbor) -> Unit = {},
    relationsPhase: DirectedRelationsPhase = DirectedRelationsPhase.Idle,
    onOpenRelations: () -> Unit = {},
    onRetryRelations: () -> Unit = {},
    onLoadMoreRelations: () -> Unit = {},
    onPreviewRelation: (DirectedRelationRecord) -> Unit = {},
    onOpenRelation: (DirectedRelationRecord) -> Unit = {},
    relatedPhase: RelatedDiscoveryPhase = RelatedDiscoveryPhase.Idle,
    mentionPhase: MentionDiscoveryPhase = MentionDiscoveryPhase.Idle,
    onRevealRelated: () -> Unit = {},
    onRefreshRelated: () -> Unit = {},
    onShowAllRelated: () -> Unit = {},
    onRevealMentions: () -> Unit = {},
    onRefreshMentions: () -> Unit = {},
    onShowAllMentions: () -> Unit = {},
    onPreviewDiscovered: (NodeRecord) -> Unit = {},
    onOpenDiscovered: (NodeRecord) -> Unit = {},
    explorationPhase: ReaderExplorationPhase = ReaderExplorationPhase.AwaitingLens,
    onSelectExplorationLens: (ExplorationLens) -> Unit = {},
    onRefreshExploration: () -> Unit = {},
    onPreviewExploration: (NodeRecord) -> Unit = {},
    onOpenExploration: (NodeRecord) -> Unit = {},
) {
    val document = (phase as? DocumentReaderPhase.Ready)?.document
    var appearanceVisible by rememberSaveable { mutableStateOf(false) }
    val appearanceControl = remember { FocusRequester() }
    val contextControl = remember { FocusRequester() }
    val documentControl = remember { FocusRequester() }
    var relationsVisible by rememberSaveable { mutableStateOf(false) }
    var explorationVisible by rememberSaveable { mutableStateOf(false) }
    var contextVisible by rememberSaveable { mutableStateOf(false) }
    var contextActionTaken by remember { mutableStateOf(false) }
    var headingSerial by remember(document?.source) { mutableLongStateOf(0L) }
    var headingRequest by remember(document?.source) {
        mutableStateOf<DocumentHeadingRequest?>(null)
    }
    var deliveredFocus by remember { mutableStateOf<DocumentFocusRequest?>(null) }
    var headerVisible by remember(document?.source) { mutableStateOf(true) }
    var previousProgress by
        remember(document?.source) { mutableFloatStateOf(initialAnchor.progress) }
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
        val surfaceObscured =
            appearanceVisible ||
                previewVisible ||
                contextVisible ||
                relationsVisible ||
                explorationVisible
        val presentation =
            rememberDocumentPresentation(
                appearance = settings.preferences.appearance,
                reduceMotion = settings.preferences.reduceMotion,
                availableWidth = readingWidth,
            )
        ReadingSurface(
            title =
                document?.anchor?.title
                    ?: stringResource(
                        if (kind == ReaderSurfaceKind.GlossaryTerm) {
                            R.string.glossary_term_title
                        } else {
                            R.string.reader_title
                        },
                    ),
            obscured = surfaceObscured,
            scrollable = document == null,
            titleVisible = document == null,
            dividerVisible = document == null,
            headerVisible = document == null || headerVisible,
            headerVisibilityMillis = motion.native(SlipboxTokens.Motion.CROSSFADE_MS),
            contentBackground = MaterialTheme.colorScheme.background,
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
                Row(
                    horizontalArrangement = Arrangement.spacedBy(0.dp),
                ) {
                    if (document != null && kind == ReaderSurfaceKind.Note) {
                        IconControl(
                            icon =
                                painterResource(
                                    if (bookmarked) {
                                        R.drawable.ic_bookmark
                                    } else {
                                        R.drawable.ic_bookmark_outline
                                    },
                                ),
                            label =
                                stringResource(
                                    if (bookmarked) {
                                        R.string.action_bookmarked
                                    } else {
                                        R.string.action_bookmark
                                    },
                                ),
                            onClick = onToggleBookmark,
                            tint =
                                if (bookmarked) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    null
                                },
                        )
                    }
                    if (document != null) {
                        IconControl(
                            icon = painterResource(R.drawable.ic_more),
                            label =
                                stringResource(
                                    if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                        R.string.glossary_term_info
                                    } else {
                                        R.string.reader_note_info
                                    },
                                ),
                            onClick = {
                                contextActionTaken = false
                                contextVisible = true
                            },
                            modifier = Modifier.focusRequester(contextControl),
                        )
                    }
                    if (document == null) {
                        TextControl(
                            label = stringResource(R.string.action_appearance),
                            onClick = { appearanceVisible = true },
                            modifier = Modifier.focusRequester(appearanceControl),
                        )
                    }
                }
            },
            overlay = {
                ReaderLinkNotice(
                    phase = linkPhase,
                    motion = motion,
                    onDismiss = onDismissLinkNotice,
                    modifier =
                        Modifier
                            .align(Alignment.TopCenter)
                            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top))
                            .padding(
                                top =
                                    if (document != null && !headerVisible) {
                                        8.dp
                                    } else {
                                        SlipboxDimensions.headerMinHeight + 8.dp
                                    },
                                start = SlipboxDimensions.readingPadding,
                                end = SlipboxDimensions.readingPadding,
                            )
                            .widthIn(max = SlipboxDimensions.readingMeasure)
                            .fillMaxWidth()
                            .zIndex(1f),
                )
                AppearanceSheet(
                    visible = appearanceVisible,
                    settings = settings,
                    motion = motion,
                    onDismiss = { appearanceVisible = false },
                    restoreFocusTo = if (document == null) appearanceControl else contextControl,
                )
                ReaderContextSheet(
                    document = document,
                    kind = kind,
                    visible = contextVisible,
                    motion = motion,
                    onDismiss = { contextVisible = false },
                    onHeading = { index ->
                        contextActionTaken = true
                        contextVisible = false
                        headingSerial += 1
                        headingRequest = DocumentHeadingRequest(index, headingSerial)
                    },
                    onNeighbor = { neighbor ->
                        contextActionTaken = true
                        contextVisible = false
                        onOpenFilingNeighbor(neighbor)
                    },
                    onExplore = {
                        contextActionTaken = true
                        contextVisible = false
                        explorationVisible = true
                    },
                    onRelations = {
                        contextActionTaken = true
                        contextVisible = false
                        relationsVisible = true
                        onOpenRelations()
                    },
                    onAppearance = {
                        contextActionTaken = true
                        contextVisible = false
                        appearanceVisible = true
                    },
                    restoreFocusTo = if (contextActionTaken) null else contextControl,
                )
                DirectedRelationsSheet(
                    noteKey = document?.anchor?.nodeKey,
                    visible = relationsVisible,
                    phase = relationsPhase,
                    motion = motion,
                    onDismiss = { relationsVisible = false },
                    onRetry = onRetryRelations,
                    onLoadMore = onLoadMoreRelations,
                    onPreview = { relation ->
                        relationsVisible = false
                        onPreviewRelation(relation)
                    },
                    onOpen = { relation ->
                        relationsVisible = false
                        onOpenRelation(relation)
                    },
                    relatedPhase = relatedPhase,
                    mentionPhase = mentionPhase,
                    onRevealRelated = onRevealRelated,
                    onRefreshRelated = onRefreshRelated,
                    onShowAllRelated = onShowAllRelated,
                    onRevealMentions = onRevealMentions,
                    onRefreshMentions = onRefreshMentions,
                    onShowAllMentions = onShowAllMentions,
                    onPreviewDiscovered = { note ->
                        relationsVisible = false
                        onPreviewDiscovered(note)
                    },
                    onOpenDiscovered = { note ->
                        relationsVisible = false
                        onOpenDiscovered(note)
                    },
                    restoreFocusTo = contextControl,
                )
                ReaderExplorationSheet(
                    visible = explorationVisible,
                    phase = explorationPhase,
                    motion = motion,
                    onDismiss = { explorationVisible = false },
                    onSelect = onSelectExplorationLens,
                    onRefresh = onRefreshExploration,
                    onPreview = { note ->
                        explorationVisible = false
                        onPreviewExploration(note)
                    },
                    onOpen = { note ->
                        explorationVisible = false
                        onOpenExploration(note)
                    },
                    restoreFocusTo = contextControl,
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
                    ReaderNotice(
                        stringResource(
                            if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                R.string.glossary_term_loading
                            } else {
                                R.string.reader_loading
                            },
                        ),
                    )

                DocumentReaderPhase.NotFound ->
                    ReaderNotice(
                        stringResource(
                            if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                R.string.glossary_term_not_found
                            } else {
                                R.string.reader_not_found
                            },
                        ),
                        problem = true,
                    )

                DocumentReaderPhase.SourceUnavailable ->
                    ReaderNotice(
                        stringResource(
                            if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                R.string.glossary_term_source_unavailable
                            } else {
                                R.string.reader_source_unavailable
                            },
                        ),
                        problem = true,
                    )

                is DocumentReaderPhase.UnsupportedSize ->
                    ReaderNotice(
                        text =
                            if (phase.maxLines == null) {
                                stringResource(
                                    if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                        R.string.glossary_term_too_large
                                    } else {
                                        R.string.reader_too_large
                                    },
                                )
                            } else {
                                pluralStringResource(
                                    if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                        R.plurals.glossary_term_too_many_lines
                                    } else {
                                        R.plurals.reader_too_many_lines
                                    },
                                    phase.maxLines,
                                    phase.maxLines,
                                )
                            },
                        problem = true,
                    )

                DocumentReaderPhase.Failed -> {
                    ReaderNotice(
                        stringResource(
                            if (kind == ReaderSurfaceKind.GlossaryTerm) {
                                R.string.glossary_term_unavailable
                            } else {
                                R.string.reader_unavailable
                            },
                        ),
                        problem = true,
                    )
                    TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                }

                is DocumentReaderPhase.Ready -> {
                    DocumentContentView(
                        title = phase.document.anchor.title,
                        source = phase.document.source,
                        presentation = presentation,
                        initialPosition =
                            DocumentPosition(
                                mark = initialAnchor.mark,
                                progress = initialAnchor.progress,
                                offset = initialAnchor.offset,
                            ),
                        initialHeadingIndex =
                            phase.document.outline
                                .indexOfFirst {
                                    it.nodeKey == phase.document.addressedAnchor.nodeKey
                                }
                                .takeIf { it >= 0 },
                        revealHeading = headingRequest,
                        restoreFocus = deliveredFocus,
                        accessible = !surfaceObscured,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .weight(1f)
                                .focusRequester(documentControl)
                                .testTag(READER_DOCUMENT_TAG),
                        onIntent = { intent ->
                            if (intent is DocumentIntent.Position) {
                                val progress = intent.position.progress
                                val delta = progress - previousProgress
                                headerVisible =
                                    when {
                                        progress <= READER_TOP_PROGRESS -> true
                                        delta >= READER_SCROLL_PROGRESS_DELTA -> false
                                        delta <= -READER_SCROLL_PROGRESS_DELTA -> true
                                        else -> headerVisible
                                    }
                                previousProgress = progress
                            }
                            onIntent(intent)
                        },
                        resolveAsset = resolveAsset,
                    )
                }
            }
        }
    }
}

@Composable
private fun ReaderLinkNotice(
    phase: ReaderLinkPhase,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val text =
        when (phase) {
            ReaderLinkPhase.Idle -> return
            ReaderLinkPhase.Resolving -> return
            is ReaderLinkPhase.Missing -> stringResource(R.string.reader_link_missing)
            is ReaderLinkPhase.Unsupported -> stringResource(R.string.reader_link_unsupported)
            is ReaderLinkPhase.Failed -> stringResource(R.string.reader_link_failed)
            ReaderLinkPhase.ExternalUnavailable ->
                stringResource(R.string.reader_link_browser_unavailable)
            is ReaderLinkPhase.AssetMissing ->
                stringResource(R.string.reader_asset_missing)
            is ReaderLinkPhase.AssetUnsupported ->
                stringResource(R.string.reader_asset_unsupported)
            is ReaderLinkPhase.AssetOversized ->
                stringResource(R.string.reader_asset_too_large)
            is ReaderLinkPhase.AssetViewerUnavailable ->
                stringResource(R.string.reader_asset_viewer_unavailable)
            is ReaderLinkPhase.AssetFailed ->
                stringResource(R.string.reader_asset_failed)
        }
    var visible by remember(phase) { mutableStateOf(true) }
    LaunchedEffect(phase) {
        delay(LINK_NOTICE_MILLIS)
        visible = false
        onDismiss()
    }
    val opacityMs = motion.native(SlipboxTokens.Motion.OPACITY_MS)
    AnimatedVisibility(
        visible = visible,
        modifier = modifier,
        enter =
            if (opacityMs == 0) {
                EnterTransition.None
            } else {
                fadeIn(tween(opacityMs, easing = SlipboxSettle))
            },
        exit =
            if (opacityMs == 0) {
                ExitTransition.None
            } else {
                fadeOut(tween(opacityMs, easing = SlipboxSettle))
            },
    ) {
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = MaterialTheme.colorScheme.surface,
            contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
            border = BorderStroke(SlipboxDimensions.hairline, MaterialTheme.colorScheme.outlineVariant),
            shadowElevation = 3.dp,
            modifier = Modifier.fillMaxWidth().semantics { liveRegion = LiveRegionMode.Polite },
        ) {
            Text(
                text = text,
                style = MaterialTheme.typography.bodyMedium,
                modifier =
                    Modifier.padding(
                        horizontal = SlipboxDimensions.readingPadding,
                        vertical = 10.dp,
                    ),
            )
        }
    }
}

private const val LINK_NOTICE_MILLIS = 3_200L

private const val READER_TOP_PROGRESS = 0.002f

private const val READER_SCROLL_PROGRESS_DELTA = 0.0015f

@Composable
private fun ReaderNotice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
