/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.content

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
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
    setOf("image/png", "image/jpeg", "image/gif", "image/webp")

private val INLINE_ASSET_EXTENSIONS = setOf("png", "jpg", "jpeg", "gif", "webp")
private val LINK_SCHEME = Regex("^[A-Za-z][A-Za-z0-9+.-]*:")

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

private const val MAX_ASSET_TARGET_BYTES = 4096
private val ENCODED_TARGET = Regex("[0-9a-f]+")

/** Retains target spelling; repository containment remains the resolver's responsibility. */
internal fun documentAssetRequest(path: String): DocumentAssetRequest? {
    val separator = path.indexOf('/')
    if (separator <= 0) {
        return null
    }
    val token = path.substring(0, separator)
    val encoded = path.substring(separator + 1)
    if (
        !TOKEN.matches(token) ||
            encoded.length !in 2..(MAX_ASSET_TARGET_BYTES * 2) ||
            encoded.length % 2 != 0 ||
            !ENCODED_TARGET.matches(encoded)
    ) {
        return null
    }
    val bytes = ByteArray(encoded.length / 2)
    for (index in bytes.indices) {
        bytes[index] = encoded.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
    val target =
        try {
            Charsets.UTF_8
                .newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (_: CharacterCodingException) {
            return null
        }
    if (target.isEmpty()) return null
    return DocumentAssetRequest(token, target)
}

internal fun documentAssetPath(token: String, target: String): String {
    val bytes = target.toByteArray(Charsets.UTF_8)
    require(bytes.size in 1..MAX_ASSET_TARGET_BYTES)
    val encoded = buildString(bytes.size * 2) {
        for (byte in bytes) append("%02x".format(Locale.ROOT, byte.toInt() and 0xff))
    }
    return "$token/$encoded"
}

internal fun documentAssetUrl(token: String, target: String): String =
    DocumentOrigin.assetBase(token) + documentAssetPath(token, target).substringAfter('/')

internal fun admittedAssetType(mimeType: String): String? =
    mimeType.substringBefore(';').trim().lowercase(Locale.ROOT).takeIf { it in ASSET_TYPES }

internal fun admittedInlineAssetTarget(target: String): Boolean {
    val path =
        if (target.startsWith("file:", ignoreCase = true)) {
            target.substring("file:".length)
        } else {
            if (LINK_SCHEME.containsMatchIn(target)) return false
            target
        }.substringBefore("::")
    return path.substringAfterLast('.', "").lowercase(Locale.ROOT) in INLINE_ASSET_EXTENSIONS
}
