/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

@Composable
internal fun ReadingSurface(
    title: String,
    modifier: Modifier = Modifier,
    leading: @Composable () -> Unit = {},
    trailing: @Composable () -> Unit = {},
    body: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier =
            modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .safeDrawingPadding(),
    ) {
        ReadingHeader(title = title, leading = leading, trailing = trailing)
        HorizontalDivider(
            thickness = SlipboxDimensions.hairline,
            color = MaterialTheme.colorScheme.outline,
        )
        ReadingColumn(modifier = Modifier.weight(1f), body = body)
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
        Icon(painter = icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
    }
}

@Composable
private fun HeaderControl(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Box(
        modifier =
            modifier
                .defaultMinSize(
                    minWidth = SlipboxDimensions.touchTarget,
                    minHeight = SlipboxDimensions.touchTarget,
                )
                .clickable(role = Role.Button, onClick = onClick),
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
