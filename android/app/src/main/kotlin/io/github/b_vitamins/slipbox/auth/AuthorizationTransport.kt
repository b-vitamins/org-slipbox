/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

internal class AuthorizationRequest(
    val url: String,
    val form: Map<String, String>? = null,
    val bearer: String? = null,
) {

    override fun toString(): String =
        "AuthorizationRequest(${AuthorizationRequestPolicy.label(url)}," +
            " ${if (form == null) "read" else "form ${form.keys.sorted()}"}," +
            " ${if (bearer == null) "unauthorized" else "bearer redacted"})"
}

/** The only destinations and credential channels the device flow requires. */
internal object AuthorizationRequestPolicy {

    private val FORM_ENDPOINTS =
        setOf(
            "https://github.com/login/device/code",
            "https://github.com/login/oauth/access_token",
        )

    private val BEARER_ENDPOINTS =
        setOf(
            "https://api.github.com/user",
            "https://api.github.com/user/installations",
        )

    fun accepts(request: AuthorizationRequest): Boolean =
        when {
            request.url in FORM_ENDPOINTS -> request.form != null && request.bearer == null
            request.url in BEARER_ENDPOINTS ->
                request.form == null && !request.bearer.isNullOrEmpty()
            else -> false
        }

    fun label(url: String): String = url.takeIf { it in FORM_ENDPOINTS || it in BEARER_ENDPOINTS }
        ?: "refused address"
}

internal sealed interface AuthorizationReply {


    data class Answered(val status: Int, val body: String) : AuthorizationReply


    data class Failed(val origin: String) : AuthorizationReply
}

/** Blocking exchange; callers must run off the UI thread. */
internal fun interface AuthorizationTransport {

    fun exchange(request: AuthorizationRequest): AuthorizationReply
}
