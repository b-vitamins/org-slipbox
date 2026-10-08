/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {

    @get:Rule val profile = BaselineProfileRule()

    private val journey = BenchmarkJourney()

    @Before
    fun prepareCorpus() {
        journey.stabilizeDisplay()
        journey.prepareFromInstrumentation(MEDIUM_CORPUS)
    }

    @After
    fun releaseDisplay() {
        journey.releaseDisplay()
    }

    @Test
    fun startup() =
        profile.collect(
            packageName = TARGET_PACKAGE,
            includeInStartupProfile = true,
            filterPredicate = ::applicationRule,
        ) {
            startActivityAndWait(journey.readerIntent())
        }

    @Test
    fun searchAndReading() =
        profile.collect(
            packageName = TARGET_PACKAGE,
            filterPredicate = ::applicationRule,
        ) {
            startActivityAndWait(journey.readerIntent())
            journey.searchAndOpen()
            journey.followDocumentLink()
            device.pressBack()
            journey.scrollDocument()
        }

    private fun applicationRule(rule: String): Boolean =
        rule.contains("Lio/github/b_vitamins/slipbox/") &&
            !rule.contains("Lio/github/b_vitamins/slipbox/benchmark/")
}

internal const val SMALL_CORPUS = 250
internal const val MEDIUM_CORPUS = 2_500
internal const val LARGE_CORPUS = 10_000
