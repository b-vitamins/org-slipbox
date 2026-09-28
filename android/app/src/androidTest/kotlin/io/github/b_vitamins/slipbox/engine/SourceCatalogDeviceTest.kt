/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.git.SlipboxNativeGit
import io.github.b_vitamins.slipbox.sources.SourceCatalogGateway
import io.github.b_vitamins.slipbox.sources.SourceCatalogResult
import java.util.concurrent.FutureTask
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SourceCatalogDeviceTest {

    @Test
    fun packagedCatalogBoundaryLoadsPrivateEmptyStateOffMain() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val task = FutureTask { SourceCatalogGateway(context).load() }
        Thread(task, "slipbox-catalog-device-test").start()

        assertNull(SlipboxNativeGit.loadFailure)
        assertEquals(SourceCatalogResult.Empty(0), task.get(10, TimeUnit.SECONDS))
    }
}
