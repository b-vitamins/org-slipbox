/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.version

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxVersionTest {

    @Test
    fun candidateNameStatesItsStageAndCandidate() {
        assertEquals("0.19.0-dev.1", SlipboxVersion.versionName(0, 19, 0, 1, "dev"))
    }

    @Test
    fun releaseNameStatesNothingBesidesTheVersion() {
        assertEquals("1.2.3", SlipboxVersion.versionName(1, 2, 3, 7, "release"))
    }

    @Test
    fun onlyTheReleaseStageIsARelease() {
        assertFalse(SlipboxVersion.isDevelopmentCandidate(SlipboxVersion.RELEASE_STAGE))
        assertTrue(SlipboxVersion.isDevelopmentCandidate("dev"))
        assertTrue(SlipboxVersion.isDevelopmentCandidate("beta"))
    }

    @Test
    fun codeRisesWithEveryComponent() {
        assertTrue(
            SlipboxVersion.versionCode(0, 19, 0, 2) > SlipboxVersion.versionCode(0, 19, 0, 1),
        )
        assertTrue(
            SlipboxVersion.versionCode(0, 19, 1, 0) > SlipboxVersion.versionCode(0, 19, 0, 999),
        )
        assertTrue(
            SlipboxVersion.versionCode(0, 20, 0, 0) > SlipboxVersion.versionCode(0, 19, 99, 999),
        )
        assertTrue(
            SlipboxVersion.versionCode(1, 0, 0, 0) > SlipboxVersion.versionCode(0, 99, 99, 999),
        )
    }

    @Test
    fun theStageChangesTheNameAndNotTheCode() {
        assertNotEquals(
            SlipboxVersion.versionName(0, 19, 0, 7, "dev"),
            SlipboxVersion.versionName(0, 19, 0, 7, "release"),
        )
        assertEquals(1_900_007, SlipboxVersion.versionCode(0, 19, 0, 7))
    }

    @Test
    fun resettingTheCandidateAtAFinalReleaseWouldLowerTheCode() {
        val lastCandidate = SlipboxVersion.versionCode(0, 19, 0, 7)
        assertTrue(SlipboxVersion.versionCode(0, 19, 0, 0) < lastCandidate)
        assertTrue(SlipboxVersion.versionCode(0, 19, 1, 0) > lastCandidate)
        assertTrue(SlipboxVersion.versionCode(0, 19, 0, 8) > lastCandidate)
    }

    @Test
    fun theFinalReleaseKeepsItsVersionAndRaisesTheCandidate() {
        val candidateCode = SlipboxVersion.versionCode(0, 19, 0, 7)
        val releaseCode = SlipboxVersion.versionCode(0, 19, 0, 8)

        assertEquals("0.19.0", SlipboxVersion.versionName(0, 19, 0, 8, "release"))
        assertTrue(releaseCode > candidateCode)
    }

    @Test
    fun codeIsTheSameForTheSameComponents() {
        assertEquals(
            SlipboxVersion.versionCode(0, 19, 0, 1),
            SlipboxVersion.versionCode(0, 19, 0, 1),
        )
        assertEquals(1_900_001, SlipboxVersion.versionCode(0, 19, 0, 1))
    }

    @Test
    fun theLargestCodeThisSchemeCanDeriveIsStillInstallable() {
        assertTrue(SlipboxVersion.versionCode(99, 99, 99, 999) <= 2_100_000_000)
    }

    @Test(expected = IllegalArgumentException::class)
    fun aComponentPastItsWindowIsRefused() {
        SlipboxVersion.versionCode(0, 100, 0, 0)
    }

    @Test(expected = IllegalArgumentException::class)
    fun aCandidatePastItsWindowIsRefused() {
        SlipboxVersion.versionCode(0, 19, 0, 1000)
    }

    @Test(expected = IllegalArgumentException::class)
    fun aNegativeComponentIsRefused() {
        SlipboxVersion.versionCode(0, -1, 0, 0)
    }

    @Test(expected = IllegalArgumentException::class)
    fun aStageThatIsNotOneWordIsRefused() {
        SlipboxVersion.versionName(0, 19, 0, 1, "dev-1")
    }

    @Test(expected = IllegalArgumentException::class)
    fun anEmptyStageIsRefused() {
        SlipboxVersion.isDevelopmentCandidate("")
    }
}
