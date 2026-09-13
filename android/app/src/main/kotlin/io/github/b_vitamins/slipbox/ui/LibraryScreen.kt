/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import io.github.b_vitamins.slipbox.R

@Composable
internal fun LibraryScreen(onOpenAbout: () -> Unit, modifier: Modifier = Modifier) {
    ReadingSurface(
        title = stringResource(R.string.app_name),
        modifier = modifier,
        trailing = {
            TextControl(label = stringResource(R.string.action_about), onClick = onOpenAbout)
        },
    ) {
        Text(
            text = stringResource(R.string.library_empty),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
