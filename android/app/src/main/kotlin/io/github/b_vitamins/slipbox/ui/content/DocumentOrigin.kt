/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import java.util.Locale

/** HTTPS application content, never a filesystem URL. */
internal object DocumentOrigin {
    const val AUTHORITY = "appassets.androidplatform.net"
    const val ORIGIN = "https://$AUTHORITY"

    const val BUNDLE_PREFIX = "/bundle/"
    const val ASSET_PREFIX = "/asset/"

    const val BUNDLE_DIRECTORY = "document"
    const val PAGE_FILE = "index.html"

    const val PAGE = "$ORIGIN$BUNDLE_PREFIX$PAGE_FILE"

    fun assetBase(token: String): String = "$ORIGIN$ASSET_PREFIX$token/"
}

private val SEGMENT = Regex("[A-Za-z0-9_][A-Za-z0-9._-]*")

private val BUNDLE_TYPES =
    mapOf(
        "html" to "text/html",
        "css" to "text/css",
        "js" to "text/javascript",
        "json" to "application/json",
        "txt" to "text/plain",
        "woff2" to "font/woff2",
        "woff" to "font/woff",
        "ttf" to "font/ttf",
    )

private val ASSET_TYPES =
    setOf("image/png", "image/jpeg", "image/gif", "image/webp", "text/plain")

internal fun bundleEntry(path: String, packaged: Set<String>): String? {
    if (path.isEmpty() || path.split('/').any { !SEGMENT.matches(it) }) {
        return null
    }
    return path.takeIf { it in packaged }
}

internal fun bundleMimeType(path: String): String? =
    BUNDLE_TYPES[path.substringAfterLast('.', "").lowercase(Locale.ROOT)]

internal fun bundleEncoding(mimeType: String): String? =
    if (mimeType.startsWith("text/") || mimeType == "application/json") "utf-8" else null

internal data class DocumentAssetRequest(val token: String, val target: String)

private val TOKEN = Regex("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")

private val TRAVERSAL = setOf(".", "..")

/** Retains target spelling; repository containment remains the resolver's responsibility. */
internal fun documentAssetRequest(path: String): DocumentAssetRequest? {
    val separator = path.indexOf('/')
    if (separator <= 0) {
        return null
    }
    val token = path.substring(0, separator)
    val target = path.substring(separator + 1)
    if (!TOKEN.matches(token) || target.isEmpty()) {
        return null
    }
    if (target.split('/').any { it.isEmpty() || it in TRAVERSAL }) {
        return null
    }
    return DocumentAssetRequest(token, target)
}

internal fun admittedAssetType(mimeType: String): String? =
    mimeType.substringBefore(';').trim().lowercase(Locale.ROOT).takeIf { it in ASSET_TYPES }
