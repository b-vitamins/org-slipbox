/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.sync

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonClassDiscriminator
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction

internal const val REFRESH_PROTOCOL_VERSION: Int = 1
internal const val MAX_REFRESH_REQUEST_BYTES: Int = 32 * 1024
internal const val MAX_REFRESH_RESPONSE_BYTES: Int = 64 * 1024
internal const val MAX_REFRESH_ATTEMPTS: Int = 3

@Serializable
internal data class RefreshRequest(
    val operation: Long,
    val attempt: Int,
    val source: RefreshSource,
    val repository: String,
    val store: String,
    val version: Int = REFRESH_PROTOCOL_VERSION,
)

@Serializable
internal data class RefreshStatusRequest(
    val source: String,
    val version: Int = REFRESH_PROTOCOL_VERSION,
)

@Serializable
data class RefreshSource(
    val id: String,
    @SerialName("display_name") val displayName: String,
    val provider: RefreshProvider,
    val visibility: RefreshVisibility,
    @SerialName("provider_repository_id") val providerRepositoryId: String? = null,
    val account: String? = null,
    val remote: String,
    val branch: String,
    @SerialName("notes_folder") val notesFolder: String,
    val credential: String? = null,
)

@Serializable
enum class RefreshProvider {
    @SerialName("generic_https")
    GENERIC_HTTPS,

    @SerialName("github")
    GITHUB,
}

@Serializable
enum class RefreshVisibility {
    @SerialName("public")
    PUBLIC,

    @SerialName("private")
    PRIVATE,
}

@Serializable
@JsonClassDiscriminator("outcome")
internal sealed class RefreshResponse {

    abstract val version: Int

    @Serializable
    @SerialName("answered")
    data class Answered(
        override val version: Int,
        val operation: Long,
        val disposition: RefreshDisposition,
        val status: RefreshStatus,
    ) : RefreshResponse()

    @Serializable
    @SerialName("refused")
    data class Refused(
        override val version: Int,
        val reason: RefreshFailureReason,
        val retry: RefreshRetry,
    ) : RefreshResponse()
}

@Serializable
enum class RefreshDisposition {
    @SerialName("started")
    STARTED,

    @SerialName("coalesced")
    COALESCED,
}

@Serializable
internal data class RefreshStatusResponse(
    val version: Int,
    val outcome: RefreshStatusOutcome,
)

@Serializable
@JsonClassDiscriminator("kind")
internal sealed class RefreshStatusOutcome {

    @Serializable
    @SerialName("known")
    data class Known(val status: RefreshStatus) : RefreshStatusOutcome()

    @Serializable
    @SerialName("idle")
    data class Idle(val source: String) : RefreshStatusOutcome()

    @Serializable
    @SerialName("refused")
    data class Refused(val reason: RefreshFailureReason) : RefreshStatusOutcome()
}

@Serializable
data class RefreshStatus(
    val source: RefreshSource,
    val operation: Long,
    val state: RefreshState,
    @SerialName("fetched_revision") val fetchedRevision: String?,
    @SerialName("ready_revision") val readyRevision: String?,
    @SerialName("ready_generation") val readyGeneration: String?,
    val progress: RefreshProgress,
    val failure: RefreshFailure?,
)

@Serializable
enum class RefreshState {
    @SerialName("recovering")
    RECOVERING,

    @SerialName("fetching")
    FETCHING,

    @SerialName("comparing")
    COMPARING,

    @SerialName("materializing")
    MATERIALIZING,

    @SerialName("indexing")
    INDEXING,

    @SerialName("publishing")
    PUBLISHING,

    @SerialName("ready")
    READY,

    @SerialName("failed")
    FAILED,

    @SerialName("cancelled")
    CANCELLED,
}

@Serializable
data class RefreshProgress(
    val completed: Long,
    val total: Long,
    val unit: RefreshProgressUnit,
)

@Serializable
enum class RefreshProgressUnit {
    @SerialName("none")
    NONE,

    @SerialName("objects")
    OBJECTS,

    @SerialName("entries")
    ENTRIES,

    @SerialName("files")
    FILES,

    @SerialName("steps")
    STEPS,
}

@Serializable
data class RefreshFailure(
    val reason: RefreshFailureReason,
    val retry: RefreshRetry,
)

@Serializable
enum class RefreshFailureReason {
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

    @SerialName("foreground-refused")
    FOREGROUND_REFUSED,

    @SerialName("tls-initialization-failed")
    TLS_INITIALIZATION_FAILED,

    @SerialName("authorization-required")
    AUTHORIZATION_REQUIRED,

    @SerialName("authorization-failed")
    AUTHORIZATION_FAILED,

    @SerialName("rate-limited")
    RATE_LIMITED,

    @SerialName("transport-failed")
    TRANSPORT_FAILED,

    @SerialName("repository-invalid")
    REPOSITORY_INVALID,

    @SerialName("branch-unavailable")
    BRANCH_UNAVAILABLE,

    @SerialName("notes-folder-unavailable")
    NOTES_FOLDER_UNAVAILABLE,

    @SerialName("unsafe-source")
    UNSAFE_SOURCE,

    @SerialName("input-exhausted")
    INPUT_EXHAUSTED,

    @SerialName("rebuild-required")
    REBUILD_REQUIRED,

    @SerialName("storage-failed")
    STORAGE_FAILED,

    @SerialName("publication-conflict")
    PUBLICATION_CONFLICT,

    @SerialName("superseded")
    SUPERSEDED,

    @SerialName("cancelled")
    CANCELLED,

    @SerialName("encoding-failed")
    ENCODING_FAILED,

    @SerialName("panicked")
    PANICKED,
}

@Serializable
@JsonClassDiscriminator("strategy")
sealed class RefreshRetry {

    @Serializable
    @SerialName("never")
    data object Never : RefreshRetry()

    @Serializable
    @SerialName("reauthorize")
    data object Reauthorize : RefreshRetry()

    @Serializable
    @SerialName("backoff")
    data class Backoff(@SerialName("after_seconds") val afterSeconds: Long) : RefreshRetry()

    @Serializable
    @SerialName("free-storage")
    data object FreeStorage : RefreshRetry()

    @Serializable
    @SerialName("rebuild")
    data object Rebuild : RefreshRetry()
}

enum class RefreshContractFault {
    FOREGROUND_REFUSED,
    TLS_INITIALIZATION_FAILED,
    REQUEST_OVERSIZED,
    NO_ANSWER,
    RESPONSE_OVERSIZED,
    NOT_UTF8,
    MALFORMED,
    UNSUPPORTED_VERSION,
    FOREIGN_OPERATION,
    FOREIGN_SOURCE,
}

internal object RefreshWire {

    private val json =
        Json {
            encodeDefaults = true
            explicitNulls = true
            ignoreUnknownKeys = false
            isLenient = false
        }

    fun encode(request: RefreshRequest): ByteArray =
        json.encodeToString(RefreshRequest.serializer(), request).toByteArray(Charsets.UTF_8)

    fun encode(request: RefreshStatusRequest): ByteArray =
        json.encodeToString(RefreshStatusRequest.serializer(), request).toByteArray(Charsets.UTF_8)

    fun decodeResponse(answer: ByteArray): RefreshResponse =
        decode(answer) { json.decodeFromString(RefreshResponse.serializer(), it) }

    fun decodeStatus(answer: ByteArray): RefreshStatusResponse =
        decode(answer) { json.decodeFromString(RefreshStatusResponse.serializer(), it) }

    private fun <T> decode(answer: ByteArray, parse: (String) -> T): T {
        val decoder =
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
        val text =
            try {
                decoder.decode(ByteBuffer.wrap(answer)).toString()
            } catch (_: CharacterCodingException) {
                throw RefreshContractException(RefreshContractFault.NOT_UTF8)
            }
        return try {
            parse(text)
        } catch (_: SerializationException) {
            throw RefreshContractException(RefreshContractFault.MALFORMED)
        }
    }
}

class RefreshContractException internal constructor(val fault: RefreshContractFault) :
    RuntimeException("the native refresh contract failed: ${fault.name.lowercase()}")
