/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxAppearanceTest {

    @Test
    fun theSystemChoiceFollowsThePlatform() {
        assertTrue(SlipboxAppearance.System.paintsDark(systemInDark = true))
        assertFalse(SlipboxAppearance.System.paintsDark(systemInDark = false))
    }

    @Test
    fun anExplicitChoiceOutranksThePlatform() {
        assertFalse(SlipboxAppearance.Light.paintsDark(systemInDark = true))
        assertTrue(SlipboxAppearance.Dark.paintsDark(systemInDark = false))
    }
}
