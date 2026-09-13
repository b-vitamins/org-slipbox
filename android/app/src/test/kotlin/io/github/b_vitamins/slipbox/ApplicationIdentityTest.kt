/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox

import org.junit.Assert.assertEquals
import org.junit.Test

class ApplicationIdentityTest {

    @Test
    fun theReleaseIdentityIsTheOneThisProjectOwns() {
        assertEquals("io.github.b_vitamins.slipbox", BuildConfig.APPLICATION_ID_BASE)
    }

    @Test
    fun theDebugIdentityIsTheReleaseIdentityPlusDebug() {
        assertEquals(".debug", BuildConfig.DEBUG_APPLICATION_ID_SUFFIX)
        assertEquals(
            "io.github.b_vitamins.slipbox.debug",
            BuildConfig.APPLICATION_ID_BASE + BuildConfig.DEBUG_APPLICATION_ID_SUFFIX,
        )
    }

    @Test
    fun theRunningBuildCarriesTheIdentityItsTypePins() {
        val expected =
            if (BuildConfig.DEBUG) {
                "io.github.b_vitamins.slipbox.debug"
            } else {
                "io.github.b_vitamins.slipbox"
            }
        assertEquals(expected, BuildConfig.APPLICATION_ID)
    }
}
