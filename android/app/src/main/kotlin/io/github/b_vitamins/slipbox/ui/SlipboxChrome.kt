/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.hideFromAccessibility
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxSettle
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens

private val LocalSurfaceObscured = compositionLocalOf { false }

/**
 * [obscured] hides the surface from accessibility and focus and disables its controls.
 */
@Composable
internal fun ReadingSurface(
    title: String,
    modifier: Modifier = Modifier,
    obscured: Boolean = false,
    leading: @Composable () -> Unit = {},
    trailing: @Composable () -> Unit = {},
    overlay: @Composable BoxScope.() -> Unit = {},
    body: @Composable ColumnScope.() -> Unit,
) {
    Box(modifier = modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        CompositionLocalProvider(LocalSurfaceObscured provides obscured) {
            Column(modifier = Modifier.fillMaxSize().safeDrawingPadding().withheld(obscured)) {
                ReadingHeader(title = title, leading = leading, trailing = trailing)
                HorizontalDivider(
                    thickness = SlipboxDimensions.hairline,
                    color = MaterialTheme.colorScheme.outline,
                )
                ReadingColumn(modifier = Modifier.weight(1f), body = body)
            }
        }
        overlay()
    }
}

@Composable
internal fun ReadingColumn(
    modifier: Modifier = Modifier,
    body: @Composable ColumnScope.() -> Unit,
) {
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
        Column(
            modifier =
                Modifier
                    // Cap before fillMaxWidth raises the minimum constraint.
                    .widthIn(max = SlipboxDimensions.readingMeasure)
                    .fillMaxWidth()
                    // Keep the canvas full-height even for a short note.
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.surface)
                    .verticalScroll(rememberScrollState())
                    .padding(SlipboxDimensions.readingPadding),
            content = body,
        )
    }
}

@Composable
internal fun TextControl(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    HeaderControl(onClick = onClick, modifier = modifier) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            maxLines = 1,
        )
    }
}

@Composable
internal fun IconControl(
    icon: Painter,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    HeaderControl(
        onClick = onClick,
        modifier = modifier.semantics { contentDescription = label },
    ) {
        Icon(
            painter = icon,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
            modifier = Modifier.size(SlipboxDimensions.glyph),
        )
    }
}

@Composable
internal fun ChoiceRow(
    label: String,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
) {
    MarkedRow(
        label = label,
        marked = selected,
        modifier =
            modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onSelect),
    )
}

@Composable
internal fun ToggleRow(
    label: String,
    marked: Boolean,
    onToggle: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    MarkedRow(
        label = label,
        marked = marked,
        modifier =
            modifier.toggleable(value = marked, role = Role.Switch, onValueChange = onToggle),
    )
}

@Composable
private fun MarkedRow(label: String, marked: Boolean, modifier: Modifier = Modifier) {
    Row(
        modifier =
            modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.touchTarget)
                .padding(vertical = SlipboxDimensions.headerPaddingVertical),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.weight(1f),
        )
        // Reserved whether or not it is marked, so a change never reflows the labels.
        Box(modifier = Modifier.size(SlipboxDimensions.glyph)) {
            if (marked) {
                Icon(
                    painter = painterResource(R.drawable.ic_check),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}

/**
 * A modal reveal dismissed by Back or the scrim. Focus stays inside while visible;
 * dismissal restores [restoreFocusTo] only if the sheet held focus.
 */
@Composable
internal fun ContextualSheet(
    title: String,
    visible: Boolean,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    restoreFocusTo: FocusRequester? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    BackHandler(enabled = visible, onBack = onDismiss)
    val colors = SlipboxTheme.colors
    val dismissLabel = stringResource(R.string.action_dismiss)
    val opacityMs = motion.native(SlipboxTokens.Motion.OPACITY_MS)
    val columnMs = motion.native(SlipboxTokens.Motion.COLUMN_MS)
    var held by remember { mutableStateOf(false) }
    LaunchedEffect(visible) {
        if (!visible && held) {
            held = false
            // Restore after row removal clears focus for this frame.
            withFrameNanos {}
            restoreFocusTo?.requestFocus()
        }
    }
    Box(modifier = modifier.fillMaxSize()) {
        AnimatedVisibility(
            visible = visible,
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
            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .background(colors.scrim)
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClickLabel = dismissLabel,
                            role = Role.Button,
                            onClick = onDismiss,
                        ),
            )
        }
        AnimatedVisibility(
            visible = visible,
            modifier = Modifier.align(Alignment.BottomCenter),
            enter =
                if (columnMs == 0) {
                    EnterTransition.None
                } else {
                    slideInVertically(
                        animationSpec = tween(columnMs, easing = SlipboxSettle),
                        initialOffsetY = { height -> height },
                    )
                },
            exit =
                if (columnMs == 0) {
                    ExitTransition.None
                } else {
                    slideOutVertically(
                        animationSpec = tween(columnMs, easing = SlipboxSettle),
                        targetOffsetY = { height -> height },
                    )
                },
        ) {
            Column(
                modifier =
                    Modifier
                        .widthIn(max = SlipboxDimensions.readingMeasure)
                        .fillMaxWidth()
                        .background(colors.surface)
                        .onFocusChanged { state -> if (state.hasFocus) held = true }
                        // Permit focus restoration during the exit transition.
                        .focusProperties { if (visible) onExit = { cancelFocusChange() } }
                        .focusGroup()
                        .semantics {
                            paneTitle = title
                            isTraversalGroup = true
                        },
            ) {
                HorizontalDivider(
                    thickness = SlipboxDimensions.hairline,
                    color = MaterialTheme.colorScheme.outline,
                )
                Column(
                    modifier =
                        Modifier
                            .verticalScroll(rememberScrollState())
                            .windowInsetsPadding(
                                WindowInsets.safeDrawing.only(
                                    WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom,
                                ),
                            )
                            .padding(SlipboxDimensions.readingPadding),
                    content = content,
                )
            }
        }
    }
}

/**
 * The focus group blocks entry to the whole subtree, not just an individual target.
 */
private fun Modifier.withheld(obscured: Boolean): Modifier =
    if (!obscured) {
        this
    } else {
        this.semantics { hideFromAccessibility() }
            .focusProperties { onEnter = { cancelFocusChange() } }
            .focusGroup()
    }

@Composable
private fun HeaderControl(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val action =
        if (LocalSurfaceObscured.current) {
            Modifier.semantics(mergeDescendants = true) { disabled() }
        } else {
            Modifier.clickable(role = Role.Button, onClick = onClick)
        }
    Box(
        modifier =
            modifier
                .defaultMinSize(
                    minWidth = SlipboxDimensions.touchTarget,
                    minHeight = SlipboxDimensions.touchTarget,
                )
                .then(action),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
private fun ReadingHeader(
    title: String,
    leading: @Composable () -> Unit,
    trailing: @Composable () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = SlipboxDimensions.headerMinHeight)
                .padding(
                    horizontal = SlipboxDimensions.headerPaddingHorizontal,
                    vertical = SlipboxDimensions.headerPaddingVertical,
                ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading()
        Text(
            text = title,
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        trailing()
    }
}
