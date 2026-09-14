/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.put
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicLong

/** One call the library received, and the thread it was made on. */
internal data class NativeCall(val entry: String, val request: JsonObject, val thread: String)

/**
 * A stand-in for the packaged library: the same documents, none of the engine.
 *
 * It answers canonically until a test poses another library, and can hold an
 * arrived call so a queue, a cancellation or a disposal races nothing.
 */
internal class FakeNativeLibrary : NativeSeam {

    /** What the library declares it admits. */
    var contractDocument: JsonObject = EngineFixtures.response("contract")

    /** What a call answers, by entry name and request document. */
    var reply: (String, JsonObject) -> ByteArray? = { entry, request -> canonical(entry, request) }

    /** Counted down as a call arrives. */
    var arrival: CountDownLatch? = null

    /** Awaited by an arrived call before it answers. */
    var gate: CountDownLatch? = null

    val calls: MutableList<NativeCall> = CopyOnWriteArrayList()

    /** Handed out so the first session opened is the one the shared requests name. */
    private val handles = AtomicLong(EngineOperations.READ_HANDLE - 1)

    fun requests(entry: String): List<JsonObject> =
        calls.filter { it.entry == entry }.map { it.request }

    override fun contract(): ByteArray? = call("contract", JsonObject(emptyMap()))

    override fun openRead(request: ByteArray): ByteArray? = call("openRead", parsed(request))

    override fun openMaintenance(request: ByteArray): ByteArray? =
        call("openMaintenance", parsed(request))

    override fun read(request: ByteArray): ByteArray? = call("read", parsed(request))

    override fun maintain(request: ByteArray): ByteArray? = call("maintain", parsed(request))

    override fun closeSession(request: ByteArray): ByteArray? =
        call("closeSession", parsed(request))

    /** The canonical answer of the operation [request] names. */
    fun answered(request: JsonObject): JsonObject =
        answered(request, EngineOperations.answers.getValue(named(request)))

    fun answered(request: JsonObject, answer: JsonElement): JsonObject =
        buildJsonObject {
            put("version", ADAPTER_PROTOCOL_VERSION)
            put("outcome", "answered")
            put("handle", request.getValue("handle"))
            put("binding", request.getValue("binding"))
            put("answer", answer)
        }

    fun closed(request: JsonObject, retired: Boolean = true): JsonObject =
        buildJsonObject {
            put("version", ADAPTER_PROTOCOL_VERSION)
            put("outcome", "closed")
            put("handle", request.getValue("handle"))
            put("retired", retired)
            put("binding", if (retired) request.getValue("binding") else JsonNull)
        }

    fun encoded(document: JsonObject): ByteArray = document.toString().toByteArray(Charsets.UTF_8)

    /** One shared response document, as the library would answer it. */
    fun response(key: String): ByteArray = encoded(EngineFixtures.response(key))

    fun parsed(document: ByteArray): JsonObject =
        Json.parseToJsonElement(document.decodeToString()).jsonObject

    fun handleOf(request: JsonObject): Long = request.getValue("handle").jsonPrimitive.long

    /** What the library answers when a test has posed nothing else. */
    fun canonical(entry: String, request: JsonObject): ByteArray =
        encoded(
            when (entry) {
                "contract" -> contractDocument
                "openRead" -> opened(request, "read")
                "openMaintenance" -> opened(request, "maintenance")
                "read", "maintain" -> answered(request)
                else -> closed(request)
            },
        )

    private fun call(entry: String, request: JsonObject): ByteArray? {
        calls += NativeCall(entry, request, Thread.currentThread().name)
        arrival?.countDown()
        gate?.await()
        return reply(entry, request)
    }

    private fun named(request: JsonObject): String =
        request.getValue("operation").jsonObject.getValue(OPERATION_DISCRIMINATOR)
            .jsonPrimitive.content

    private fun opened(request: JsonObject, capability: String): JsonObject =
        buildJsonObject {
            put("version", ADAPTER_PROTOCOL_VERSION)
            put("outcome", "opened")
            put("handle", handles.incrementAndGet())
            put("capability", capability)
            put("binding", request.getValue("binding"))
        }
}
