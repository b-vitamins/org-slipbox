/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

@file:OptIn(ExperimentalSerializationApi::class)

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.descriptors.PolymorphicKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonClassDiscriminator
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction

// Kotlin mirrors of the canonical Android UTF-8 JSON contract.

internal const val ADAPTER_PROTOCOL_VERSION: Int = 1

internal const val OPERATION_DISCRIMINATOR: String = "kind"

/** Every finite bound the boundary enforces, as the library declares them. */
@Serializable
data class AdapterLimits(
    @SerialName("max_request_bytes") val maxRequestBytes: Int,
    @SerialName("max_response_bytes") val maxResponseBytes: Int,
    @SerialName("max_page_entries") val maxPageEntries: Int,
    @SerialName("max_relation_entries") val maxRelationEntries: Int,
    @SerialName("max_note_source_lines") val maxNoteSourceLines: Int,
    @SerialName("max_context_lines") val maxContextLines: Int,
    @SerialName("max_path_bytes") val maxPathBytes: Int,
    @SerialName("max_open_sessions") val maxOpenSessions: Int,
    @SerialName("max_queued_requests") val maxQueuedRequests: Int,
)

/**
 * Version 1 accepts positive native bounds no larger than these limits.
 */
internal val ADMITTED_LIMITS: AdapterLimits =
    AdapterLimits(
        maxRequestBytes = 65536,
        maxResponseBytes = 4194304,
        maxPageEntries = 200,
        maxRelationEntries = 1000,
        maxNoteSourceLines = 1000,
        maxContextLines = 200,
        maxPathBytes = 4096,
        maxOpenSessions = 8,
        maxQueuedRequests = 32,
    )

internal const val MAX_CONTRACT_BYTES: Int = 16384

internal fun AdapterLimits.declared(): List<Int> =
    listOf(
        maxRequestBytes,
        maxResponseBytes,
        maxPageEntries,
        maxRelationEntries,
        maxNoteSourceLines,
        maxContextLines,
        maxPathBytes,
        maxOpenSessions,
        maxQueuedRequests,
    )

internal fun AdapterLimits.withinPolicy(): Boolean =
    declared().zip(ADMITTED_LIMITS.declared()).all { (declared, admitted) ->
        declared in 1..admitted
    }

/**
 * Reserve queue space for admitted requests, every session retirement and host disposal.
 * Widen before addition to reject overflowing capacities.
 */
internal fun AdapterLimits.laneCapacity(): Int {
    val capacity = maxQueuedRequests.toLong() + maxOpenSessions.toLong() + 1L
    if (capacity !in 1L..Int.MAX_VALUE.toLong()) {
        throw EngineContractException(EngineFault.LIMIT_MISMATCH)
    }
    return capacity.toInt()
}

/** The immutable source and generation a session answers for. */
@Serializable
data class GenerationBinding(
    val source: String,
    val generation: String,
)

/** The explicit root and database a session is constructed against. */
@Serializable
data class SessionContext(
    val root: String,
    val database: String,
)

/** Reading and internal index maintenance are separate capabilities. */
@Serializable
enum class AdapterCapability {
    @SerialName("read")
    READ,

    @SerialName("maintenance")
    MAINTENANCE,
}

@Serializable
enum class SearchNodesSort {
    @SerialName("relevance")
    RELEVANCE,

    @SerialName("title")
    TITLE,

    @SerialName("file")
    FILE,

    @SerialName("file-mtime")
    FILE_MTIME,

    @SerialName("backlink-count")
    BACKLINK_COUNT,

    @SerialName("forward-link-count")
    FORWARD_LINK_COUNT,
}

@Serializable
enum class ExplorationLens {
    @SerialName("structure")
    STRUCTURE,

    @SerialName("refs")
    REFS,

    @SerialName("time")
    TIME,

    @SerialName("tasks")
    TASKS,

    @SerialName("bridges")
    BRIDGES,

    @SerialName("dormant")
    DORMANT,

    @SerialName("unresolved")
    UNRESOLVED,
}

/**
 * Read-only operations for Notes, Glossary, Relations and live Explorations.
 */
@Serializable
@JsonClassDiscriminator(OPERATION_DISCRIMINATOR)
sealed class ReadOperation {

    @Serializable
    @SerialName("status")
    data object Status : ReadOperation()

    @Serializable
    @SerialName("indexedFiles")
    data object IndexedFiles : ReadOperation()

    @Serializable
    @SerialName("searchNodes")
    data class SearchNodes(
        val query: String,
        val limit: Int,
        val sort: SearchNodesSort? = null,
    ) : ReadOperation()

    @Serializable
    @SerialName("searchNodeContent")
    data class SearchNodeContent(
        val query: String,
        val limit: Int,
    ) : ReadOperation()

    @Serializable
    @SerialName("nodeFromId")
    data class NodeFromId(
        val id: String,
    ) : ReadOperation()

    @Serializable
    @SerialName("nodeFromKey")
    data class NodeFromKey(
        @SerialName("node_key") val nodeKey: String,
    ) : ReadOperation()

    @Serializable
    @SerialName("readNodeSource")
    data class ReadNodeSource(
        @SerialName("node_key") val nodeKey: String,
        @SerialName("context_before") val contextBefore: Int? = null,
        @SerialName("context_after") val contextAfter: Int? = null,
        @SerialName("max_lines") val maxLines: Int? = null,
    ) : ReadOperation()

    @Serializable
    @SerialName("listGlossaryTerms")
    data class ListGlossaryTerms(
        val limit: Int,
        val after: String? = null,
    ) : ReadOperation()

    @Serializable
    @SerialName("searchGlossary")
    data class SearchGlossary(
        val query: String,
        val limit: Int,
    ) : ReadOperation()

    @Serializable
    @SerialName("glossaryTerm")
    data class GlossaryTerm(
        @SerialName("node_key") val nodeKey: String,
    ) : ReadOperation()

    @Serializable
    @SerialName("backlinks")
    data class Backlinks(
        @SerialName("node_key") val nodeKey: String,
        val limit: Int,
        val unique: Boolean = false,
    ) : ReadOperation()

    @Serializable
    @SerialName("forwardLinks")
    data class ForwardLinks(
        @SerialName("node_key") val nodeKey: String,
        val limit: Int,
        val unique: Boolean = false,
    ) : ReadOperation()

    @Serializable
    @SerialName("explore")
    data class Explore(
        @SerialName("node_key") val nodeKey: String,
        val lens: ExplorationLens,
        val limit: Int,
        val unique: Boolean = false,
    ) : ReadOperation()
}

/**
 * Initial indexing and dedicated file-index update or removal.
 */
@Serializable
@JsonClassDiscriminator(OPERATION_DISCRIMINATOR)
sealed class MaintenanceOperation {

    @Serializable
    @SerialName("index")
    data object Index : MaintenanceOperation()

    @Serializable
    @SerialName("indexFile")
    data class IndexFile(
        @SerialName("file_path") val filePath: String,
    ) : MaintenanceOperation()
}

@Serializable
internal data class OpenRequest(
    val capability: AdapterCapability,
    val binding: GenerationBinding,
    val context: SessionContext,
    val version: Int = ADAPTER_PROTOCOL_VERSION,
)

/**
 * The request binding must match the binding retained by its handle.
 */
@Serializable
internal data class ReadRequest(
    val handle: Long,
    val binding: GenerationBinding,
    val operation: ReadOperation,
    val version: Int = ADAPTER_PROTOCOL_VERSION,
)

@Serializable
internal data class MaintenanceRequest(
    val handle: Long,
    val binding: GenerationBinding,
    val operation: MaintenanceOperation,
    val version: Int = ADAPTER_PROTOCOL_VERSION,
)

@Serializable
internal data class CloseRequest(
    val handle: Long,
    val binding: GenerationBinding,
    val version: Int = ADAPTER_PROTOCOL_VERSION,
)

/**
 * An explicitly versioned adapter response.
 */
@Serializable
@JsonClassDiscriminator("outcome")
sealed class AdapterResponse {

    abstract val version: Int

    /** What the library admits, so a mismatched one is refused before it is used. */
    @Serializable
    @SerialName("contract")
    data class Contract(
        val limits: AdapterLimits,
        @SerialName("read_operations") val readOperations: List<String>,
        @SerialName("maintenance_operations") val maintenanceOperations: List<String>,
        override val version: Int,
    ) : AdapterResponse()

    @Serializable
    @SerialName("opened")
    data class Opened(
        val handle: Long,
        val capability: AdapterCapability,
        val binding: GenerationBinding,
        override val version: Int,
    ) : AdapterResponse()

    @Serializable
    @SerialName("answered")
    data class Answered(
        val handle: Long,
        val binding: GenerationBinding,
        /** The canonical engine result, under the operation that produced it. */
        val answer: EngineAnswer,
        override val version: Int,
    ) : AdapterResponse()

    @Serializable
    @SerialName("closed")
    data class Closed(
        val handle: Long,
        /** True on the call that retired the session, false on a later repeat. */
        val retired: Boolean,
        /** The binding the retired session held, and null on a later repeat. */
        val binding: GenerationBinding?,
        override val version: Int,
    ) : AdapterResponse()

    /**
     * A refusal with closed reason/bound tokens and no engine diagnostic prose.
     */
    @Serializable
    @SerialName("refused")
    data class Refused(
        val reason: RefusalReason,
        val bound: AdapterBound? = null,
        val engine: EngineRefusal? = null,
        override val version: Int,
    ) : AdapterResponse() {

        /** The refusal as its closed tokens, carrying no caller input. */
        fun summary(): String =
            buildString {
                append(reason.token)
                bound?.let { append(", bound ").append(it.token) }
                engine?.let { append(", engine code ").append(it.code) }
            }
    }
}

/** The classified numbers of an engine refusal, without its prose. */
@Serializable
data class EngineRefusal(
    val code: Int,
    val kind: EngineRefusalKind? = null,
)

@Serializable
enum class EngineRefusalKind {
    @SerialName("parse-error")
    PARSE_ERROR,

    @SerialName("invalid-params")
    INVALID_PARAMS,

    @SerialName("method-not-found")
    METHOD_NOT_FOUND,

    @SerialName("not-found")
    NOT_FOUND,

    @SerialName("conflict")
    CONFLICT,

    @SerialName("path-denied")
    PATH_DENIED,

    @SerialName("stale-request")
    STALE_REQUEST,

    @SerialName("internal")
    INTERNAL,
}

@Serializable
enum class RefusalReason {
    @SerialName("unsupported-version")
    UNSUPPORTED_VERSION,

    @SerialName("malformed-request")
    MALFORMED_REQUEST,

    @SerialName("unknown-operation")
    UNKNOWN_OPERATION,

    @SerialName("out-of-bounds")
    OUT_OF_BOUNDS,

    @SerialName("invalid-context")
    INVALID_CONTEXT,

    @SerialName("binding-mismatch")
    BINDING_MISMATCH,

    @SerialName("capability-mismatch")
    CAPABILITY_MISMATCH,

    @SerialName("unknown-handle")
    UNKNOWN_HANDLE,

    @SerialName("retired-handle")
    RETIRED_HANDLE,

    @SerialName("sessions-exhausted")
    SESSIONS_EXHAUSTED,

    @SerialName("identities-exhausted")
    IDENTITIES_EXHAUSTED,

    @SerialName("engine-refused")
    ENGINE_REFUSED,

    @SerialName("engine-failed")
    ENGINE_FAILED,

    @SerialName("uncanonical-result")
    UNCANONICAL_RESULT,

    @SerialName("encoding-failed")
    ENCODING_FAILED,

    @SerialName("panicked")
    PANICKED,
}

@Serializable
enum class AdapterBound {
    @SerialName("request-bytes")
    REQUEST_BYTES,

    @SerialName("response-bytes")
    RESPONSE_BYTES,

    @SerialName("page-entries")
    PAGE_ENTRIES,

    @SerialName("relation-entries")
    RELATION_ENTRIES,

    @SerialName("note-source-lines")
    NOTE_SOURCE_LINES,

    @SerialName("context-lines")
    CONTEXT_LINES,

    @SerialName("path-bytes")
    PATH_BYTES,

    @SerialName("open-sessions")
    OPEN_SESSIONS,

    @SerialName("queued-requests")
    QUEUED_REQUESTS,
}

/**
 * Fixed diagnostic categories for native contract failures.
 */
enum class EngineFault {
    NO_CONTRACT,
    OVERSIZED_CONTRACT,
    LIMIT_MISMATCH,
    VOCABULARY_MISMATCH,
    NO_ANSWER,
    NOT_UTF8,
    MALFORMED,
    AMBIGUOUS,
    UNSUPPORTED_VERSION,
    UNEXPECTED_OUTCOME,
    FOREIGN_ANSWER,
    FOREIGN_SESSION,
    FOREIGN_RETIREMENT,
    ;

    /** The fixed sentence this fault reports. */
    val summary: String
        get() =
            when (this) {
                NO_CONTRACT -> "the library declares no contract"
                OVERSIZED_CONTRACT -> "the contract is longer than a contract may be"
                LIMIT_MISMATCH -> "the library declares bounds this contract does not admit"
                VOCABULARY_MISMATCH -> "the library admits another operation vocabulary"
                NO_ANSWER -> "the library allocated no answer"
                NOT_UTF8 -> "the answer is not standard UTF-8"
                MALFORMED -> "the answer is not a document of this contract"
                AMBIGUOUS -> "the answer names one member of one object twice"
                UNSUPPORTED_VERSION -> "the library answers another protocol version"
                UNEXPECTED_OUTCOME -> "the library answered another outcome"
                FOREIGN_ANSWER -> "the library answered for another session or operation"
                FOREIGN_SESSION -> "the library opened a session for another capability or binding"
                FOREIGN_RETIREMENT -> "the library retired another session"
            }
}

/** The wire token of this reason, read from the contract rather than restated. */
val RefusalReason.token: String
    get() = RefusalReason.serializer().descriptor.getElementName(ordinal)

/** The wire token of this bound, read from the contract rather than restated. */
val AdapterBound.token: String
    get() = AdapterBound.serializer().descriptor.getElementName(ordinal)

internal fun ReadOperation.kind(): String =
    EngineWire.discriminant(ReadOperation.serializer(), this)

internal fun MaintenanceOperation.kind(): String =
    EngineWire.discriminant(MaintenanceOperation.serializer(), this)

/**
 * Strict request/response encoding with explicit optional fields.
 */
internal object EngineWire {

    /** A sealed descriptor lists its subclasses as its second element. */
    private const val SEALED_SUBCLASSES = 1

    private const val ESCAPED_DIGITS = 4

    private const val HEXADECIMAL = 16

    private const val FORM_FEED = '\u000C'

    private val json =
        Json {
            encodeDefaults = true
            explicitNulls = true
            ignoreUnknownKeys = false
            isLenient = false
        }

    /** Every operation a reading session can name. */
    val readOperations: Set<String>
        get() = vocabulary(ReadOperation.serializer().descriptor)

    /** Every operation a maintenance session can name. */
    val maintenanceOperations: Set<String>
        get() = vocabulary(MaintenanceOperation.serializer().descriptor)

    fun encode(request: OpenRequest): ByteArray =
        utf8(json.encodeToString(OpenRequest.serializer(), request))

    fun encode(request: ReadRequest): ByteArray =
        utf8(json.encodeToString(ReadRequest.serializer(), request))

    fun encode(request: MaintenanceRequest): ByteArray =
        utf8(json.encodeToString(MaintenanceRequest.serializer(), request))

    fun encode(request: CloseRequest): ByteArray =
        utf8(json.encodeToString(CloseRequest.serializer(), request))

    /**
     * Read operation discriminants from the sealed serializer descriptor.
     */
    fun vocabulary(descriptor: SerialDescriptor): Set<String> {
        require(descriptor.kind == PolymorphicKind.SEALED) {
            "${descriptor.serialName} is not a sealed hierarchy"
        }
        return descriptor.getElementDescriptor(SEALED_SUBCLASSES).elementNames.toSet()
    }

    fun <T> discriminant(serializer: SerializationStrategy<T>, operation: T): String {
        val named = json.encodeToJsonElement(serializer, operation).jsonObject
        val discriminant = named[OPERATION_DISCRIMINATOR]
        return checkNotNull(discriminant?.jsonPrimitive?.content) {
            "${serializer.descriptor.serialName} names no operation"
        }
    }

    /**
     * Reject invalid UTF-8 and duplicate decoded keys before typed parsing.
     * Parser diagnostic text is not exposed.
     */
    fun decode(answer: ByteArray): AdapterResponse {
        val document = text(answer)
        if (!unambiguous(document)) {
            throw EngineContractException(EngineFault.AMBIGUOUS)
        }
        return try {
            json.decodeFromString(AdapterResponse.serializer(), document)
        } catch (malformed: SerializationException) {
            throw EngineContractException(EngineFault.MALFORMED)
        }
    }

    /**
     * Compare decoded object keys, ignoring string values. Syntax validation
     * remains the typed parser's responsibility.
     */
    private fun unambiguous(document: String): Boolean {
        // Object frames hold keys; array frames have no key set.
        val frames = mutableListOf<MutableSet<String>?>()
        var awaitingName = false
        var index = 0
        while (index < document.length) {
            when (document[index]) {
                '{' -> {
                    frames.add(mutableSetOf())
                    awaitingName = true
                    index++
                }
                '[' -> {
                    frames.add(null)
                    awaitingName = false
                    index++
                }
                '}', ']' -> {
                    if (frames.isNotEmpty()) {
                        frames.removeAt(frames.size - 1)
                    }
                    awaitingName = false
                    index++
                }
                ',' -> {
                    awaitingName = frames.lastOrNull() != null
                    index++
                }
                '"' -> {
                    val name = StringBuilder()
                    index = read(document, index, name)
                    val members = frames.lastOrNull()
                    if (awaitingName && members != null && !members.add(name.toString())) {
                        return false
                    }
                    awaitingName = false
                }
                else -> index++
            }
        }
        return true
    }

    /**
     * Decode a string and return the index past it, or the document end if incomplete.
     */
    private fun read(document: String, quote: Int, name: StringBuilder): Int {
        var index = quote + 1
        while (index < document.length) {
            val character = document[index]
            if (character == '"') {
                return index + 1
            }
            if (character != '\\') {
                name.append(character)
                index++
                continue
            }
            if (index + 1 == document.length) {
                return document.length
            }
            val escape = document[index + 1]
            index += 2
            when (escape) {
                'u' -> {
                    if (index + ESCAPED_DIGITS > document.length) {
                        return document.length
                    }
                    val code =
                        document.substring(index, index + ESCAPED_DIGITS)
                            .toIntOrNull(HEXADECIMAL) ?: return document.length
                    name.append(code.toChar())
                    index += ESCAPED_DIGITS
                }
                'b' -> name.append('\b')
                'f' -> name.append(FORM_FEED)
                'n' -> name.append('\n')
                'r' -> name.append('\r')
                't' -> name.append('\t')
                else -> name.append(escape)
            }
        }
        return document.length
    }

    private fun utf8(document: String): ByteArray = document.toByteArray(Charsets.UTF_8)

    private fun text(answer: ByteArray): String {
        val decoder =
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
        return try {
            decoder.decode(ByteBuffer.wrap(answer)).toString()
        } catch (malformed: CharacterCodingException) {
            throw EngineContractException(EngineFault.NOT_UTF8)
        }
    }
}
