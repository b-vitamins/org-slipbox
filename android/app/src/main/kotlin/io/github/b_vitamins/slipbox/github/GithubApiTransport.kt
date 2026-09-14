/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import java.io.InputStream
import java.net.URL
import javax.net.ssl.HttpsURLConnection

internal class GithubApiRequest(val address: GithubAddress, val bearer: String?)

internal sealed interface GithubApiReply {

    /** [bodyExceeded] marks an incomplete, locally capped body. */
    class Answered(
        val status: Int,
        val body: String,
        val headers: GithubApiHeaders,
        val bodyExceeded: Boolean = false,
    ) : GithubApiReply

    class Failed(val origin: String) : GithubApiReply
}

internal class GithubApiHeaders(
    val link: String? = null,
    val retryAfterSeconds: Long? = null,
    val rateLimitRemaining: Long? = null,
    val rateLimitResetEpochSeconds: Long? = null,
    val singleSignOn: String? = null,
) {

    companion object {

        val None = GithubApiHeaders()

        fun of(
            link: String?,
            retryAfter: String?,
            rateLimitRemaining: String?,
            rateLimitReset: String?,
            singleSignOn: String?,
        ): GithubApiHeaders =
            GithubApiHeaders(
                link = bounded(link, MAX_LINK_CHARS),
                retryAfterSeconds = count(retryAfter),
                rateLimitRemaining = count(rateLimitRemaining),
                rateLimitResetEpochSeconds = count(rateLimitReset),
                singleSignOn = bounded(singleSignOn, MAX_FIELD_CHARS),
            )

        private fun bounded(value: String?, limit: Int): String? =
            value?.takeIf { it.isNotEmpty() && it.length <= limit }

        private fun count(value: String?): Long? =
            bounded(value, MAX_COUNT_CHARS)?.toLongOrNull()?.takeIf { it >= 0 }

        private const val MAX_LINK_CHARS = 4096

        private const val MAX_FIELD_CHARS = 512

        private const val MAX_COUNT_CHARS = 19
    }
}

internal fun interface GithubApiTransport {

    fun fetch(request: GithubApiRequest): GithubApiReply
}

/** Bounded HTTPS reads without redirects or response-bearing diagnostics. */
internal class HttpsGithubApiTransport(
    private val connectTimeoutMillis: Int = CONNECT_TIMEOUT_MILLIS,
    private val readTimeoutMillis: Int = READ_TIMEOUT_MILLIS,
) : GithubApiTransport {

    override fun fetch(request: GithubApiRequest): GithubApiReply {
        var connection: HttpsURLConnection? = null
        return try {
            val opened = URL(request.address.url).openConnection() as HttpsURLConnection
            connection = opened
            opened.connectTimeout = connectTimeoutMillis
            opened.readTimeout = readTimeoutMillis
            opened.useCaches = false
            opened.instanceFollowRedirects = false
            opened.setRequestProperty("Accept", ACCEPT)
            opened.setRequestProperty("User-Agent", AGENT)
            opened.setRequestProperty("X-GitHub-Api-Version", API_VERSION)
            request.bearer?.let { opened.setRequestProperty("Authorization", "Bearer $it") }
            val status = opened.responseCode
            val stream =
                if (status < HttpsURLConnection.HTTP_BAD_REQUEST) {
                    opened.inputStream
                } else {
                    opened.errorStream
                }
            val body = read(stream)
            GithubApiReply.Answered(status, body.text, headersOf(opened), body.exceeded)
        } catch (error: Exception) {
            GithubApiReply.Failed(error.javaClass.name)
        } finally {
            connection?.disconnect()
        }
    }

    private fun headersOf(connection: HttpsURLConnection): GithubApiHeaders =
        GithubApiHeaders.of(
            link = connection.getHeaderField("Link"),
            retryAfter = connection.getHeaderField("Retry-After"),
            rateLimitRemaining = connection.getHeaderField("X-RateLimit-Remaining"),
            rateLimitReset = connection.getHeaderField("X-RateLimit-Reset"),
            singleSignOn = connection.getHeaderField("X-GitHub-SSO"),
        )

    private fun read(stream: InputStream?): Body {
        if (stream == null) {
            return Body("", false)
        }
        return stream.use { open ->
            val bytes = ByteArray(MAX_BODY_BYTES + 1)
            var filled = 0
            while (filled < bytes.size) {
                val read = open.read(bytes, filled, bytes.size - filled)
                if (read < 0) {
                    break
                }
                filled += read
            }
            if (filled > MAX_BODY_BYTES) {
                Body("", true)
            } else {
                Body(String(bytes, 0, filled, Charsets.UTF_8), false)
            }
        }
    }

    private class Body(val text: String, val exceeded: Boolean)

    private companion object {

        const val ACCEPT = "application/vnd.github+json"

        const val API_VERSION = "2022-11-28"

        const val AGENT = "Slipbox-Android"

        const val CONNECT_TIMEOUT_MILLIS = 15_000

        const val READ_TIMEOUT_MILLIS = 20_000

        // A repository or tree page outgrows the authorization reply cap without being unbounded.
        const val MAX_BODY_BYTES = 2 * 1024 * 1024
    }
}
