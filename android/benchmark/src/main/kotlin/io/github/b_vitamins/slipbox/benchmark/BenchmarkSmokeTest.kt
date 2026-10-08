/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Cheap device qualification; full measurement stays an explicit benchmark run. */
@RunWith(AndroidJUnit4::class)
class BenchmarkSmokeTest {

    @get:Rule val benchmark = MacrobenchmarkRule()

    private val journey = BenchmarkJourney()

    @Test
    fun searchReadAndScroll() {
        journey.stabilizeDisplay()
        try {
            journey.prepareFromInstrumentation(SMALL_CORPUS)
            benchmark.measureRepeated(
                packageName = TARGET_PACKAGE,
                metrics = listOf(FrameTimingMetric()),
                compilationMode = CompilationMode.None(),
                iterations = 1,
                setupBlock = {
                    killProcess()
                    journey.launch(this)
                },
                measureBlock = {
                    journey.searchAndOpen()
                    journey.scrollDocument(2)
                },
            )
        } finally {
            journey.releaseDisplay()
        }
    }
}
