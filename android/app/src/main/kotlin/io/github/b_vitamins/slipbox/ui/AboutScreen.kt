/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import io.github.b_vitamins.slipbox.BuildConfig
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferenceFault
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.settings.rememberReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale
import io.github.b_vitamins.slipbox.version.SlipboxVersion

@Composable
internal fun AboutScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    settings: ReadingSettings = rememberReadingSettings(),
) {
    // Resolve here: NavDisplay retains entry content across presentation changes.
    val motion = SlipboxMotion(rememberPlatformMotionScale(), settings.preferences.reduceMotion)
    var revealed by rememberSaveable { mutableStateOf(false) }
    val opened = remember { FocusRequester() }
    ReadingSurface(
        title = stringResource(R.string.about_title),
        modifier = modifier,
        obscured = revealed,
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
                onClick = { revealed = true },
                modifier = Modifier.focusRequester(opened),
            )
        },
        overlay = {
            AppearanceSheet(
                visible = revealed,
                settings = settings,
                motion = motion,
                onDismiss = { revealed = false },
                restoreFocusTo = opened,
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
internal fun AppearanceSheet(
    visible: Boolean,
    settings: ReadingSettings,
    motion: SlipboxMotion,
    onDismiss: () -> Unit,
    restoreFocusTo: FocusRequester? = null,
) {
    ContextualSheet(
        title = stringResource(R.string.appearance_title),
        visible = visible,
        motion = motion,
        onDismiss = onDismiss,
        restoreFocusTo = restoreFocusTo,
    ) {
        for (appearance in SlipboxAppearance.entries) {
            ChoiceRow(
                label = stringResource(appearance.label()),
                selected = settings.preferences.appearance == appearance,
                onSelect = { settings.select(appearance) },
            )
        }
        ToggleRow(
            label = stringResource(R.string.appearance_reduce_motion),
            marked = settings.preferences.reduceMotion,
            onToggle = { reduce -> settings.selectReduceMotion(reduce) },
        )
        val fault = settings.fault
        if (fault != null) {
            Text(
                text = stringResource(fault.notice()),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.error,
            )
        }
    }
}

@StringRes
private fun SlipboxAppearance.label(): Int =
    when (this) {
        SlipboxAppearance.System -> R.string.appearance_system
        SlipboxAppearance.Light -> R.string.appearance_light
        SlipboxAppearance.Dark -> R.string.appearance_dark
    }

@StringRes
private fun ReadingPreferenceFault.notice(): Int =
    when (this) {
        ReadingPreferenceFault.Unwritable -> R.string.appearance_unwritable
        ReadingPreferenceFault.Unreadable,
        ReadingPreferenceFault.Oversized,
        ReadingPreferenceFault.Malformed,
        ReadingPreferenceFault.UnsupportedVersion,
        -> R.string.appearance_unreadable
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
