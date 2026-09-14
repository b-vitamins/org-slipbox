/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

/** Authorization failures without response bodies or exception messages. */
sealed class AuthorizationFault {


    abstract val stage: AuthorizationStage

    final override fun toString(): String = "AuthorizationFault(${stage.description}, $description)"

    internal abstract val description: String


    object ConfigurationUnavailable : AuthorizationFault() {

        override val stage: AuthorizationStage = AuthorizationStage.Configuration

        override val description: String = "no registration"
    }


    data class RequestRefused(
        override val stage: AuthorizationStage,
        val refusal: GrantRefusal,
    ) : AuthorizationFault() {

        override val description: String = refusal.description
    }


    data class TransportFailed(
        override val stage: AuthorizationStage,
        val origin: String,
    ) : AuthorizationFault() {

        override val description: String = "transport $origin"
    }


    data class UnexpectedStatus(
        override val stage: AuthorizationStage,
        val status: Int,
    ) : AuthorizationFault() {

        override val description: String = "status $status"
    }


    data class AttemptFailed(val origin: String) : AuthorizationFault() {

        override val stage: AuthorizationStage = AuthorizationStage.Attempt

        override val description: String = "attempt $origin"
    }


    data class MalformedAnswer(
        override val stage: AuthorizationStage,
        val defect: AnswerDefect,
    ) : AuthorizationFault() {

        override val description: String = defect.description
    }
}

enum class AuthorizationStage(internal val description: String) {
    Configuration("configuration"),
    Attempt("attempt"),
    DeviceCode("device code"),
    Grant("grant"),
    Account("account"),
    Installations("installations"),
}

enum class AnswerDefect(internal val description: String) {
    NotText("not text"),
    NotJson("not an object"),
    FieldMissing("field missing"),
    FieldUnusable("field unusable"),
}

enum class GrantRefusal(internal val token: String, internal val description: String) {

    DeviceFlowDisabled("device_flow_disabled", "device flow disabled"),


    ClientUnrecognized("incorrect_client_credentials", "client unrecognized"),


    DeviceCodeUnrecognized("incorrect_device_code", "device code unrecognized"),


    GrantUnsupported("unsupported_grant_type", "grant unsupported"),


    Unclassified("", "unclassified");

    internal companion object {

        fun of(token: String): GrantRefusal =
            entries.firstOrNull { it.token.isNotEmpty() && it.token == token } ?: Unclassified
    }
}
