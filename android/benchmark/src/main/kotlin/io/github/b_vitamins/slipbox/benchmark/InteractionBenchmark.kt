/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(androidx.benchmark.macro.ExperimentalMetricApi::class)

package io.github.b_vitamins.slipbox.benchmark

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.MemoryUsageMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class InteractionBenchmark {

    @get:Rule val benchmark = MacrobenchmarkRule()

    private val journey = BenchmarkJourney()

    @Test fun searchSmall() = search(SMALL_CORPUS)

    @Test fun searchMedium() = search(MEDIUM_CORPUS)

    @Test fun searchLarge() = search(LARGE_CORPUS)

    @Test
    fun richDocumentScroll() =
        interaction(
            nodes = MEDIUM_CORPUS,
            setup = { journey.searchAndOpen() },
            measure = { journey.scrollDocument(12) },
        )

    @Test
    fun linkedReading() =
        interaction(
            nodes = MEDIUM_CORPUS,
            setup = { journey.searchAndOpen() },
            measure = {
                journey.followDocumentLink()
                journey.scrollDocument(2)
            },
        )

    @Test
    fun glossaryNavigation() =
        interaction(
            nodes = MEDIUM_CORPUS,
            measure = { journey.openGlossaryTerm() },
        )

    @Test
    fun readingDuringIncrementalIndexing() =
        interaction(
            nodes = LARGE_CORPUS,
            setup = { journey.searchAndOpen() },
            measure = {
                journey.startIndexContention()
                journey.scrollDocument(12)
            },
        )

    private fun search(nodes: Int) =
        interaction(nodes = nodes, measure = { journey.searchAndOpen() })

    private fun interaction(
        nodes: Int,
        setup: () -> Unit = {},
        measure: () -> Unit,
    ) {
        journey.stabilizeDisplay()
        try {
            journey.prepareFromInstrumentation(nodes)
            benchmark.measureRepeated(
                packageName = TARGET_PACKAGE,
                metrics =
                    listOf(
                        FrameTimingMetric(),
                        MemoryUsageMetric(MemoryUsageMetric.Mode.Max),
                    ),
                compilationMode = CompilationMode.Partial(BaselineProfileMode.UseIfAvailable),
                iterations = 8,
                setupBlock = {
                    killProcess()
                    journey.launch(this)
                    setup()
                },
                measureBlock = { measure() },
            )
        } finally {
            journey.releaseDisplay()
        }
    }
}
