/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.git

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonClassDiscriminator
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction

internal const val GIT_PROTOCOL_VERSION: Int = 1
internal const val MAX_GIT_REQUEST_BYTES: Int = 16 * 1024
internal const val MAX_GIT_RESPONSE_BYTES: Int = 4 * 1024

@Serializable
internal data class GitRequest(
    val operation: Long,
    val remote: String,
    val branch: String,
    val repository: String,
    val version: Int = GIT_PROTOCOL_VERSION,
)

@Serializable
@JsonClassDiscriminator("outcome")
internal sealed class GitResponse {

    abstract val version: Int

    @Serializable
    @SerialName("fetched")
    data class Fetched(
        override val version: Int,
        val operation: Long,
        val disposition: GitDisposition,
        val revision: String,
        @SerialName("received_objects") val receivedObjects: Long,
    ) : GitResponse()

    @Serializable
    @SerialName("refused")
    data class Refused(
        override val version: Int,
        val reason: GitRefusalReason,
    ) : GitResponse()
}

@Serializable
enum class GitDisposition {
    @SerialName("cloned")
    CLONED,

    @SerialName("updated")
    UPDATED,

    @SerialName("unchanged")
    UNCHANGED,
}

@Serializable
enum class GitRefusalReason {
    @SerialName("unsupported-version")
    UNSUPPORTED_VERSION,

    @SerialName("malformed-request")
    MALFORMED_REQUEST,

    @SerialName("out-of-bounds")
    OUT_OF_BOUNDS,

    @SerialName("operation-conflict")
    OPERATION_CONFLICT,

    @SerialName("operations-exhausted")
    OPERATIONS_EXHAUSTED,

    @SerialName("destination-refused")
    DESTINATION_REFUSED,

    @SerialName("credential-refused")
    CREDENTIAL_REFUSED,

    @SerialName("cancelled")
    CANCELLED,

    @SerialName("storage-failed")
    STORAGE_FAILED,

    @SerialName("repository-invalid")
    REPOSITORY_INVALID,

    @SerialName("branch-unavailable")
    BRANCH_UNAVAILABLE,

    @SerialName("transport-failed")
    TRANSPORT_FAILED,

    @SerialName("encoding-failed")
    ENCODING_FAILED,

    @SerialName("panicked")
    PANICKED,
}

enum class GitContractFault {
    FOREGROUND_REFUSED,
    TLS_INITIALIZATION_FAILED,
    REQUEST_OVERSIZED,
    NO_ANSWER,
    RESPONSE_OVERSIZED,
    NOT_UTF8,
    MALFORMED,
    UNSUPPORTED_VERSION,
    FOREIGN_OPERATION,
}

internal object GitWire {

    private val json =
        Json {
            encodeDefaults = true
            explicitNulls = true
            ignoreUnknownKeys = false
            isLenient = false
        }

    fun encode(request: GitRequest): ByteArray =
        json.encodeToString(GitRequest.serializer(), request).toByteArray(Charsets.UTF_8)

    fun decode(answer: ByteArray): GitResponse {
        val decoder =
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
        val text =
            try {
                decoder.decode(ByteBuffer.wrap(answer)).toString()
            } catch (malformed: CharacterCodingException) {
                throw GitContractException(GitContractFault.NOT_UTF8)
            }
        return try {
            json.decodeFromString(GitResponse.serializer(), text)
        } catch (malformed: SerializationException) {
            throw GitContractException(GitContractFault.MALFORMED)
        }
    }
}
