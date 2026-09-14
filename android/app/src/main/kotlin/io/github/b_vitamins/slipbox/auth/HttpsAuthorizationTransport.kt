/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import java.io.InputStream
import java.net.URL
import java.net.URLEncoder
import javax.net.ssl.HttpsURLConnection

/** Bounded HTTPS exchanges without redirects or response-bearing diagnostics. */
internal class HttpsAuthorizationTransport(
    private val connectTimeoutMillis: Int = CONNECT_TIMEOUT_MILLIS,
    private val readTimeoutMillis: Int = READ_TIMEOUT_MILLIS,
) : AuthorizationTransport {

    override fun exchange(request: AuthorizationRequest): AuthorizationReply {
        if (!request.url.startsWith(HTTPS)) {
            return AuthorizationReply.Failed(INSECURE_ADDRESS)
        }
        var connection: HttpsURLConnection? = null
        return try {
            val opened = URL(request.url).openConnection() as HttpsURLConnection
            connection = opened
            opened.connectTimeout = connectTimeoutMillis
            opened.readTimeout = readTimeoutMillis
            opened.useCaches = false
            opened.instanceFollowRedirects = false
            opened.setRequestProperty("Accept", ACCEPT)
            opened.setRequestProperty("User-Agent", AGENT)
            opened.setRequestProperty("X-GitHub-Api-Version", API_VERSION)
            request.bearer?.let { opened.setRequestProperty("Authorization", "Bearer $it") }
            if (request.form != null) {
                opened.requestMethod = "POST"
                opened.doOutput = true
                opened.setRequestProperty("Content-Type", FORM_TYPE)
                opened.outputStream.use { it.write(encode(request.form)) }
            }
            val status = opened.responseCode
            val stream = if (status < HttpsURLConnection.HTTP_BAD_REQUEST) {
                opened.inputStream
            } else {
                opened.errorStream
            }
            AuthorizationReply.Answered(status, read(stream))
        } catch (error: Exception) {
            AuthorizationReply.Failed(error.javaClass.name)
        } finally {
            connection?.disconnect()
        }
    }


    private fun read(stream: InputStream?): String {
        if (stream == null) {
            return ""
        }
        return stream.use { open ->
            val bytes = ByteArray(MAX_BODY_BYTES)
            var filled = 0
            while (filled < bytes.size) {
                val read = open.read(bytes, filled, bytes.size - filled)
                if (read < 0) {
                    break
                }
                filled += read
            }
            String(bytes, 0, filled, Charsets.UTF_8)
        }
    }

    private fun encode(form: Map<String, String>): ByteArray =
        form.entries
            .joinToString("&") { (name, value) ->
                "${URLEncoder.encode(name, CHARSET)}=${URLEncoder.encode(value, CHARSET)}"
            }.toByteArray(Charsets.UTF_8)

    private companion object {

        const val HTTPS = "https://"


        const val ACCEPT = "application/json, application/vnd.github+json"

        const val API_VERSION = "2022-11-28"

        const val FORM_TYPE = "application/x-www-form-urlencoded; charset=utf-8"

        const val CHARSET = "UTF-8"

        const val CONNECT_TIMEOUT_MILLIS = 15_000

        const val READ_TIMEOUT_MILLIS = 20_000


        const val MAX_BODY_BYTES = 32 * 1024

        const val INSECURE_ADDRESS = "insecure address"

        const val AGENT = "Slipbox-Android"
    }
}
