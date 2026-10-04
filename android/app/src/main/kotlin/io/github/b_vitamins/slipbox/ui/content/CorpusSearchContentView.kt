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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import io.github.b_vitamins.slipbox.engine.ContentSegment
import io.github.b_vitamins.slipbox.engine.CorpusSearchEntity
import io.github.b_vitamins.slipbox.engine.CorpusSearchHit
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.DocumentTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import java.util.UUID
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/** Search rows rendered by the same Org and KaTeX implementation as the web surface. */
@Composable
internal fun CorpusSearchContentView(
    hits: List<CorpusSearchHit>,
    presentation: DocumentPresentation,
    onOpen: (CorpusSearchHit) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val raised = rememberUpdatedState(onOpen)
    key(context) {
        val host =
            remember {
                CorpusSearchHost(context) { hit -> raised.value(hit) }
            }
        AndroidView(
            factory = { host.view },
            modifier = modifier,
            update = { host.present(hits, presentation) },
            onRelease = { host.dispose() },
        )
    }
}

private data class SearchMount(val token: String, val hits: List<CorpusSearchHit>)

private class CorpusSearchHost(
    context: Context,
    private val onOpen: (CorpusSearchHit) -> Unit,
) {
    private val loader = documentAssetLoader(context.assets) { _, _ -> null }
    private var mounted: SearchMount? = null
    private var pending: Presented? = null
    private var displayed: Presented? = null
    private var ready = false
    private var retired = false

    val view: WebView =
        WebView(context).apply {
            DocumentSettings.harden(this, DocumentSettings.debuggingAllowed(context.applicationInfo.flags))
            webViewClient = Client()
            listen(this)
            loadUrl(DocumentOrigin.SEARCH_PAGE)
        }

    fun present(hits: List<CorpusSearchHit>, presentation: DocumentPresentation) {
        if (retired) return
        val live = mounted
        val next =
            if (live != null && live.hits == hits) {
                live
            } else {
                SearchMount(UUID.randomUUID().toString(), hits)
            }
        mounted = next
        view.setBackgroundColor(background(presentation.theme))
        val presented = Presented(next, presentation)
        if (displayed == presented) return
        displayed = presented
        if (ready) {
            view.evaluateJavascript(SearchPayload.present(next, presentation), null)
        } else {
            pending = presented
        }
    }

    fun dispose() {
        if (retired) return
        retired = true
        mounted = null
        pending = null
        displayed = null
        if (ready) {
            ready = false
            view.evaluateJavascript(SearchPayload.DISPOSE) { view.destroy() }
        } else {
            view.destroy()
        }
    }

    private fun flush() {
        val presented = pending ?: return
        pending = null
        view.evaluateJavascript(
            SearchPayload.present(presented.mounted, presented.presentation),
            null,
        )
    }

    private fun listen(view: WebView) {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) return
        WebViewCompat.addWebMessageListener(
            view,
            SearchSelectionChannel.OBJECT_NAME,
            setOf(DocumentOrigin.ORIGIN),
        ) { _, message, origin, mainFrame, _ ->
            val text = if (message.type == WebMessageCompat.TYPE_STRING) message.data else null
            val key =
                SearchSelectionChannel.read(
                    origin = origin.toString(),
                    mainFrame = mainFrame,
                    message = text,
                    token = mounted?.token,
                )
            val hit = mounted?.hits?.firstOrNull { it.node.nodeKey == key }
            if (hit != null) onOpen(hit)
            else if (key != null) Log.w(TAG, "refused a stale search selection")
        }
    }

    private inner class Client : WebViewClient() {
        override fun shouldInterceptRequest(
            view: WebView,
            request: WebResourceRequest,
        ): WebResourceResponse? {
            val url = request.url
            if (!local(url) || request.method != METHOD_GET) return DocumentResponses.refused()
            if (!request.isForMainFrame && bundleMimeType(url.path.orEmpty()) == "text/html") {
                return DocumentResponses.refused()
            }
            return loader.shouldInterceptRequest(url) ?: DocumentResponses.refused()
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = true

        @Deprecated("Called in place of the request overload below API 24.")
        override fun shouldOverrideUrlLoading(view: WebView, url: String): Boolean = true

        override fun onPageFinished(view: WebView, url: String) {
            if (retired || url != DocumentOrigin.SEARCH_PAGE) return
            ready = true
            flush()
        }
    }

    private data class Presented(
        val mounted: SearchMount,
        val presentation: DocumentPresentation,
    )

    private companion object {
        const val TAG = "CorpusSearchHost"
        const val METHOD_GET = "GET"

        fun local(url: Uri): Boolean =
            url.scheme == "https" && url.authority == DocumentOrigin.AUTHORITY

        fun background(theme: DocumentTheme): Int =
            when (theme) {
                DocumentTheme.Light -> SlipboxTokens.Palette.PAPER_LIGHT
                DocumentTheme.Dark -> SlipboxTokens.Palette.PAPER_DARK
            }.toInt()
    }
}

private object SearchPayload {
    const val DISPOSE = "if (window.slipboxSearchHost) window.slipboxSearchHost.dispose();"

    private val format = Json

    fun present(mounted: SearchMount, presentation: DocumentPresentation): String =
        buildString {
            append("if (window.slipboxSearchHost) window.slipboxSearchHost.present({\"token\":")
            appendQuoted(mounted.token)
            append(",\"results\":")
            append(format.encodeToString(mounted.hits.map(SearchResult::of)))
            append(",\"presentation\":").append(presentation.toJson())
            append("});")
        }

    @Serializable
    private data class SearchResult(
        val key: String,
        val title: String,
        val tags: List<String>,
        val term: Boolean,
        val excerpt: List<ContentSegment>,
    ) {
        companion object {
            fun of(hit: CorpusSearchHit): SearchResult =
                SearchResult(
                    key = hit.node.nodeKey,
                    title = hit.node.title,
                    tags = hit.node.tags,
                    term = hit.entity == CorpusSearchEntity.GLOSSARY,
                    excerpt = hit.excerpt.segments,
                )
        }
    }
}

private object SearchSelectionChannel {
    const val OBJECT_NAME = "slipboxSearch"
    private const val MESSAGE_LIMIT = 8192
    private val format = Json { ignoreUnknownKeys = false }

    fun read(origin: String?, mainFrame: Boolean, message: String?, token: String?): String? {
        if (origin?.trimEnd('/') != DocumentOrigin.ORIGIN || !mainFrame) return null
        if (message == null || message.length > MESSAGE_LIMIT) return null
        val selection =
            try {
                format.decodeFromString<Selection>(message)
            } catch (_: SerializationException) {
                return null
            }
        if (token == null || selection.token != token) return null
        return selection.key.takeIf { it.isNotEmpty() && it.length <= 4096 }
    }

    @Serializable
    private data class Selection(val token: String, @SerialName("key") val key: String)
}
