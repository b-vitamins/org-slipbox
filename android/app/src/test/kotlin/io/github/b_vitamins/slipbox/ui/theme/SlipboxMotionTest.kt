/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxMotionTest {

    @Test
    fun aNativeDurationIsTheCanonicalOne() {
        // The platform scales a Compose animation itself; scaling here would double it.
        assertEquals(200, SlipboxMotion(platformScale = 0.5f).native(200))
        assertEquals(200, SlipboxMotion(platformScale = 10f).native(200))
    }

    @Test
    fun aDocumentDurationCarriesThePlatformScaleOnce() {
        assertEquals(400, SlipboxMotion(platformScale = 2f).document(200))
        assertEquals(100, SlipboxMotion(platformScale = 0.5f).document(200))
        assertEquals(38, SlipboxMotion(platformScale = 0.5f).document(75))
    }

    @Test
    fun aReaderWhoAsksForLessMotionGetsNone() {
        val motion = SlipboxMotion(platformScale = 1f, reduceMotion = true)
        assertTrue(motion.reduced)
        assertEquals(0, motion.native(200))
        assertEquals(0, motion.document(200))
    }

    @Test
    fun aPlatformWithoutAnimationGetsNone() {
        val motion = SlipboxMotion(platformScale = 0f)
        assertTrue(motion.reduced)
        assertEquals(0, motion.native(200))
        assertEquals(0, motion.document(200))
    }

    @Test
    fun anUnscaledPlatformKeepsItsMotion() {
        val motion = SlipboxMotion()
        assertFalse(motion.reduced)
        assertEquals(150, motion.native(150))
        assertEquals(150, motion.document(150))
    }
}
