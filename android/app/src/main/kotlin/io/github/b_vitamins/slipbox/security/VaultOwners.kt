/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** Bounded record-path leases, released only after each owning worker terminates. */
internal object VaultOwners {

    const val LIMIT: Int = 32

    private val paths = HashSet<String>()

    val held: Int
        get() = synchronized(paths) { paths.size }

    fun claim(path: String): VaultFailure? =
        synchronized(paths) {
            when {
                path in paths -> VaultFailure.VaultHeld
                paths.size >= LIMIT -> VaultFailure.VaultAtCapacity(VaultBound.OpenRecords)
                else -> {
                    paths.add(path)
                    null
                }
            }
        }

    fun release(path: String) {
        synchronized(paths) { paths.remove(path) }
    }
}
