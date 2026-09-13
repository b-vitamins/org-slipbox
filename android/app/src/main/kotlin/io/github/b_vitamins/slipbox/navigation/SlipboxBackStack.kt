/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.snapshots.SnapshotStateList

@Composable
fun rememberSlipboxBackStack(): SnapshotStateList<SlipboxDestination> =
    rememberSaveable(saver = slipboxBackStackSaver()) {
        mutableStateListOf(SlipboxDestination.start)
    }

internal fun slipboxBackStackSaver(): Saver<SnapshotStateList<SlipboxDestination>, Any> =
    listSaver(
        save = { stack -> saveBackStack(stack) },
        restore = { saved ->
            mutableStateListOf<SlipboxDestination>().apply {
                addAll(restoreBackStack(saved.filterIsInstance<String>()))
            }
        },
    )

internal fun saveBackStack(stack: List<SlipboxDestination>): List<String> =
    stack.map(SlipboxDestination::id)

/** Drop unknown IDs, duplicate roots and adjacent repeats while preserving destination order. */
internal fun restoreBackStack(ids: List<String>): List<SlipboxDestination> {
    val restored = mutableListOf(SlipboxDestination.start)
    for (destination in ids.mapNotNull(SlipboxDestination::fromId)) {
        if (destination != SlipboxDestination.start) restored.pushDestination(destination)
    }
    return restored
}

internal fun MutableList<SlipboxDestination>.pushDestination(
    destination: SlipboxDestination,
): Boolean {
    if (lastOrNull() == destination) return false
    add(destination)
    return true
}

/** The final root entry is never popped. */
internal fun MutableList<SlipboxDestination>.popDestination(): Boolean {
    if (size <= 1) return false
    removeAt(lastIndex)
    return true
}
