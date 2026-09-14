/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.settings

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File

/** Unreadable preferences use defaults and expose a fault without overwriting the record. */
@Stable
internal class ReadingSettings(private val store: ReadingPreferencesStore) {

    var preferences: ReadingPreferences by mutableStateOf(ReadingPreferences())
        private set

    var fault: ReadingPreferenceFault? by mutableStateOf(null)
        private set

    /** Whether the current preferences match the stored record. */
    var stored: Boolean by mutableStateOf(false)
        private set

    init {
        when (val record = store.read()) {
            ReadingPreferencesRecord.Absent -> Unit
            is ReadingPreferencesRecord.Stored -> {
                preferences = record.preferences
                stored = true
            }
            is ReadingPreferencesRecord.Unusable -> fault = record.fault
        }
    }

    fun select(appearance: SlipboxAppearance) {
        apply(preferences.copy(appearance = appearance))
    }

    fun selectReduceMotion(reduceMotion: Boolean) {
        apply(preferences.copy(reduceMotion = reduceMotion))
    }

    /** Apply the choice even if persistence fails; retain the fault for display. */
    private fun apply(next: ReadingPreferences) {
        if (next == preferences && stored) return
        preferences = next
        val failed = store.write(next)
        fault = failed
        stored = failed == null
    }
}

internal fun readingPreferencesFile(context: Context): ReadingPreferencesStore =
    ReadingPreferencesFile(File(context.noBackupFilesDir, ReadingPreferencesFile.FILE_NAME))

/** Remember directory creation and the initial disk read for this composition's context. */
@Composable
internal fun rememberReadingSettings(): ReadingSettings {
    val context = LocalContext.current
    return remember(context) { ReadingSettings(readingPreferencesFile(context)) }
}
