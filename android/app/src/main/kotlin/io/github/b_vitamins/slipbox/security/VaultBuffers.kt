/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** Plaintext-buffer allocation; callers clear acquired arrays in finally. */
internal fun interface VaultBuffers {

    /** A zeroed array of [length] bytes for its taker to fill and then clear. */
    fun take(length: Int): ByteArray

    companion object {

        val Direct: VaultBuffers = VaultBuffers { length -> ByteArray(length) }
    }
}
