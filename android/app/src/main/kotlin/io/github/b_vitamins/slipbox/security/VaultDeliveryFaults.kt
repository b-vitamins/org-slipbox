/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** Receives callback fault types without messages, causes or suppressed errors. */
fun interface VaultDeliveryFaults {

    /** Reports that a delivery threw a fault of type [origin]. */
    fun onDeliveryFault(origin: String)

    companion object {

        /** Discards delivery faults, for a caller that does not observe them. */
        val Ignored: VaultDeliveryFaults =
            object : VaultDeliveryFaults {
                override fun onDeliveryFault(origin: String) = Unit
            }
    }
}

/** Contains secondary observer faults without recursive reporting. */
internal fun VaultDeliveryFaults.report(origin: String) {
    try {
        onDeliveryFault(origin)
    } catch (fault: Throwable) {
        // There is nowhere left to report to: this is the observer of reports.
    }
}
