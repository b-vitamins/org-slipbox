/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.json.JsonElement

/** One admitted operation and the shared documents it exchanges. */
internal data class AdmittedOperation<T>(
    val operation: T,
    val request: String,
    val answer: String,
)

/** One instance of every operation the mirror declares, as the fixtures name it. */
internal object EngineOperations {

    const val SOURCE: String = "0102030405060708090a0b0c0d0e0f10"
    const val NODE_KEY: String = "heading:alpha.org:3"
    const val GLOSSARY_KEY: String = "heading:riemann.org:2"
    const val NODE_ID: String = "11111111-2222-3333-4444-555555555555"
    const val QUERY: String = "Ωμέγα théorie 漢字"
    const val READ_HANDLE: Long = 7
    const val MAINTENANCE_HANDLE: Long = 8

    val readBinding: GenerationBinding = GenerationBinding(SOURCE, "fixture-01")

    val maintenanceBinding: GenerationBinding = GenerationBinding(SOURCE, "fixture-02")

    val context: SessionContext = SessionContext("/data/fixture/root", "/data/fixture/slipbox.db")

    val read: List<AdmittedOperation<ReadOperation>> =
        listOf(
            AdmittedOperation(ReadOperation.Status, "read_status", "answered_status"),
            AdmittedOperation(
                ReadOperation.IndexedFiles,
                "read_indexed_files",
                "answered_indexed_files",
            ),
            AdmittedOperation(
                ReadOperation.SearchNodes(QUERY, 25, SearchNodesSort.TITLE),
                "read_search_nodes",
                "answered_search_nodes",
            ),
            AdmittedOperation(
                ReadOperation.SearchNodeContent(QUERY, 25),
                "read_search_node_content",
                "answered_search_node_content",
            ),
            AdmittedOperation(
                ReadOperation.NodeFromId(NODE_ID),
                "read_node_from_id",
                "answered_node_from_id",
            ),
            AdmittedOperation(
                ReadOperation.NodeFromKey(NODE_KEY),
                "read_node_from_key",
                "answered_node_from_key",
            ),
            AdmittedOperation(
                ReadOperation.ReadNodeSource(NODE_KEY, 0, 0, 1_000),
                "read_source",
                "answered_read_node_source",
            ),
            AdmittedOperation(
                ReadOperation.ListGlossaryTerms(50, null),
                "read_list_glossary_terms",
                "answered_list_glossary_terms",
            ),
            AdmittedOperation(
                ReadOperation.SearchGlossary(QUERY, 50),
                "read_search_glossary",
                "answered_search_glossary",
            ),
            AdmittedOperation(
                ReadOperation.GlossaryTerm(GLOSSARY_KEY),
                "read_glossary_term",
                "answered_glossary_term",
            ),
            AdmittedOperation(
                ReadOperation.Backlinks(NODE_KEY, 200, false),
                "read_backlinks",
                "answered_backlinks",
            ),
            AdmittedOperation(
                ReadOperation.ForwardLinks(NODE_KEY, 200, false),
                "read_forward_links",
                "answered_forward_links",
            ),
            AdmittedOperation(
                ReadOperation.Explore(NODE_KEY, ExplorationLens.STRUCTURE, 200, true),
                "read_explore",
                "answered_explore",
            ),
        )

    val maintenance: List<AdmittedOperation<MaintenanceOperation>> =
        listOf(
            AdmittedOperation(MaintenanceOperation.Index, "maintain_index", "answered_index"),
            AdmittedOperation(
                MaintenanceOperation.IndexFile("alpha.org"),
                "maintain_index_file",
                "answered_index_file",
            ),
        )

    /** The answer document of every admitted operation, under its discriminant. */
    val answers: Map<String, JsonElement> =
        (read.map { it.operation.kind() to it.answer } +
            maintenance.map { it.operation.kind() to it.answer })
            .associate { (kind, key) -> kind to EngineFixtures.response(key).getValue("answer") }
}
