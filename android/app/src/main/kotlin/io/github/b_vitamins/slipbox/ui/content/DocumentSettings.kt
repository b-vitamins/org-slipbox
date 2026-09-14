/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.annotation.SuppressLint
import android.content.pm.ApplicationInfo
import android.os.Build
import android.webkit.WebSettings
import android.webkit.WebView

internal object DocumentSettings {
    // DocumentPresentation already applies platform font scaling.
    const val TEXT_ZOOM = 100

    fun debuggingAllowed(flags: Int): Boolean = flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    @SuppressLint("SetJavaScriptEnabled")
    fun harden(view: WebView, debuggable: Boolean) {
        WebView.setWebContentsDebuggingEnabled(debuggable)
        with(view.settings) {
            javaScriptEnabled = true
            blockNetworkLoads = true
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            allowFileAccess = false
            allowContentAccess = false
            refuseFileUrlAccess()
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
            domStorageEnabled = false
            setGeolocationEnabled(false)
            mediaPlaybackRequiresUserGesture = true
            setSupportZoom(false)
            builtInZoomControls = false
            displayZoomControls = false
            useWideViewPort = false
            loadWithOverviewMode = false
            textZoom = TEXT_ZOOM
            defaultTextEncodingName = "utf-8"
            setNeedInitialFocus(false)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                // The local-only document has no URLs to report.
                safeBrowsingEnabled = false
            }
        }
    }
}

@Suppress("DEPRECATION")
private fun WebSettings.refuseFileUrlAccess() {
    allowFileAccessFromFileURLs = false
    allowUniversalAccessFromFileURLs = false
}
