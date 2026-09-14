/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.pm.ApplicationInfo
import android.os.Build
import android.webkit.WebSettings
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class DocumentViewSettingsTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private lateinit var view: WebView

    @Before
    fun mount() {
        composeRule.setContent {
            DocumentContentView(
                source = DocumentSource(id = "settings", generation = 1, org = Notes.RICH),
                presentation =
                    documentPresentation(
                        density = LocalDensity.current,
                        dark = false,
                        motion = SlipboxMotion(),
                        availableWidth = 411.dp,
                    ),
                modifier = Modifier.fillMaxSize().testTag(DOCUMENT_TAG),
            )
        }
        composeRule.waitForIdle()
        view =
            checkNotNull(
                composeRule.runOnIdle { documentViewIn(composeRule.activity.window.decorView) },
            ) {
                "no document view is composed"
            }
        view.awaitMounted()
    }

    @Test
    fun everythingBeyondRunningTheRendererIsOff() {
        composeRule.runOnIdle { inspect(view.settings) }
    }

    @Test
    fun onlyADebuggableBuildIsInspectable() {
        assertFalse("a release build", DocumentSettings.debuggingAllowed(0))
        assertTrue(
            "a debuggable build",
            DocumentSettings.debuggingAllowed(ApplicationInfo.FLAG_DEBUGGABLE),
        )
        val flags = composeRule.activity.applicationInfo.flags
        assertEquals(
            "this build, which the suite runs against",
            (flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0,
            DocumentSettings.debuggingAllowed(flags),
        )
    }

    @Test
    fun theDocumentIsReadFromTheLocalOriginOverHttps() {
        assertEquals(DocumentOrigin.PAGE, composeRule.runOnIdle { view.url })
        assertEquals(DocumentOrigin.ORIGIN, view.text("location.origin"))
        assertEquals("https:", view.text("location.protocol"))
        assertEquals("the local origin is a secure context", "true", view.answer("isSecureContext"))
    }

    @Test
    fun theOnlyThingHandedToThePageIsOneChannel() {
        val reachable = view.strings(NAMES)
        val own = reachable.filter { it.startsWith("slipbox") }
        assertEquals(
            "the whole of the app's own surface",
            listOf(DocumentChannel.OBJECT_NAME, "slipboxHost"),
            own,
        )
        assertEquals(
            "the channel carries one way to speak",
            "function",
            view.text("typeof window.${DocumentChannel.OBJECT_NAME}.postMessage"),
        )
        val platform = reachable.filterNot { own.contains(it) }
        val carried = platform.flatMap { name -> view.strings(membersOf(name)).map { "$name.$it" } }
        assertTrue(
            "the engine's own namespace carries something of the app: $carried",
            carried.none { it.contains("slipbox", ignoreCase = true) },
        )
        assertEquals(
            listOf("dispose", "present"),
            view.strings("JSON.stringify(Object.keys(window.slipboxHost).sort())"),
        )
        Evidence.record(
            "content-surface",
            Record()
                .text("app", own.joinToString(" "))
                .text("engine", platform.joinToString(" "))
                .text("engineCarries", carried.joinToString(" ")),
        )
    }

    @Suppress("DEPRECATION")
    private fun inspect(settings: WebSettings) {
        assertTrue("the renderer is script, so script runs", settings.javaScriptEnabled)
        assertTrue("no load reaches a network", settings.blockNetworkLoads)
        assertEquals(
            "no insecure subresource is admitted",
            WebSettings.MIXED_CONTENT_NEVER_ALLOW,
            settings.mixedContentMode,
        )
        assertFalse("no file is addressable", settings.allowFileAccess)
        assertFalse("no content provider is addressable", settings.allowContentAccess)
        assertFalse("no file origin reads a file", settings.allowFileAccessFromFileURLs)
        assertFalse("no file origin reads any origin", settings.allowUniversalAccessFromFileURLs)
        assertFalse("no window opens itself", settings.javaScriptCanOpenWindowsAutomatically)
        assertFalse("nothing is stored in the page", settings.domStorageEnabled)
        assertTrue("no media plays unasked", settings.mediaPlaybackRequiresUserGesture)
        assertFalse("the reader does not zoom a measured column", settings.supportZoom())
        assertFalse(settings.builtInZoomControls)
        assertFalse(settings.displayZoomControls)
        assertFalse("the viewport is the view", settings.useWideViewPort)
        assertFalse(settings.loadWithOverviewMode)
        assertEquals("utf-8", settings.defaultTextEncodingName)
        assertEquals(DocumentSettings.TEXT_ZOOM, settings.textZoom)
        assertFalse("no URL here is worth reporting", settings.safeBrowsingEnabled)
        Evidence.record(
            "content-settings",
            Record()
                .flag("javaScript", settings.javaScriptEnabled)
                .flag("blockNetworkLoads", settings.blockNetworkLoads)
                .count("mixedContentMode", settings.mixedContentMode)
                .flag("allowFileAccess", settings.allowFileAccess)
                .flag("allowContentAccess", settings.allowContentAccess)
                .flag("allowFileAccessFromFileURLs", settings.allowFileAccessFromFileURLs)
                .flag("allowUniversalAccessFromFileURLs", settings.allowUniversalAccessFromFileURLs)
                .flag("domStorage", settings.domStorageEnabled)
                .flag("safeBrowsing", settings.safeBrowsingEnabled)
                .count("textZoom", settings.textZoom),
        )
    }

    private companion object {
        const val NAMES =
            "JSON.stringify(Object.getOwnPropertyNames(window)" +
                ".filter(name => /slipbox|android|native|engine|vault|bridge/i.test(name))" +
                ".sort())"

        fun membersOf(name: String): String =
            "JSON.stringify((function () {" +
                "  const held = window[${name.quoted()}];" +
                "  if (held === null || typeof held !== 'object') { return []; }" +
                "  const named = Object.getOwnPropertyNames(held);" +
                "  for (const key in held) { named.push(String(key)); }" +
                "  return named.sort();" +
                "})())"
    }
}
