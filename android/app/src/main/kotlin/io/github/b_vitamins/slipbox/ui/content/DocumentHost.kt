/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.Context
import android.net.Uri
import android.util.Log
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.DocumentTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import java.util.UUID

private const val TAG = "DocumentHost"
private const val METHOD_GET = "GET"

/** Owns one local WebView, its mount token and its navigation channel. */
internal class DocumentHost(
    context: Context,
    private val onIntent: (DocumentIntent) -> Unit,
    private val resolver: DocumentAssetResolver,
    debuggable: Boolean = DocumentSettings.debuggingAllowed(context.applicationInfo.flags),
) {
    private val loader =
        documentAssetLoader(context.assets) { token, target -> openAsset(token, target) }

    // Request interception also reads this on a WebView worker thread.
    @Volatile
    private var mount: DocumentMount? = null

    private var pending: Presented? = null
    private var ready = false
    private var retired = false

    val view: WebView =
        WebView(context).apply {
            DocumentSettings.harden(this, debuggable)
            webViewClient = Client()
            listen(this)
            loadUrl(DocumentOrigin.PAGE)
        }

    /** Presentation-only changes retain the token, scroll and focus. */
    fun present(source: DocumentSource, presentation: DocumentPresentation) {
        if (retired) {
            return
        }
        val live = mount
        val next = if (live != null && live.source == source) live else DocumentMount(token(), source)
        mount = next
        view.setBackgroundColor(background(presentation.theme))
        val presented = Presented(next, presentation)
        if (ready) {
            view.evaluateJavascript(DocumentPayload.present(next, presentation), null)
        } else {
            pending = presented
        }
    }

    /** Retires the token before renderer teardown and WebView destruction. */
    fun dispose() {
        if (retired) {
            return
        }
        retired = true
        mount = null
        pending = null
        if (ready) {
            ready = false
            view.evaluateJavascript(DocumentPayload.DISPOSE) { view.destroy() }
        } else {
            view.destroy()
        }
    }

    private fun flush() {
        val presented = pending ?: return
        pending = null
        view.evaluateJavascript(
            DocumentPayload.present(presented.mount, presented.presentation),
            null,
        )
    }

    private fun openAsset(token: String, target: String): DocumentAsset? {
        val live = mount ?: return null
        if (live.token != token) {
            return null
        }
        return resolver.resolve(live.binding, target)
    }

    private fun listen(view: WebView) {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(
                view,
                DocumentChannel.OBJECT_NAME,
                setOf(DocumentOrigin.ORIGIN),
            ) { _, message, origin, mainFrame, _ -> receive(message, origin, mainFrame) }
        } else {
            Log.w(TAG, "the platform WebView carries no typed message channel")
        }
    }

    private fun receive(message: WebMessageCompat, origin: Uri, mainFrame: Boolean) {
        val text = if (message.type == WebMessageCompat.TYPE_STRING) message.data else null
        when (val event = DocumentChannel.read(origin.toString(), mainFrame, text, mount?.token)) {
            is DocumentEvent.Raised -> onIntent(event.intent)
            // Refusals log the check, never note-derived payload text.
            is DocumentEvent.Refused -> Log.w(TAG, "refused a document message: ${event.reason}")
        }
    }

    private inner class Client : WebViewClient() {
        override fun shouldInterceptRequest(
            view: WebView,
            request: WebResourceRequest,
        ): WebResourceResponse? {
            val url = request.url
            if (!local(url) || request.method != METHOD_GET) {
                return DocumentResponses.refused()
            }
            // Host HTML belongs only in the main frame.
            if (!request.isForMainFrame && bundleMimeType(url.path.orEmpty()) == "text/html") {
                return DocumentResponses.refused()
            }
            // A null interception result would fall through to the network.
            return loader.shouldInterceptRequest(url) ?: DocumentResponses.refused()
        }

        override fun shouldOverrideUrlLoading(
            view: WebView,
            request: WebResourceRequest,
        ): Boolean = true

        @Deprecated("Called in place of the request overload below API 24.")
        override fun shouldOverrideUrlLoading(view: WebView, url: String): Boolean = true

        override fun onPageFinished(view: WebView, url: String) {
            if (retired || url != DocumentOrigin.PAGE) {
                return
            }
            ready = true
            flush()
        }
    }

    private data class Presented(
        val mount: DocumentMount,
        val presentation: DocumentPresentation,
    )

    private companion object {
        fun token(): String = UUID.randomUUID().toString()

        fun local(url: Uri): Boolean =
            url.scheme == "https" && url.authority == DocumentOrigin.AUTHORITY

        fun background(theme: DocumentTheme): Int =
            when (theme) {
                DocumentTheme.Light -> SlipboxTokens.Palette.SURFACE_LIGHT
                DocumentTheme.Dark -> SlipboxTokens.Palette.SURFACE_DARK
            }.toInt()
    }
}
