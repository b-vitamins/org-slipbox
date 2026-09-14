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
        "AuthorizationRequest($url, ${if (form == null) "read" else "form ${form.keys.sorted()}"}," +
            " ${if (bearer == null) "unauthorized" else "bearer redacted"})"
}

internal sealed interface AuthorizationReply {


    data class Answered(val status: Int, val body: String) : AuthorizationReply


    data class Failed(val origin: String) : AuthorizationReply
}

/** Blocking exchange; callers must run off the UI thread. */
internal fun interface AuthorizationTransport {

    fun exchange(request: AuthorizationRequest): AuthorizationReply
}
