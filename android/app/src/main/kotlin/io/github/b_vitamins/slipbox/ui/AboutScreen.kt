/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import io.github.b_vitamins.slipbox.BuildConfig
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.version.SlipboxVersion

@Composable
internal fun AboutScreen(onBack: () -> Unit, modifier: Modifier = Modifier) {
    ReadingSurface(
        title = stringResource(R.string.about_title),
        modifier = modifier,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = onBack,
            )
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding)) {
            AboutEntry(
                label = stringResource(R.string.about_label_version),
                value = BuildConfig.VERSION_NAME,
            )
            AboutEntry(
                label = stringResource(R.string.about_label_build),
                value = BuildConfig.BUILD_TYPE,
            )
            AboutEntry(
                label = stringResource(R.string.about_label_identifier),
                value = BuildConfig.APPLICATION_ID,
            )
            AboutEntry(
                label = stringResource(R.string.about_label_license),
                value = stringResource(R.string.about_license),
            )
            if (SlipboxVersion.isDevelopmentCandidate(BuildConfig.VERSION_STAGE)) {
                Text(
                    text = stringResource(R.string.about_development_build),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun AboutEntry(label: String, value: String) {
    Column(modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
    }
}
