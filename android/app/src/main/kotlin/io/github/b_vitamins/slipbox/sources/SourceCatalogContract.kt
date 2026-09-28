/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.sources

import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.sync.RefreshSource
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonClassDiscriminator

internal const val SOURCE_CATALOG_PROTOCOL_VERSION = 1
internal const val MAX_SOURCE_CATALOG_REQUEST_BYTES = 32 * 1024
internal const val MAX_SOURCE_CATALOG_RESPONSE_BYTES = 64 * 1024

@Serializable
@JsonClassDiscriminator("operation")
internal sealed class SourceCatalogRequest {

    abstract val version: Int

    @Serializable
    @SerialName("load")
    data class Load(
        val catalog: String,
        override val version: Int = SOURCE_CATALOG_PROTOCOL_VERSION,
    ) : SourceCatalogRequest()

    @Serializable
    @SerialName("verify")
    data class Verify(
        val catalog: String,
        @SerialName("expected_revision") val expectedRevision: Long,
        val source: RefreshSource,
        val store: String,
        override val version: Int = SOURCE_CATALOG_PROTOCOL_VERSION,
    ) : SourceCatalogRequest()

    @Serializable
    @SerialName("commit")
    data class Commit(
        val catalog: String,
        @SerialName("expected_revision") val expectedRevision: Long,
        val source: RefreshSource,
        val store: String,
        val generation: String,
        override val version: Int = SOURCE_CATALOG_PROTOCOL_VERSION,
    ) : SourceCatalogRequest()
}

@Serializable
@JsonClassDiscriminator("outcome")
internal sealed class SourceCatalogResponse {

    abstract val version: Int

    @Serializable
    @SerialName("loaded")
    data class Loaded(
        override val version: Int,
        val revision: Long,
        @SerialName("active_source") val activeSource: RefreshSource?,
    ) : SourceCatalogResponse()

    @Serializable
    @SerialName("ready")
    data class Ready(
        override val version: Int,
        val revision: Long,
        val ready: ReadySource,
    ) : SourceCatalogResponse()

    @Serializable
    @SerialName("refused")
    data class Refused(
        override val version: Int,
        val reason: SourceCatalogFailure,
    ) : SourceCatalogResponse()
}

@Serializable
internal data class ReadySource(
    val source: RefreshSource,
    val binding: GenerationBinding,
    val revision: String,
    @SerialName("content_root") val contentRoot: String,
    val database: String,
    val stats: ReadySourceStats,
)

@Serializable
internal data class ReadySourceStats(
    @SerialName("files_indexed") val filesIndexed: Long,
    @SerialName("nodes_indexed") val nodesIndexed: Long,
    @SerialName("links_indexed") val linksIndexed: Long,
)

@Serializable
internal enum class SourceCatalogFailure {
    @SerialName("unsupported-version")
    UNSUPPORTED_VERSION,

    @SerialName("malformed-request")
    MALFORMED_REQUEST,

    @SerialName("out-of-bounds")
    OUT_OF_BOUNDS,

    @SerialName("catalog-unavailable")
    CATALOG_UNAVAILABLE,

    @SerialName("catalog-conflict")
    CATALOG_CONFLICT,

    @SerialName("source-conflict")
    SOURCE_CONFLICT,

    @SerialName("generation-unavailable")
    GENERATION_UNAVAILABLE,

    @SerialName("generation-mismatch")
    GENERATION_MISMATCH,

    @SerialName("storage-failed")
    STORAGE_FAILED,

    @SerialName("encoding-failed")
    ENCODING_FAILED,

    @SerialName("panicked")
    PANICKED,
}

internal enum class SourceCatalogFault {
    STORAGE,
    REQUEST_OVERSIZED,
    NO_ANSWER,
    RESPONSE_OVERSIZED,
    NOT_UTF8,
    MALFORMED,
    UNSUPPORTED_VERSION,
    FOREIGN_SOURCE,
    FOREIGN_GENERATION,
}

internal object SourceCatalogWire {

    private val json =
        Json {
            encodeDefaults = true
            explicitNulls = true
            ignoreUnknownKeys = false
            isLenient = false
        }

    fun encode(request: SourceCatalogRequest): ByteArray =
        json.encodeToString(SourceCatalogRequest.serializer(), request).toByteArray(Charsets.UTF_8)

    fun decode(answer: ByteArray): SourceCatalogResponse {
        val decoder =
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
        val text =
            try {
                decoder.decode(ByteBuffer.wrap(answer)).toString()
            } catch (_: CharacterCodingException) {
                throw SourceCatalogContractException(SourceCatalogFault.NOT_UTF8)
            }
        return try {
            json.decodeFromString(SourceCatalogResponse.serializer(), text)
        } catch (_: SerializationException) {
            throw SourceCatalogContractException(SourceCatalogFault.MALFORMED)
        }
    }
}

internal class SourceCatalogContractException(val fault: SourceCatalogFault) :
    RuntimeException("the native source catalog contract failed: ${fault.name.lowercase()}")
