/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import io.github.b_vitamins.slipbox.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class NativeLibraryPackagingTest {

    @Test
    fun theQualifiedAbisAreTheOnesThisReleasePackages() {
        assertEquals(listOf("arm64-v8a"), BuildConfig.QUALIFIED_ABIS.split(","))
    }

    @Test
    fun noAbiIsPackagedBeforeItIsQualified() {
        val qualified = BuildConfig.QUALIFIED_ABIS.split(",")
        for (abi in listOf("armeabi-v7a", "x86", "x86_64", "riscv64")) {
            assertTrue("$abi is packaged without a qualifying run", abi !in qualified)
        }
    }

    @Test
    fun theLibraryEntryNamesTheCargoArtifactOfTheAbi() {
        assertEquals("slipbox_android", SlipboxNativeEngine.LIBRARY_NAME)
        assertEquals(
            "lib/arm64-v8a/libslipbox_android.so",
            SlipboxNativeEngine.libraryEntry("arm64-v8a"),
        )
    }

    @Test
    fun aHostWithoutThePackagedLibraryReportsTheFailureInsteadOfProbing() {
        assertNotNull(SlipboxNativeEngine.loadFailure)

        val refusal =
            assertThrows(IllegalStateException::class.java) {
                SlipboxNativeEngine.runFixtureProbe(File("."))
            }
        assertTrue(
            refusal.message.orEmpty(),
            refusal.message.orEmpty().contains("did not load"),
        )
    }
}
