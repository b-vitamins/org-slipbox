/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.PrintWriter
import java.io.StringWriter

class EngineContractTest {

    private val binding = EngineOperations.readBinding

    private val maintenanceBinding = EngineOperations.maintenanceBinding

    /** Written back with the settings the mirror reads with, so a document round trips. */
    private val writer =
        Json {
            encodeDefaults = true
            explicitNulls = true
        }

    /** Fragments of a document or a caller's input that no diagnostic may quote. */
    private val quoted =
        listOf(
            EngineOperations.QUERY,
            EngineOperations.NODE_KEY,
            EngineOperations.NODE_ID,
            EngineOperations.SOURCE,
            "alpha.org",
            "/data/fixture",
            "captureNode",
            "{",
            "\"",
        )

    @Test
    fun everySharedRequestDocumentIsTheOneThisMirrorEncodes() {
        assertEquals(
            EngineFixtures.request("open_read"),
            document(
                EngineWire.encode(
                    OpenRequest(AdapterCapability.READ, binding, EngineOperations.context),
                ),
            ),
        )
        assertEquals(
            EngineFixtures.request("open_maintenance"),
            document(
                EngineWire.encode(
                    OpenRequest(
                        AdapterCapability.MAINTENANCE,
                        maintenanceBinding,
                        EngineOperations.context,
                    ),
                ),
            ),
        )
        for (admitted in EngineOperations.read) {
            val request = ReadRequest(READ_HANDLE, binding, admitted.operation)
            assertEquals(
                admitted.request,
                EngineFixtures.request(admitted.request),
                document(EngineWire.encode(request)),
            )
        }
        for (admitted in EngineOperations.maintenance) {
            val request =
                MaintenanceRequest(MAINTENANCE_HANDLE, maintenanceBinding, admitted.operation)
            assertEquals(
                admitted.request,
                EngineFixtures.request(admitted.request),
                document(EngineWire.encode(request)),
            )
        }
        assertEquals(
            EngineFixtures.request("close"),
            document(EngineWire.encode(CloseRequest(READ_HANDLE, binding))),
        )
        assertEquals(
            EngineFixtures.request("close_other_binding"),
            document(EngineWire.encode(CloseRequest(READ_HANDLE, maintenanceBinding))),
        )
    }

    @Test
    fun everySharedAnswerIsTheOneThisMirrorTypes() {
        for (admitted in EngineOperations.read) {
            val answered = answered(admitted.answer)
            assertEquals(admitted.answer, READ_HANDLE, answered.handle)
            assertEquals(admitted.answer, binding, answered.binding)
            assertNotNull(admitted.answer, admitted.operation.answerIn(answered.answer))
            assertEquals(
                admitted.answer,
                EngineFixtures.response(admitted.answer),
                written(answered),
            )
        }
        for (admitted in EngineOperations.maintenance) {
            val answered = answered(admitted.answer)
            assertEquals(admitted.answer, MAINTENANCE_HANDLE, answered.handle)
            assertEquals(admitted.answer, maintenanceBinding, answered.binding)
            assertNotNull(admitted.answer, admitted.operation.answerIn(answered.answer))
            assertEquals(
                admitted.answer,
                EngineFixtures.response(admitted.answer),
                written(answered),
            )
        }
    }

    @Test
    fun anAnswerOfAnotherOperationIsNoAnswerOfThisOne() {
        val read = EngineOperations.read.map { answered(it.answer).answer }
        for ((operation, admitted) in EngineOperations.read.withIndex()) {
            for ((answer, carried) in read.withIndex()) {
                val paired = admitted.operation.answerIn(carried)
                if (operation == answer) {
                    assertNotNull("$operation of $answer", paired)
                } else {
                    assertNull("$operation of $answer", paired)
                }
            }
        }
        val maintenance = EngineOperations.maintenance.map { answered(it.answer).answer }
        for ((operation, admitted) in EngineOperations.maintenance.withIndex()) {
            for ((answer, carried) in maintenance.withIndex()) {
                val paired = admitted.operation.answerIn(carried)
                if (operation == answer) {
                    assertNotNull("$operation of $answer", paired)
                } else {
                    assertNull("$operation of $answer", paired)
                }
            }
        }
        val crossedRead = answered("crossed_answer_read").answer
        val crossedMaintenance = answered("crossed_answer_maintenance").answer
        for (admitted in EngineOperations.read) {
            assertNull(admitted.request, admitted.operation.answerIn(crossedMaintenance))
        }
        for (admitted in EngineOperations.maintenance) {
            assertNull(admitted.request, admitted.operation.answerIn(crossedRead))
        }
    }

    @Test
    fun theLookupsThatAdmitAbsenceCarryNoRecord() {
        val id = answered("answered_node_from_id_absent").answer
        val key = answered("answered_node_from_key_absent").answer
        val term = answered("answered_glossary_term_absent").answer

        assertEquals(EngineAnswer.NodeFromId(null), id)
        assertEquals(EngineAnswer.NodeFromKey(null), key)
        assertEquals(EngineAnswer.GlossaryTerm(GlossaryTermResult(null)), term)
        assertNotNull(ReadOperation.NodeFromId(EngineOperations.NODE_ID).answerIn(id))
        assertNotNull(ReadOperation.NodeFromKey(EngineOperations.NODE_KEY).answerIn(key))
        assertNotNull(ReadOperation.GlossaryTerm(EngineOperations.GLOSSARY_KEY).answerIn(term))
    }

    @Test
    fun theSharedContractDeclaresTheBoundsAndVocabularyThisMirrorAdmits() {
        val contract = decoded("contract") as AdapterResponse.Contract
        val read = contract.readOperations
        val maintenance = contract.maintenanceOperations

        assertEquals(ADAPTER_PROTOCOL_VERSION, contract.version)
        assertEquals(ADMITTED_LIMITS, contract.limits)
        assertTrue(contract.limits.withinPolicy())
        assertEquals(EngineWire.readOperations, read.toSet())
        assertEquals(EngineWire.maintenanceOperations, maintenance.toSet())
        assertEquals(read.size, read.toSet().size)
        assertEquals(maintenance.size, maintenance.toSet().size)
        assertEquals(EngineFixtures.response("contract"), written(contract))
    }

    @Test
    fun aBoundOutsideThePolicyIsAMismatchWhicheverBoundItIs() {
        assertTrue(ADMITTED_LIMITS.withinPolicy())
        for ((bound, admitted) in ADMITTED_LIMITS.declared().withIndex()) {
            assertEquals(bound.toString(), admitted, declaring(bound, admitted).declared()[bound])
            assertFalse(bound.toString(), declaring(bound, 0).withinPolicy())
            assertFalse(bound.toString(), declaring(bound, -1).withinPolicy())
            assertFalse(bound.toString(), declaring(bound, admitted + 1).withinPolicy())
            assertFalse(bound.toString(), declaring(bound, Int.MAX_VALUE).withinPolicy())
            assertTrue(bound.toString(), declaring(bound, 1).withinPolicy())
        }
        val outside =
            listOf(
                "bad_contract_negative_queue",
                "bad_contract_zero_queue",
                "bad_contract_unbounded_queue",
                "bad_contract_excessive_sessions",
            )
        for (key in outside) {
            val contract = decoded(key) as AdapterResponse.Contract
            assertFalse(key, contract.limits.withinPolicy())
        }
    }

    @Test
    fun boundsThatDoNotFitALaneAreAMismatchRatherThanAFailedAllocation() {
        assertEquals(41, ADMITTED_LIMITS.laneCapacity())

        val overflowing = ADMITTED_LIMITS.copy(maxQueuedRequests = Int.MAX_VALUE)
        val refusal =
            assertThrows(EngineContractException::class.java) { overflowing.laneCapacity() }
        assertEquals(EngineFault.LIMIT_MISMATCH, refusal.fault)
    }

    @Test
    fun anOpenedSessionAndItsRetirementReportTheBindingTheyCarry() {
        assertEquals(
            AdapterResponse.Opened(
                READ_HANDLE,
                AdapterCapability.READ,
                binding,
                ADAPTER_PROTOCOL_VERSION,
            ),
            decoded("opened"),
        )
        assertEquals(
            AdapterResponse.Opened(
                MAINTENANCE_HANDLE,
                AdapterCapability.MAINTENANCE,
                maintenanceBinding,
                ADAPTER_PROTOCOL_VERSION,
            ),
            decoded("opened_maintenance"),
        )
        assertEquals(
            AdapterResponse.Opened(
                READ_HANDLE,
                AdapterCapability.READ,
                maintenanceBinding,
                ADAPTER_PROTOCOL_VERSION,
            ),
            decoded("opened_other_binding"),
        )
        assertEquals(
            AdapterResponse.Closed(READ_HANDLE, true, binding, ADAPTER_PROTOCOL_VERSION),
            decoded("closed"),
        )
        assertEquals(
            AdapterResponse.Closed(READ_HANDLE, false, null, ADAPTER_PROTOCOL_VERSION),
            decoded("closed_repeat"),
        )
    }

    @Test
    fun aDocumentOutsideThisContractIsRefusedAsACategoryOnly() {
        val outside =
            listOf(
                "bad_answered_without_version",
                "bad_opened_without_version",
                "bad_answered_foreign_payload",
                "bad_answered_unknown_kind",
                "bad_answered_unknown_entry",
                "bad_answered_missing_field",
                "bad_answered_wrong_type",
                "bad_answered_null_result",
                "bad_answered_extra_answer_field",
                "bad_answered_no_kind",
                "bad_contract_missing_limit",
            )
        for (key in outside) {
            val refusal = assertThrows(key, EngineContractException::class.java) { decoded(key) }
            assertEquals(key, EngineFault.MALFORMED, refusal.fault)
            assertRedacted(key, refusal)
        }

        val prose = "an answer of a library that answers no documents at all"
        val refusal =
            assertThrows(EngineContractException::class.java) {
                EngineWire.decode(prose.toByteArray(Charsets.UTF_8))
            }
        assertEquals(EngineFault.MALFORMED, refusal.fault)
        assertFalse(trace(refusal).contains(prose))
    }

    @Test
    fun anAnswerNamingNoVersionIsRefusedRatherThanReadAsThisVersion() {
        for (key in listOf("bad_answered_without_version", "bad_opened_without_version")) {
            val refusal = assertThrows(key, EngineContractException::class.java) { decoded(key) }
            assertEquals(key, EngineFault.MALFORMED, refusal.fault)
        }
        assertEquals(2, decoded("bad_answered_unsupported_version").version)
    }

    @Test
    fun anAnswerNamingOneMemberTwiceIsRefusedRatherThanResolved() {
        val version = "\"version\":1"
        val source = "\"source\":\"0102030405060708090a0b0c0d0e0f10\""
        val ambiguous =
            listOf(
                "contradicted version" to
                    rewritten("answered_status", version, "\"version\":1,\"version\":2"),
                "reversed version" to
                    rewritten("answered_status", version, "\"version\":2,\"version\":1"),
                "identical version" to
                    rewritten("answered_status", version, "\"version\":1,\"version\":1"),
                "escaped version first" to
                    rewritten("answered_status", version, "\"\\u0076ersion\":2,\"version\":1"),
                "escaped version second" to
                    rewritten("answered_status", version, "\"version\":1,\"\\u0076ersion\":2"),
                "outcome" to
                    rewritten(
                        "answered_status",
                        "\"outcome\":\"answered\"",
                        "\"outcome\":\"answered\",\"outcome\":\"refused\"",
                    ),
                "handle" to rewritten("closed", "\"handle\":7", "\"handle\":7,\"handle\":8"),
                "retirement" to
                    rewritten("closed", "\"retired\":true", "\"retired\":true,\"retired\":false"),
                "capability" to
                    rewritten(
                        "opened",
                        "\"capability\":\"read\"",
                        "\"capability\":\"maintenance\",\"capability\":\"read\"",
                    ),
                "binding source" to rewritten("closed", source, "$source,$source"),
                "binding generation" to
                    rewritten(
                        "answered_status",
                        "\"generation\":\"fixture-01\"",
                        "\"generation\":\"fixture-02\",\"generation\":\"fixture-01\"",
                    ),
                "answer kind" to
                    rewritten(
                        "answered_status",
                        "\"kind\":\"status\"",
                        "\"kind\":\"status\",\"kind\":\"indexedFiles\"",
                    ),
                "escaped answer kind" to
                    rewritten(
                        "answered_status",
                        "\"kind\":\"status\"",
                        "\"kin\\u0064\":\"indexedFiles\",\"kind\":\"status\"",
                    ),
                "the whole answer" to
                    rewritten(
                        "answered_status",
                        "\"answer\":{",
                        "\"answer\":{\"kind\":\"indexedFiles\"},\"answer\":{",
                    ),
                "a result member" to
                    rewritten(
                        "answered_status",
                        "\"files_indexed\":2",
                        "\"files_indexed\":2,\"files_indexed\":3",
                    ),
                "a member of an object inside an array" to
                    rewritten(
                        "answered_search_nodes",
                        "\"file_path\":\"alpha.org\"",
                        "\"file_path\":\"alpha.org\",\"file_path\":\"beta.org\"",
                    ),
                "a declared bound" to
                    rewritten(
                        "contract",
                        "\"max_page_entries\":200",
                        "\"max_page_entries\":200,\"max_page_entries\":1",
                    ),
            )

        for ((case, answer) in ambiguous) {
            val refusal =
                assertThrows(case, EngineContractException::class.java) {
                    EngineWire.decode(answer)
                }
            assertEquals(case, EngineFault.AMBIGUOUS, refusal.fault)
            assertRedacted(case, refusal)
        }
    }

    @Test
    fun anAnswerNamingEachMemberOnceIsReadWhateverItsValuesHold() {
        val once =
            listOf(
                "contract",
                "contract_reordered_vocabulary",
                "opened",
                "closed",
                "refused",
                "answered_status",
                "answered_search_nodes",
                "answered_search_node_content",
                "answered_read_node_source",
                "answered_explore",
            )
        for (key in once) {
            assertNotNull(key, decoded(key))
        }

        val spelled = rewritten("answered_status", "\"version\":1", "\"\\u0076ersion\":1")
        assertEquals(ADAPTER_PROTOCOL_VERSION, EngineWire.decode(spelled).version)

        // A value quoting a document of its own is a string, whatever it spells.
        val quoting =
            rewritten(
                "answered_search_nodes",
                "\"title\":\"",
                "\"title\":\"{\\\"version\\\":1,\\\"version\\\":2} \\\\ \\\" ",
            )
        val read = EngineWire.decode(quoting) as AdapterResponse.Answered
        val nodes = (read.answer as EngineAnswer.SearchNodes).result.nodes

        assertEquals(
            "{\"version\":1,\"version\":2} \\ \" " + EngineOperations.QUERY,
            nodes.first().title,
        )
    }

    @Test
    fun anUnknownKeyOutcomeOrTokenIsRefusedRatherThanGuessedAt() {
        val outside =
            listOf(
                EngineFixtures.response("closed") + ("invented" to JsonPrimitive(1)),
                EngineFixtures.response("closed") + ("outcome" to JsonPrimitive("invented")),
                EngineFixtures.response("closed") + ("handle" to JsonPrimitive(true)),
                EngineFixtures.response("refused") + ("reason" to JsonPrimitive("invented")),
                EngineFixtures.response("refused_bound") + ("bound" to JsonPrimitive("invented")),
                EngineFixtures.response("opened") + ("capability" to JsonPrimitive("invented")),
            )

        for (answer in outside) {
            val refusal =
                assertThrows(EngineContractException::class.java) { decode(JsonObject(answer)) }
            assertEquals(EngineFault.MALFORMED, refusal.fault)
        }
    }

    @Test
    fun bytesThatAreNotStandardUtf8AreRefusedBeforeDecoding() {
        val answer = bytes(EngineFixtures.response("closed"))
        answer[answer.size - 2] = 0xC3.toByte()

        val refusal =
            assertThrows(EngineContractException::class.java) { EngineWire.decode(answer) }
        assertEquals(EngineFault.NOT_UTF8, refusal.fault)
        assertRedacted("not utf-8", refusal)
    }

    @Test
    fun noRouteOutOfAFaultQuotesTheDocumentItWasRaisedOn() {
        val marker = "synthetic-" + "9f8e7d6c5b4a39281706"
        val answered = EngineFixtures.response("answered_status")
        val quoting = JsonObject(answered + ("invented" to JsonPrimitive(marker)))

        val refusal = assertThrows(EngineContractException::class.java) { decode(quoting) }
        assertEquals(EngineFault.MALFORMED, refusal.fault)
        assertRedacted("marker", refusal)
        assertFalse(refusal.message.orEmpty().contains(marker))
        assertFalse(refusal.toString().contains(marker))
        assertFalse(trace(refusal).contains(marker))
    }

    @Test
    fun aRefusalNamesOnlyItsClosedTokens() {
        assertEquals("out-of-bounds, bound page-entries", refused("refused_bound").summary())
        assertEquals("engine-refused, engine code -32602", refused("refused").summary())
        assertEquals(
            AdapterResponse.Refused(
                RefusalReason.ENGINE_REFUSED,
                engine = EngineRefusal(-32_602, EngineRefusalKind.INVALID_PARAMS),
                version = ADAPTER_PROTOCOL_VERSION,
            ),
            refused("refused"),
        )
        assertEquals(RefusalReason.UNCANONICAL_RESULT, refused("refused_uncanonical").reason)
        assertEquals(RefusalReason.BINDING_MISMATCH, refused("refused_binding").reason)

        val kebab = Regex("[a-z]+(-[a-z0-9]+)*")
        for (reason in RefusalReason.entries) {
            assertTrue(reason.token, reason.token.matches(kebab))
        }
        for (bound in AdapterBound.entries) {
            assertTrue(bound.token, bound.token.matches(kebab))
        }
    }

    @Test
    fun everyFaultIsOneFixedSentenceAndNothingElse() {
        val prose = Regex("[A-Za-z0-9 -]+")
        val summaries = EngineFault.entries.map { it.summary }

        assertEquals(EngineFault.entries.size, summaries.toSet().size)
        for (summary in summaries) {
            assertTrue(summary, summary.matches(prose))
        }

        val beside = EngineSecondaryFault.entries.map { it.summary }

        assertEquals(EngineSecondaryFault.entries.size, beside.toSet().size)
        for (summary in beside) {
            assertTrue(summary, summary.matches(prose))
        }
    }

    @Test
    fun everyExplorationSectionAndEntryTheEngineWritesIsTyped() {
        val explore = answered("answered_explore").answer as EngineAnswer.Explore
        val sections = explore.result.sections
        val entries = sections.flatMap { it.entries }
        val explanations =
            entries.map {
                EngineWire.discriminant(ExplorationExplanation.serializer(), explanation(it))
            }

        assertEquals(ExplorationLens.STRUCTURE, explore.result.lens)
        assertEquals(ExplorationSectionKind.entries.toSet(), sections.map { it.kind }.toSet())
        assertEquals(
            setOf("Backlink", "ForwardLink", "Reflink", "UnlinkedReference", "Anchor"),
            entries.map { it::class.simpleName }.toSet(),
        )
        assertEquals(
            EngineWire.vocabulary(ExplorationExplanation.serializer().descriptor),
            explanations.toSet(),
        )
    }

    @Test
    fun everyOperationReportsTheDiscriminantItsDocumentCarries() {
        val read =
            EngineOperations.read.map { admitted ->
                val request = ReadRequest(READ_HANDLE, binding, admitted.operation)
                assertEquals(named(document(EngineWire.encode(request))), admitted.operation.kind())
                admitted.operation.kind()
            }
        val maintenance =
            EngineOperations.maintenance.map { admitted ->
                val request =
                    MaintenanceRequest(MAINTENANCE_HANDLE, maintenanceBinding, admitted.operation)
                assertEquals(named(document(EngineWire.encode(request))), admitted.operation.kind())
                admitted.operation.kind()
            }

        assertEquals(EngineWire.readOperations, read.toSet())
        assertEquals(EngineWire.maintenanceOperations, maintenance.toSet())
        assertEquals(EngineOperations.answers.keys, read.toSet() + maintenance.toSet())
    }

    /** [ADMITTED_LIMITS] with the bound at [index] of the declared order set to [bound]. */
    private fun declaring(index: Int, bound: Int): AdapterLimits =
        with(ADMITTED_LIMITS) {
            when (index) {
                0 -> copy(maxRequestBytes = bound)
                1 -> copy(maxResponseBytes = bound)
                2 -> copy(maxPageEntries = bound)
                3 -> copy(maxRelationEntries = bound)
                4 -> copy(maxNoteSourceLines = bound)
                5 -> copy(maxContextLines = bound)
                6 -> copy(maxPathBytes = bound)
                7 -> copy(maxOpenSessions = bound)
                8 -> copy(maxQueuedRequests = bound)
                else -> throw AssertionError("the policy declares no bound $index")
            }
        }

    private fun explanation(entry: ExplorationEntry): ExplorationExplanation =
        when (entry) {
            is ExplorationEntry.Backlink -> entry.record.explanation
            is ExplorationEntry.ForwardLink -> entry.record.explanation
            is ExplorationEntry.Reflink -> entry.record.explanation
            is ExplorationEntry.UnlinkedReference -> entry.record.explanation
            is ExplorationEntry.Anchor -> entry.record.explanation
        }

    private fun assertRedacted(key: String, failure: Throwable) {
        assertNull(key, failure.cause)
        assertEquals(key, 0, failure.suppressed.size)
        for (route in listOf(failure.message.orEmpty(), failure.toString(), trace(failure))) {
            for (fragment in quoted) {
                assertFalse("$key quotes $fragment", route.contains(fragment))
            }
        }
    }

    private fun trace(failure: Throwable): String {
        val written = StringWriter()
        failure.printStackTrace(PrintWriter(written))
        return written.toString()
    }

    private fun named(request: JsonObject): String =
        request.getValue("operation").jsonObject.getValue(OPERATION_DISCRIMINATOR)
            .jsonPrimitive.content

    private fun answered(key: String): AdapterResponse.Answered =
        decoded(key) as AdapterResponse.Answered

    private fun refused(key: String): AdapterResponse.Refused =
        decoded(key) as AdapterResponse.Refused

    private fun decoded(key: String): AdapterResponse = decode(EngineFixtures.response(key))

    private fun decode(document: JsonObject): AdapterResponse = EngineWire.decode(bytes(document))

    private fun written(response: AdapterResponse): JsonObject =
        writer.encodeToJsonElement(AdapterResponse.serializer(), response).jsonObject

    private fun bytes(document: JsonObject): ByteArray =
        document.toString().toByteArray(Charsets.UTF_8)

    /** Vary fixture text without a map erasing duplicate keys. */
    private fun rewritten(key: String, named: String, again: String): ByteArray {
        val text = EngineFixtures.response(key).toString()
        assertTrue("$key writes $named", text.contains(named))
        return text.replace(named, again).toByteArray(Charsets.UTF_8)
    }

    private fun document(encoded: ByteArray): JsonObject =
        Json.parseToJsonElement(encoded.decodeToString()).jsonObject

    private companion object {

        const val READ_HANDLE: Long = EngineOperations.READ_HANDLE

        const val MAINTENANCE_HANDLE: Long = EngineOperations.MAINTENANCE_HANDLE
    }
}
