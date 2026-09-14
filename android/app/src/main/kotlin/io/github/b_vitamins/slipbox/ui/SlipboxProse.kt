/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.sp
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens

@Composable
internal fun ProseText(text: AnnotatedString, modifier: Modifier = Modifier) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = SlipboxTheme.colors.ink,
        modifier = modifier.fillMaxWidth(),
    )
}

/**
 * Code uses the renderer's `--code-surface` fallback (paper). Long lines scroll locally.
 */
@Composable
internal fun MonoBlock(text: String, modifier: Modifier = Modifier) {
    val colors = SlipboxTheme.colors
    val corner = RoundedCornerShape(SlipboxDimensions.blockCorner)
    Box(
        modifier =
            modifier
                .fillMaxWidth()
                .clip(corner)
                .background(colors.paper)
                .border(SlipboxDimensions.hairline, colors.hairline, corner)
                .horizontalScroll(rememberScrollState())
                .padding(SlipboxDimensions.readingPadding),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = colors.ink,
            softWrap = false,
        )
    }
}

/** Displays math source without typesetting, with local horizontal scrolling. */
@Composable
internal fun MathBlock(source: String, modifier: Modifier = Modifier) {
    Box(modifier = modifier.fillMaxWidth().horizontalScroll(rememberScrollState())) {
        Text(
            text = source,
            style = MaterialTheme.typography.bodySmall,
            color = SlipboxTheme.colors.muted,
            softWrap = false,
        )
    }
}

@Composable
@ReadOnlyComposable
internal fun inlineCodeSpan(): SpanStyle =
    SpanStyle(
        fontFamily = FontFamily.Monospace,
        fontSize = SlipboxTokens.Type.CODE_SIZE_SP.sp,
        color = SlipboxTheme.colors.ink,
        background = SlipboxTheme.colors.paper,
    )

@Composable
@ReadOnlyComposable
internal fun proseLinkStyles(visited: Boolean = false): TextLinkStyles {
    val colors = SlipboxTheme.colors
    val tone = if (visited) colors.linkVisited else colors.link
    return TextLinkStyles(
        style = SpanStyle(color = tone, textDecoration = TextDecoration.Underline),
        pressedStyle =
            SpanStyle(
                color = tone,
                background = colors.surface,
                textDecoration = TextDecoration.Underline,
            ),
    )
}
