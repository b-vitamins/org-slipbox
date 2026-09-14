/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import java.net.URI
import java.net.URISyntaxException

/** An HTTPS API address whose continuations stay on the same endpoint. */
internal class GithubAddress private constructor(val url: String, private val path: String) {

    fun continuation(destination: String): GithubAddress? =
        parse(destination)?.takeIf { it.path == path }

    override fun toString(): String = "GithubAddress($path)"

    companion object {

        const val PER_PAGE = 100

        /** Path parts are already percent-encoded; query values are encoded here. */
        fun of(path: String, query: List<Pair<String, String>> = emptyList()): GithubAddress? {
            val suffix =
                if (query.isEmpty()) {
                    ""
                } else {
                    query.joinToString("&", "?") { (name, value) ->
                        "${GithubEncoding.segment(name)}=${GithubEncoding.segment(value)}"
                    }
                }
            return parse("$ORIGIN$path$suffix")
        }

        fun paged(path: String, query: List<Pair<String, String>> = emptyList()): GithubAddress? =
            of(path, query + (PAGE_SIZE to PER_PAGE.toString()))

        /** Unvalidated next target; [continuation] checks its destination. */
        fun nextTarget(header: String?): String? {
            if (header == null || header.length > MAX_LINK_CHARS) {
                return null
            }
            var cursor = 0
            while (true) {
                val open = header.indexOf('<', cursor)
                if (open < 0) {
                    return null
                }
                val close = header.indexOf('>', open + 1)
                if (close < 0) {
                    return null
                }
                // Parameters belong to the target before them and end at the separating comma.
                val following = header.indexOf('<', close + 1)
                val separator = header.indexOf(',', close + 1)
                val end =
                    minOf(
                        if (following < 0) header.length else following,
                        if (separator < 0) header.length else separator,
                    )
                if (isNext(header.substring(close + 1, end))) {
                    return header.substring(open + 1, close).trim().takeIf { it.isNotEmpty() }
                }
                cursor = close + 1
            }
        }

        private fun parse(url: String): GithubAddress? {
            if (url.length > MAX_URL_CHARS) {
                return null
            }
            val address =
                try {
                    URI(url)
                } catch (malformed: URISyntaxException) {
                    return null
                }
            if (address.isOpaque || !SCHEME.equals(address.scheme, ignoreCase = true)) {
                return null
            }
            if (address.rawUserInfo != null || address.rawFragment != null) {
                return null
            }
            if (!HOST.equals(address.host, ignoreCase = true)) {
                return null
            }
            if (address.port != -1 && address.port != DEFAULT_PORT) {
                return null
            }
            val path = address.rawPath
            if (path.isNullOrEmpty() || !path.startsWith("/")) {
                return null
            }
            if (path.split('/').any { it == "." || it == ".." }) {
                return null
            }
            return GithubAddress(url, path)
        }

        private fun isNext(parameters: String): Boolean =
            parameters.split(';').any { entry ->
                val separator = entry.indexOf('=')
                separator > 0 &&
                    entry.substring(0, separator).trim().equals(RELATION, ignoreCase = true) &&
                    entry.substring(separator + 1).trim().trim('"') == NEXT
            }

        private const val SCHEME = "https"

        private const val HOST = "api.github.com"

        private const val DEFAULT_PORT = 443

        private const val ORIGIN = "$SCHEME://$HOST"

        private const val PAGE_SIZE = "per_page"

        private const val RELATION = "rel"

        private const val NEXT = "next"

        private const val MAX_URL_CHARS = 2048

        private const val MAX_LINK_CHARS = 4096
    }
}

internal object GithubEncoding {

    fun segment(value: String): String = encode(value, keepSlash = false)

    fun path(value: String): String = encode(value, keepSlash = true)

    private fun encode(value: String, keepSlash: Boolean): String {
        val encoded = StringBuilder(value.length)
        for (byte in value.toByteArray(Charsets.UTF_8)) {
            val code = byte.toInt() and 0xff
            val character = code.toChar()
            when {
                character in 'A'..'Z' || character in 'a'..'z' || character in '0'..'9' ->
                    encoded.append(character)
                character == '-' || character == '.' || character == '_' || character == '~' ->
                    encoded.append(character)
                keepSlash && character == '/' -> encoded.append(character)
                else -> encoded.append('%').append(HEX[code shr 4]).append(HEX[code and 0xf])
            }
        }
        return encoded.toString()
    }

    private const val HEX = "0123456789ABCDEF"
}
