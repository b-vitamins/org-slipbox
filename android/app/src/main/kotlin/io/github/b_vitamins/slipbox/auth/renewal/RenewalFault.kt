/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.AnswerDefect
import io.github.b_vitamins.slipbox.security.VaultFailure

/** Credential-free diagnostics; exception origins are class names, not messages. */
sealed class RenewalFault {

    internal abstract val description: String

    final override fun toString(): String = "RenewalFault($description)"

    object ConfigurationUnavailable : RenewalFault() {

        override val description: String = "no registration"
    }

    data class TransportFailed(val origin: String) : RenewalFault() {

        override val description: String = "transport $origin"
    }

    data class UnexpectedStatus(val status: Int) : RenewalFault() {

        override val description: String = "status $status"
    }

    data class MalformedAnswer(val defect: AnswerDefect) : RenewalFault() {

        override val description: String = defect.description
    }

    data class RequestRefused(val refusal: RenewalRefusal) : RenewalFault() {

        override val description: String = refusal.description
    }

    data class VaultRefused(val failure: VaultFailure) : RenewalFault() {

        override val description: String = "vault ${failure.summary}"
    }
}

enum class RenewalRefusal(internal val token: String, internal val description: String) {

    ClientUnrecognized("incorrect_client_credentials", "client unrecognized"),

    GrantUnsupported("unsupported_grant_type", "grant unsupported"),

    Unclassified("", "unclassified"),
    ;

    internal companion object {

        fun of(token: String): RenewalRefusal =
            entries.firstOrNull { it.token.isNotEmpty() && it.token == token } ?: Unclassified
    }
}
