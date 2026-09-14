/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

/** Wire fixtures shared with the instrumentation suite. */
internal object EngineFixtures {

    private val trees = mutableMapOf<String, JsonObject>()

    fun request(key: String): JsonObject = entry("requests", key)

    fun response(key: String): JsonObject = entry("responses", key)

    private fun entry(tree: String, key: String): JsonObject {
        val document = synchronized(trees) { trees.getOrPut(tree) { load(tree) } }
        return requireNotNull(document[key]?.jsonObject) { "the $tree fixture declares no $key" }
    }

    private fun load(tree: String): JsonObject {
        val resource = "/engine/$tree.json"
        val stream =
            requireNotNull(EngineFixtures::class.java.getResourceAsStream(resource)) {
                "$resource is not on the test classpath"
            }
        return stream.use { Json.parseToJsonElement(it.readBytes().decodeToString()).jsonObject }
    }
}
