/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class StartupBenchmark {

    @get:Rule val benchmark = MacrobenchmarkRule()

    private val journey = BenchmarkJourney()

    @Test fun coldSmall() = startup(SMALL_CORPUS, StartupMode.COLD, 8)

    @Test fun coldMedium() = startup(MEDIUM_CORPUS, StartupMode.COLD, 8)

    @Test fun coldLarge() = startup(LARGE_CORPUS, StartupMode.COLD, 5)

    @Test fun warmMedium() = startup(MEDIUM_CORPUS, StartupMode.WARM, 8)

    private fun startup(nodes: Int, mode: StartupMode, iterations: Int) {
        journey.stabilizeDisplay()
        try {
            journey.prepareFromInstrumentation(nodes)
            benchmark.measureRepeated(
                packageName = TARGET_PACKAGE,
                metrics = listOf(StartupTimingMetric()),
                compilationMode = CompilationMode.Partial(BaselineProfileMode.UseIfAvailable),
                startupMode = mode,
                iterations = iterations,
                setupBlock = { pressHome() },
                measureBlock = { journey.launch(this) },
            )
        } finally {
            journey.releaseDisplay()
        }
    }
}
