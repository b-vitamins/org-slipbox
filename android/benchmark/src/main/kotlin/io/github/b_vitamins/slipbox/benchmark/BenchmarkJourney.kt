/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.benchmark

import android.content.ComponentName
import android.content.Intent
import android.view.KeyEvent
import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.StaleObjectException
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until

internal const val TARGET_PACKAGE = "io.github.b_vitamins.slipbox.benchmark"
private const val APP_PACKAGE = "io.github.b_vitamins.slipbox"
private const val CORPUS_ACTIVITY = "$APP_PACKAGE.benchmark.BenchmarkCorpusActivity"
private const val READER_ACTIVITY = "$APP_PACKAGE.benchmark.BenchmarkSlipboxActivity"
private const val CORPUS_STATUS_PREFIX = "Benchmark fixture "
private const val READY_PREFIX = "Benchmark fixture ready:"
private const val TIMEOUT = 30_000L
private const val PREPARE_TIMEOUT = 180_000L

internal class BenchmarkJourney(
    val device: UiDevice = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()),
) {

    fun stabilizeDisplay() {
        device.setOrientationNatural()
        device.waitForIdle()
    }

    fun releaseDisplay() {
        device.unfreezeRotation()
    }

    fun prepareFromInstrumentation(nodes: Int) {
        device.executeShellCommand("am force-stop $TARGET_PACKAGE")
        device.executeShellCommand(
            "am start -W -n $TARGET_PACKAGE/$CORPUS_ACTIVITY --ei nodes $nodes",
        )
        val status =
            requireNotNull(
                device.wait(
                    Until.findObject(By.textStartsWith(CORPUS_STATUS_PREFIX)),
                    PREPARE_TIMEOUT,
                ),
            ) { "the $nodes-node corpus did not report readiness" }
        check(status.text.startsWith(READY_PREFIX)) { status.text }
        device.executeShellCommand("am force-stop $TARGET_PACKAGE")
    }

    fun launch(scope: MacrobenchmarkScope) {
        scope.startActivityAndWait(readerIntent())
        awaitText("What are you looking for?")
    }

    fun searchAndOpen(query: String = "Momentum") {
        require(query.matches(Regex("[A-Za-z0-9]+"))) { "benchmark queries must be shell-safe" }
        val field =
            requireNotNull(
                device.wait(Until.findObject(By.clazz("android.widget.EditText")), TIMEOUT),
            ) { "the corpus search field did not appear" }
        field.click()
        field.text = query
        requireNotNull(device.wait(Until.findObject(By.text(query)), TIMEOUT)) {
            "search input did not accept $query; field=${field.text} " +
                "package=${device.currentPackageName}"
        }
        // Keep the measured surface stable on real phones where the IME would
        // otherwise consume most of a landscape or compact viewport.
        device.pressKeyCode(KeyEvent.KEYCODE_ESCAPE)
        awaitWebView()
        device.waitForIdle()
        clickWebView(horizontalFraction = 0.25f, verticalDp = 44)
        check(device.wait(Until.gone(By.clazz("android.widget.EditText")), TIMEOUT)) {
            "the first search result did not open"
        }
        awaitWebView()
        device.waitForIdle()
    }

    fun followDocumentLink() {
        clickWebView(horizontalFraction = 0.72f, verticalDp = 132)
        device.waitForIdle()
    }

    fun openGlossaryTerm() {
        awaitText("Glossary").click()
        awaitText("All terms")
        val term = scrollToText("Gradient flow")
        val bounds = term.visibleBounds
        check(device.click(bounds.centerX(), bounds.centerY())) {
            "the glossary term click was not injected"
        }
        check(device.wait(Until.gone(By.text("All terms")), TIMEOUT)) {
            "the glossary term did not open"
        }
        awaitWebView()
        device.waitForIdle()
    }

    fun scrollDocument(repetitions: Int = 8) {
        val display =
            InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics
        repeat(repetitions) {
            check(
                device.swipe(
                    display.widthPixels / 2,
                    display.heightPixels * 4 / 5,
                    display.widthPixels / 2,
                    display.heightPixels / 4,
                    12,
                ),
            ) { "the document swipe was not injected" }
        }
    }

    fun startIndexContention() {
        device.executeShellCommand(
            "am broadcast " +
                "-n $TARGET_PACKAGE/$APP_PACKAGE.benchmark.BenchmarkIndexReceiver " +
                "-a $APP_PACKAGE.benchmark.INDEX",
        )
    }

    fun corpusIntent(nodes: Int): Intent =
        Intent().apply {
            component = ComponentName(TARGET_PACKAGE, CORPUS_ACTIVITY)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            putExtra("nodes", nodes)
        }

    fun readerIntent(): Intent =
        Intent().apply {
            component = ComponentName(TARGET_PACKAGE, READER_ACTIVITY)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        }

    private fun awaitText(text: String): UiObject2 =
        requireNotNull(device.wait(Until.findObject(By.text(text)), TIMEOUT)) {
            "no visible element had text: $text"
        }

    private fun awaitWebView(): UiObject2 =
        requireNotNull(device.wait(Until.findObject(By.clazz("android.webkit.WebView")), TIMEOUT)) {
            "the rendered content did not appear"
        }

    private fun scrollToText(text: String): UiObject2 {
        val display =
            InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics
        repeat(12) {
            device.findObject(By.text(text))?.let { return it }
            check(
                device.swipe(
                    display.widthPixels / 2,
                    display.heightPixels * 4 / 5,
                    display.widthPixels / 2,
                    display.heightPixels / 3,
                    12,
                ),
            ) { "the glossary swipe was not injected" }
            device.waitForIdle()
        }
        throw IllegalArgumentException("no glossary row had text: $text")
    }

    private fun clickWebView(horizontalFraction: Float, verticalDp: Int) {
        repeat(5) {
            try {
                val bounds = awaitWebView().visibleBounds
                val density =
                    InstrumentationRegistry.getInstrumentation()
                        .targetContext.resources.displayMetrics.density
                val x = bounds.left + (bounds.width() * horizontalFraction).toInt()
                val verticalOffset =
                    (verticalDp * density).toInt().coerceAtMost(bounds.height() - 1)
                val y = bounds.top + verticalOffset
                check(device.click(x, y)) { "the rendered content click was not injected" }
                return
            } catch (_: StaleObjectException) {
                device.waitForIdle()
            }
        }
        throw IllegalArgumentException("the rendered content did not remain stable for a click")
    }

}
