/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import android.content.res.AssetManager
import android.webkit.WebResourceResponse
import androidx.webkit.WebViewAssetLoader
import java.io.ByteArrayInputStream
import java.io.IOException
import java.io.InputStream

internal object DocumentResponses {
    private val HEADERS =
        mapOf(
            "X-Content-Type-Options" to "nosniff",
            "Cache-Control" to "no-store",
        )

    fun of(mimeType: String, encoding: String?, bytes: InputStream): WebResourceResponse =
        WebResourceResponse(mimeType, encoding, 200, "OK", HEADERS, bytes)

    fun of(mimeType: String, encoding: String?, bytes: ByteArray): WebResourceResponse =
        of(mimeType, encoding, ByteArrayInputStream(bytes))

    // A non-null response prevents network fallback.
    fun refused(): WebResourceResponse =
        WebResourceResponse(
            "text/plain",
            "utf-8",
            404,
            "Not Found",
            HEADERS,
            ByteArrayInputStream(ByteArray(0)),
        )
}

/** Serves only files packaged in the document asset directory. */
internal class DocumentBundleHandler(
    private val assets: AssetManager,
    private val directory: String = DocumentOrigin.BUNDLE_DIRECTORY,
) : WebViewAssetLoader.PathHandler {
    private val packaged: Set<String> by lazy { packagedFiles(assets, directory) }

    override fun handle(path: String): WebResourceResponse? {
        val name = bundleEntry(path, packaged) ?: return null
        val mimeType = bundleMimeType(name) ?: return null
        return try {
            DocumentResponses.of(mimeType, bundleEncoding(mimeType), assets.open("$directory/$name"))
        } catch (unreadable: IOException) {
            null
        }
    }
}

internal fun interface DocumentAssetSource {
    fun open(token: String, target: String): DocumentAsset?
}

internal class DocumentAssetHandler(
    private val source: DocumentAssetSource,
) : WebViewAssetLoader.PathHandler {
    override fun handle(path: String): WebResourceResponse? {
        val request = documentAssetRequest(path) ?: return null
        val asset = source.open(request.token, request.target) ?: return null
        val mimeType = admittedAssetType(asset.mimeType) ?: return null
        return DocumentResponses.of(mimeType, bundleEncoding(mimeType), asset.bytes)
    }
}

internal fun documentAssetLoader(
    assets: AssetManager,
    source: DocumentAssetSource,
): WebViewAssetLoader =
    WebViewAssetLoader
        .Builder()
        .setDomain(DocumentOrigin.AUTHORITY)
        .setHttpAllowed(false)
        .addPathHandler(DocumentOrigin.BUNDLE_PREFIX, DocumentBundleHandler(assets))
        .addPathHandler(DocumentOrigin.ASSET_PREFIX, DocumentAssetHandler(source))
        .build()

private fun packagedFiles(assets: AssetManager, directory: String): Set<String> {
    val found = sortedSetOf<String>()
    val pending = ArrayDeque(listOf(""))
    while (pending.isNotEmpty()) {
        val relative = pending.removeFirst()
        val here = if (relative.isEmpty()) directory else "$directory/$relative"
        val children = assets.list(here).orEmpty()
        if (children.isEmpty() && relative.isNotEmpty()) {
            found.add(relative)
        }
        for (child in children) {
            pending.addLast(if (relative.isEmpty()) child else "$relative/$child")
        }
    }
    return found
}
