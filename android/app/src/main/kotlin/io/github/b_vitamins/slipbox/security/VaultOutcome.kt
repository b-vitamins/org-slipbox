/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** Absence is a completed null read, never a substitute for a failure. */
sealed interface VaultOutcome<out T> {

    /** The operation ran to completion and produced [value]. */
    data class Completed<out T>(val value: T) : VaultOutcome<T>

    /** The operation did not complete, for the reason [failure] gives. */
    data class Failed(val failure: VaultFailure) : VaultOutcome<Nothing>
}
