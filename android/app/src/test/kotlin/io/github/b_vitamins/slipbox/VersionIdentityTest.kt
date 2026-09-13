/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox

import io.github.b_vitamins.slipbox.version.SlipboxVersion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VersionIdentityTest {

    @Test
    fun theManifestNameIsTheDerivedName() {
        assertEquals(
            SlipboxVersion.versionName(
                major = BuildConfig.VERSION_MAJOR,
                minor = BuildConfig.VERSION_MINOR,
                patch = BuildConfig.VERSION_PATCH,
                candidate = BuildConfig.VERSION_CANDIDATE,
                stage = BuildConfig.VERSION_STAGE,
            ),
            BuildConfig.VERSION_NAME,
        )
    }

    @Test
    fun theManifestCodeIsTheDerivedCode() {
        assertEquals(
            SlipboxVersion.versionCode(
                major = BuildConfig.VERSION_MAJOR,
                minor = BuildConfig.VERSION_MINOR,
                patch = BuildConfig.VERSION_PATCH,
                candidate = BuildConfig.VERSION_CANDIDATE,
            ),
            BuildConfig.VERSION_CODE,
        )
    }

    @Test
    fun theNameSaysWhichOfTheTwoThisBuildIs() {
        val releaseForm = Regex("""\d+\.\d+\.\d+""")
        if (SlipboxVersion.isDevelopmentCandidate(BuildConfig.VERSION_STAGE)) {
            assertFalse(BuildConfig.VERSION_NAME.matches(releaseForm))
            assertTrue(BuildConfig.VERSION_NAME.contains(BuildConfig.VERSION_STAGE))
        } else {
            assertTrue(BuildConfig.VERSION_NAME.matches(releaseForm))
        }
    }

    @Test
    fun theVersionIsThisSurfacesOwn() {
        assertTrue(BuildConfig.VERSION_CODE > 0)
        assertNotEquals("0.18.0", BuildConfig.VERSION_NAME)
    }
}
