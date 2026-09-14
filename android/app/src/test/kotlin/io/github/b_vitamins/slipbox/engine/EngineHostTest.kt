/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.put
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.PrintWriter
import java.io.StringWriter
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit.MILLISECONDS

private const val TIMEOUT_MILLIS = 10_000L

/** Far more abandoned calls than the lane's queue could ever hold at once. */
private const val CHURN = 200

/** What one callback was told, and on which threads. */
private class Heard : EngineCallback<ReadAnswer> {

    val answers: MutableList<ReadAnswer> = CopyOnWriteArrayList()

    val failures: MutableList<Throwable> = CopyOnWriteArrayList()

    val threads: MutableSet<String> = CopyOnWriteArraySet()

    override fun onAnswered(answer: ReadAnswer) {
        answers += answer
        threads += Thread.currentThread().name
    }

    override fun onFailed(failure: Throwable) {
        failures += failure
        threads += Thread.currentThread().name
    }
}

private const val SAID = "raised-outside-this-adapter"

private const val UNDERNEATH = "underneath-what-was-raised"

private const val BESIDE = "beside-what-was-raised"

private class Failing : EngineCallback<ReadAnswer> {

    override fun onAnswered(answer: ReadAnswer): Unit = throw built()

    override fun onFailed(failure: Throwable): Unit = throw built()
}

private fun built(): Throwable =
    IllegalStateException(SAID, IllegalArgumentException(UNDERNEATH)).apply {
        addSuppressed(IllegalStateException(BESIDE))
    }

class EngineHostTest {

    private val binding = EngineOperations.readBinding

    private val maintenanceBinding = EngineOperations.maintenanceBinding

    private val context = EngineOperations.context

    private val library = FakeNativeLibrary()

    private val hosts = mutableListOf<SlipboxEngineHost>()

    @After
    fun disposeHosts() {
        library.gate?.countDown()
        hosts.forEach { it.close() }
        hosts.forEach { it.awaitDisposal(TIMEOUT_MILLIS) }
    }

    @Test
    fun everyAdmittedOperationCrossesAsTheSharedRequestAndAnswersItsOwnType() {
        val host = host()
        val read = host.openRead(binding, context).await()
        val maintenance = host.openMaintenance(maintenanceBinding, context).await()

        assertEquals(EngineOperations.READ_HANDLE, read.handle)
        assertEquals(EngineOperations.MAINTENANCE_HANDLE, maintenance.handle)
        for (admitted in EngineOperations.read) {
            val answer = read.answer(admitted.operation).await()
            assertEquals(admitted.answer, answered(admitted.answer), answer)
        }
        for (admitted in EngineOperations.maintenance) {
            val answer = maintenance.carryOut(admitted.operation).await()
            assertEquals(admitted.answer, answered(admitted.answer), answer)
        }

        assertEquals(EngineFixtures.request("open_read"), library.requests("openRead").single())
        assertEquals(
            EngineFixtures.request("open_maintenance"),
            library.requests("openMaintenance").single(),
        )
        assertEquals(requested(EngineOperations.read), library.requests("read"))
        assertEquals(requested(EngineOperations.maintenance), library.requests("maintain"))
    }

    @Test
    fun everyCallRunsOnOneLaneAndNotTheCallersThread() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val overlapped = CopyOnWriteArrayList<String>()
        val canonical = library.reply
        library.reply = { entry, request ->
            overlapped += entry
            Thread.sleep(1)
            overlapped -= entry
            canonical(entry, request)
        }

        EngineOperations.read.map { session.answer(it.operation) }.forEach { it.await() }

        val threads = library.calls.filter { it.entry != "contract" }.map { it.thread }.toSet()
        assertEquals(threads.toString(), 1, threads.size)
        assertFalse(Thread.currentThread().name in threads)
        assertTrue(overlapped.toString(), overlapped.isEmpty())
    }

    @Test
    fun aLibraryThatAllocatesNoContractIsRefusedAtConstruction() {
        library.reply = { _, _ -> null }

        assertEquals(EngineFault.NO_CONTRACT, refusedAtConstruction().fault)
    }

    @Test
    fun aContractLongerThanAContractMayBeIsRefusedBeforeItIsDecoded() {
        val prose = "x".repeat(MAX_CONTRACT_BYTES + 1)
        library.reply = { _, _ -> prose.toByteArray(Charsets.UTF_8) }

        assertEquals(EngineFault.OVERSIZED_CONTRACT, refusedAtConstruction().fault)
    }

    @Test
    fun aContractBeyondTheBoundItDeclaresIsRefused() {
        declaring("max_response_bytes", 64)

        assertEquals(EngineFault.OVERSIZED_CONTRACT, refusedAtConstruction().fault)
    }

    @Test
    fun aContractOfAnotherVersionIsRefusedAtConstruction() {
        val other = JsonPrimitive(ADAPTER_PROTOCOL_VERSION + 1)
        library.contractDocument = JsonObject(library.contractDocument + ("version" to other))

        assertEquals(EngineFault.UNSUPPORTED_VERSION, refusedAtConstruction().fault)
    }

    @Test
    fun aLibraryThatAnswersAnotherOutcomeAtConstructionIsRefused() {
        library.contractDocument = EngineFixtures.response("opened")

        assertEquals(EngineFault.UNEXPECTED_OUTCOME, refusedAtConstruction().fault)
    }

    @Test
    fun aContractNamingOneMemberTwiceIsRefusedBeforeAHostHoldsIt() {
        library.reply = { entry, request ->
            val declared = library.canonical(entry, request)
            if (entry != "contract") {
                declared
            } else {
                ("{\"version\":2," + declared.decodeToString().substring(1))
                    .toByteArray(Charsets.UTF_8)
            }
        }

        assertEquals(EngineFault.AMBIGUOUS, refusedAtConstruction().fault)
        assertEquals(listOf("contract"), library.calls.map { it.entry })
    }

    @Test
    fun aContractOutsideThisContractIsRefusedAsACategoryOnly() {
        library.contractDocument = EngineFixtures.response("bad_contract_missing_limit")

        val refusal = refusedAtConstruction()

        assertEquals(EngineFault.MALFORMED, refusal.fault)
        assertNull(refusal.cause)
    }

    @Test
    fun aContractDeclaringBoundsThisMirrorDoesNotAdmitIsRefused() {
        val outside =
            listOf(
                "bad_contract_negative_queue",
                "bad_contract_zero_queue",
                "bad_contract_unbounded_queue",
                "bad_contract_excessive_sessions",
            )

        for (key in outside) {
            library.contractDocument = EngineFixtures.response(key)
            assertEquals(key, EngineFault.LIMIT_MISMATCH, refusedAtConstruction().fault)
        }
    }

    @Test
    fun aContractAdmittingAnotherVocabularyIsRefused() {
        val outside =
            listOf(
                "bad_contract_duplicate_operation",
                "bad_contract_unknown_operation",
                "bad_contract_short_vocabulary",
            )

        for (key in outside) {
            library.contractDocument = EngineFixtures.response(key)
            assertEquals(key, EngineFault.VOCABULARY_MISMATCH, refusedAtConstruction().fault)
        }
    }

    @Test
    fun orderCarriesNothingInTheDeclaredVocabulary() {
        library.contractDocument = EngineFixtures.response("contract_reordered_vocabulary")
        val host = host()

        val session = host.openRead(binding, context).await()

        assertEquals(answered("answered_status"), session.answer(ReadOperation.Status).await())
    }

    @Test
    fun anAnswerForAnotherSessionOrOperationIsRefused() {
        val host = host()
        val read = host.openRead(binding, context).await()
        val maintenance = host.openMaintenance(maintenanceBinding, context).await()

        for (substitution in substitutions(read)) {
            library.reply = { _, request ->
                library.encoded(JsonObject(library.answered(request) + substitution))
            }
            val refusal =
                assertThrows(
                    substitution.first,
                    EngineContractException::class.java,
                ) { read.answer(ReadOperation.Status).await() }
            assertEquals(substitution.first, EngineFault.FOREIGN_ANSWER, refusal.fault)
        }

        library.reply = { _, _ -> library.response("crossed_answer_maintenance") }
        assertEquals(
            EngineFault.FOREIGN_ANSWER,
            assertThrows(EngineContractException::class.java) {
                read.answer(ReadOperation.Status).await()
            }.fault,
        )

        library.reply = { _, _ -> library.response("crossed_answer_read") }
        assertEquals(
            EngineFault.FOREIGN_ANSWER,
            assertThrows(EngineContractException::class.java) {
                maintenance.carryOut(MaintenanceOperation.Index).await()
            }.fault,
        )
    }

    @Test
    fun anAnswerOfAnotherOutcomeOrVersionIsRefused() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val outside =
            mapOf(
                "closed" to EngineFault.UNEXPECTED_OUTCOME,
                "bad_answered_unsupported_version" to EngineFault.UNSUPPORTED_VERSION,
                "bad_answered_without_version" to EngineFault.MALFORMED,
                "bad_answered_unknown_kind" to EngineFault.MALFORMED,
                "bad_answered_null_result" to EngineFault.MALFORMED,
                "bad_answered_foreign_payload" to EngineFault.MALFORMED,
            )

        for ((key, fault) in outside) {
            library.reply = { _, _ -> library.response(key) }
            val refusal =
                assertThrows(key, EngineContractException::class.java) {
                    session.answer(ReadOperation.Status).await()
                }
            assertEquals(key, fault, refusal.fault)
            assertNull(key, refusal.cause)
        }
    }

    @Test
    fun anAnswerNamingOneMemberTwiceIsRefusedAndTheSessionKept() {
        val host = host()
        val session = host.openRead(binding, context).await()
        library.reply = { entry, request ->
            val answer = library.canonical(entry, request)
            if (entry != "read") {
                answer
            } else {
                ("{\"version\":1," + answer.decodeToString().substring(1))
                    .toByteArray(Charsets.UTF_8)
            }
        }

        val refusal =
            assertThrows(EngineContractException::class.java) {
                session.answer(ReadOperation.Status).await()
            }

        assertEquals(EngineFault.AMBIGUOUS, refusal.fault)
        assertNull(refusal.cause)

        library.reply = { entry, request -> library.canonical(entry, request) }
        assertEquals(answered("answered_status"), session.answer(ReadOperation.Status).await())
    }

    @Test
    fun aLibraryThatAllocatesNoAnswerIsReportedAsSuch() {
        val host = host()
        val session = host.openRead(binding, context).await()
        library.reply = { _, _ -> null }

        val failure =
            assertThrows(EngineContractException::class.java) {
                session.answer(ReadOperation.Status).await()
            }

        assertEquals(EngineFault.NO_ANSWER, failure.fault)
    }

    @Test
    fun anEngineRefusalReachesTheCallerAsItsClosedTokens() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val quoted = "unrepeatable-" + "0f1e2d3c4b5a"
        library.reply = { _, _ -> library.response("refused") }

        val refusal =
            assertThrows(EngineRefusedException::class.java) {
                session.answer(ReadOperation.SearchNodes(quoted, 5)).await()
            }

        assertEquals(RefusalReason.ENGINE_REFUSED, refusal.refusal.reason)
        assertEquals(-32_602, refusal.refusal.engine?.code)
        assertEquals(EngineRefusalKind.INVALID_PARAMS, refusal.refusal.engine?.kind)
        assertNull(refusal.cause)
        assertFalse(refusal.message.orEmpty().contains(quoted))
        assertFalse(refusal.toString().contains(quoted))
    }

    @Test
    fun aRequestAtTheDeclaredBoundCrossesAndALongerOneNeverDoes() {
        val admitted = ReadOperation.ReadNodeSource(EngineOperations.NODE_KEY, 0, 0, 1_000)
        val beyond = ReadOperation.ReadNodeSource(EngineOperations.NODE_KEY, 0, 0, 10_000)
        val bound = EngineWire.encode(ReadRequest(EngineOperations.READ_HANDLE, binding, admitted))
        declaring("max_request_bytes", bound.size)
        val host = host()
        val session = host.openRead(binding, context).await()

        assertEquals(answered("answered_read_node_source"), session.answer(admitted).await())
        val refusal =
            assertThrows(EngineRefusedException::class.java) { session.answer(beyond).await() }

        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.REQUEST_BYTES, refusal.refusal.bound)
        assertEquals(1, library.requests("read").size)
    }

    @Test
    fun anAnswerAtTheDeclaredBoundIsRead() {
        val answer = library.response("answered_backlinks")
        declaring("max_response_bytes", answer.size)
        val host = host()
        val session = host.openRead(binding, context).await()

        val backlinks = ReadOperation.Backlinks(EngineOperations.NODE_KEY, 200, false)

        assertEquals(answered("answered_backlinks"), session.answer(backlinks).await())
    }

    @Test
    fun anAnswerBeyondTheDeclaredBoundIsRefusedRatherThanTruncated() {
        val answer = library.response("answered_backlinks")
        declaring("max_response_bytes", answer.size - 1)
        val host = host()
        val session = host.openRead(binding, context).await()

        val refusal =
            assertThrows(EngineRefusedException::class.java) {
                session.answer(ReadOperation.Backlinks(EngineOperations.NODE_KEY, 200, false))
                    .await()
            }

        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.RESPONSE_BYTES, refusal.refusal.bound)
    }

    @Test
    fun aQueueBeyondTheDeclaredDepthIsRefused() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val depth = host.contract.limits.maxQueuedRequests
        val arrival = held()

        val queued = mutableListOf(session.answer(ReadOperation.Status))
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        repeat(depth - 1) { queued += session.answer(ReadOperation.IndexedFiles) }

        val refusal =
            assertThrows(EngineRefusedException::class.java) {
                session.answer(ReadOperation.Status)
            }
        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.QUEUED_REQUESTS, refusal.refusal.bound)

        library.gate?.countDown()
        queued.forEach { it.await() }
        assertEquals(depth, library.requests("read").size)
    }

    @Test
    fun abandonedCallsBeyondThePhysicalQueueNeitherRejectNorLeakAdmission() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val arrival = held()
        val underWay = session.answer(ReadOperation.Status)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))

        repeat(CHURN) { round ->
            val abandoned = session.answer(ReadOperation.IndexedFiles)
            assertEquals(round.toString(), EngineCancellation.DISCARDED, abandoned.cancel())
        }

        val retirement = session.retire()
        library.gate?.countDown()
        underWay.await()
        assertEquals(true, retirement.await(TIMEOUT_MILLIS))
        assertEquals(1, library.requests("read").size)
        assertEquals(listOf(session.handle), retiredHandles())
    }

    @Test
    fun aCallTheLaneHasNotStartedIsDiscardedAndNeverCrosses() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val arrival = held()

        val underWay = session.answer(ReadOperation.Status)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        val discarded = session.answer(ReadOperation.IndexedFiles)

        assertEquals(EngineCancellation.DISCARDED, discarded.cancel())
        assertTrue(discarded.isSettled)
        library.gate?.countDown()
        underWay.await()

        assertEquals(1, library.requests("read").size)
        assertEquals(EngineCancellation.SETTLED, underWay.cancel())
    }

    @Test
    fun aCallUnderWayIsNotClaimedToBeInterrupted() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val arrival = held()

        val underWay = session.answer(ReadOperation.Status)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))

        assertEquals(EngineCancellation.UNDER_WAY, underWay.cancel())
        library.gate?.countDown()
        assertEquals(answered("answered_status"), underWay.await())
        assertEquals(EngineCancellation.SETTLED, underWay.cancel())
    }

    @Test
    fun workAfterDisposalIsRejected() {
        val host = host()
        val session = host.openRead(binding, context).await()

        host.close()
        assertTrue(requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS)).isComplete)

        assertFalse(host.isOpen)
        assertFalse(session.isOpen)
        assertThrows(EngineClosedException::class.java) { session.answer(ReadOperation.Status) }
        assertThrows(EngineClosedException::class.java) { host.openRead(binding, context) }
        assertThrows(EngineClosedException::class.java) { host.openMaintenance(binding, context) }
    }

    @Test
    fun disposalRetiresEverySessionTheOwnerHeldAndReportsWhatItDid() {
        val host = host()
        val held = List(3) { host.openRead(binding, context).await() }

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))

        assertTrue(disposal.isComplete)
        assertTrue(disposal.failures.toString(), disposal.failures.isEmpty())
        assertEquals(held.map { it.handle }, disposal.retirements.map { it.handle })
        assertEquals(List(3) { true }, disposal.retirements.map { it.retired })
        assertEquals(held.map { it.handle }, retiredHandles())
        for (request in library.requests("closeSession")) {
            assertEquals(
                EngineFixtures.request("close").getValue("binding"),
                request.getValue("binding"),
            )
        }

        host.close()
        assertSame(disposal, host.awaitDisposal(TIMEOUT_MILLIS))
        assertEquals(3, retiredHandles().size)
    }

    @Test
    fun aRetirementThatFailsNeitherHidesTheOthersNorItself() {
        val host = host()
        val held = List(3) { host.openRead(binding, context).await() }
        val failing = held[1].handle
        library.reply = { entry, request ->
            if (entry == "closeSession" && library.handleOf(request) == failing) {
                null
            } else {
                library.canonical(entry, request)
            }
        }

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))

        assertFalse(disposal.isComplete)
        assertEquals(held.map { it.handle }, disposal.retirements.map { it.handle })
        assertEquals(listOf(true, false, true), disposal.retirements.map { it.retired })
        assertEquals(1, disposal.failures.size)
        assertEquals(
            EngineFault.NO_ANSWER,
            (disposal.failures.single() as EngineContractException).fault,
        )
        assertEquals(held.map { it.handle }, retiredHandles())
    }

    @Test
    fun aRetirementOfAnotherSessionIsRefusedAndARepeatRetiresNothing() {
        val outside =
            listOf(
                "bad_closed_other_handle",
                "bad_closed_other_binding",
                "bad_closed_repeat_binding",
            )

        for (key in outside) {
            val posed = FakeNativeLibrary()
            posed.reply = { entry, request ->
                if (entry == "closeSession") {
                    posed.response(key)
                } else {
                    posed.canonical(entry, request)
                }
            }
            val session = host(posed).openRead(binding, context).await()

            val failure =
                assertThrows(key, EngineContractException::class.java) { session.retire().await() }

            assertEquals(key, EngineFault.FOREIGN_RETIREMENT, failure.fault)
        }

        val repeating = FakeNativeLibrary()
        repeating.reply = { entry, request ->
            if (entry == "closeSession") {
                repeating.response("closed_repeat")
            } else {
                repeating.canonical(entry, request)
            }
        }
        val session = host(repeating).openRead(binding, context).await()

        assertFalse(session.retire().await())
    }

    @Test
    fun retiringOneSessionTwiceRetiresItNativelyOnce() {
        val host = host()
        val retired = host.openRead(binding, context).await()
        val kept = host.openRead(binding, context).await()

        val ordered = retired.retire()
        assertTrue(ordered.await())
        assertSame(ordered, retired.retire())
        assertTrue(retired.retire().await())

        assertEquals(listOf(retired.handle), retiredHandles())
        assertThrows(EngineClosedException::class.java) { retired.answer(ReadOperation.Status) }
        assertEquals(answered("answered_status"), kept.answer(ReadOperation.Status).await())

        host.close()
        assertTrue(requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS)).isComplete)
        assertEquals(listOf(retired.handle, kept.handle), retiredHandles())
    }

    @Test
    fun aRetirementTheLibraryDidNotAccountForIsHeldAndReportedAsItWent() {
        val host = host()
        val unaccounted = host.openRead(binding, context).await()
        val kept = host.openRead(binding, context).await()
        library.reply = { entry, request ->
            if (entry == "closeSession" && library.handleOf(request) == unaccounted.handle) {
                null
            } else {
                library.canonical(entry, request)
            }
        }

        val ordered = unaccounted.retire()
        val failure = assertThrows(EngineContractException::class.java) { ordered.await() }

        assertEquals(EngineFault.NO_ANSWER, failure.fault)
        assertSame(ordered, unaccounted.retire())
        assertSame(
            failure,
            assertThrows(EngineContractException::class.java) { unaccounted.retire().await() },
        )
        assertEquals(answered("answered_status"), kept.answer(ReadOperation.Status).await())

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))

        assertFalse(disposal.isComplete)
        assertEquals(
            listOf(unaccounted.handle, kept.handle),
            disposal.retirements.map { it.handle },
        )
        assertEquals(listOf(false, true), disposal.retirements.map { it.retired })
        assertSame(failure, disposal.failures.single())
        assertEquals(listOf(unaccounted.handle, kept.handle), retiredHandles())
    }

    @Test
    fun aRetirementTheLibraryHasNotSettledIsNeitherForgottenNorOrderedAgain() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val arrival = held()

        val ordered = session.retire()
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        assertSame(ordered, session.retire())
        assertNull(ordered.await(BRIEF_MILLIS))
        library.gate?.countDown()

        assertTrue(requireNotNull(ordered.await(TIMEOUT_MILLIS)))
        assertEquals(listOf(session.handle), retiredHandles())

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))

        assertTrue(disposal.isComplete)
        assertTrue(disposal.retirements.toString(), disposal.retirements.isEmpty())
        assertEquals(listOf(session.handle), retiredHandles())
    }

    @Test
    fun aRetirementAnsweredForAnotherSessionCertifiesNothingOnDisposal() {
        val posed = FakeNativeLibrary()
        posed.reply = { entry, request ->
            if (entry == "closeSession") {
                posed.response("bad_closed_other_handle")
            } else {
                posed.canonical(entry, request)
            }
        }
        val host = host(posed)
        val session = host.openRead(binding, context).await()

        val failure =
            assertThrows(EngineContractException::class.java) { session.retire().await() }

        assertEquals(EngineFault.FOREIGN_RETIREMENT, failure.fault)

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        val retirement = disposal.retirements.single()

        assertFalse(disposal.isComplete)
        assertEquals(session.handle, retirement.handle)
        assertFalse(retirement.retired)
        assertSame(failure, retirement.failure)
        assertEquals(1, posed.requests("closeSession").size)
    }

    @Test
    fun aConcurrentRetirementAndDisposalRetireOneSessionOnce() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val start = CountDownLatch(1)
        val racing =
            List(4) { racer ->
                Thread {
                    start.await()
                    if (racer % 2 == 0) session.retire() else host.close()
                }
            }

        racing.forEach { it.start() }
        start.countDown()
        racing.forEach { it.join(TIMEOUT_MILLIS) }

        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        assertEquals(listOf(session.handle), retiredHandles())
        assertFalse(session.isOpen)
        assertTrue(disposal.failures.toString(), disposal.failures.isEmpty())
        assertTrue(disposal.isComplete)
        assertTrue(
            disposal.retirements.toString(),
            disposal.retirements.all { it.handle == session.handle && it.retired },
        )
    }

    @Test
    fun disposalDoesNotBlockOnTheCallersThread() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val arrival = held()
        val blocked = session.answer(ReadOperation.Status)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))

        host.close()

        assertNull(host.awaitDisposal(BRIEF_MILLIS))
        library.gate?.countDown()
        blocked.await()
        assertTrue(requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS)).isComplete)
    }

    @Test
    fun aSessionOpenedIntoADisposedOwnerIsRetiredRatherThanHandedOut() {
        val host = host()
        val arrival = held()

        val opening = host.openRead(binding, context)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        host.close()
        library.gate?.countDown()

        val closed = assertThrows(EngineClosedException::class.java) { opening.await() }
        assertEquals(0, closed.suppressed.size)
        assertEquals(1, retiredHandles().size)
        assertTrue(requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS)).retirements.isEmpty())
    }

    @Test
    fun aFailedRetirementOfSuchASessionIsPreservedBesideIt() {
        val host = host()
        val arrival = held()

        val opening = host.openRead(binding, context)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        host.close()
        library.reply = { entry, request ->
            if (entry == "closeSession") null else library.canonical(entry, request)
        }
        library.gate?.countDown()

        val closed = assertThrows(EngineClosedException::class.java) { opening.await() }
        assertEquals(1, closed.suppressed.size)
        assertEquals(
            EngineFault.NO_ANSWER,
            (closed.suppressed.single() as EngineContractException).fault,
        )
        assertEquals(1, retiredHandles().size)
    }

    @Test
    fun aSessionOpenedForAnotherCapabilityOrBindingIsRetiredAndTheRefusalKept() {
        val host = host()
        val kept = host.openRead(binding, context).await()

        for (substitution in openings()) {
            library.reply = { entry, request ->
                val answer = library.canonical(entry, request)
                if (entry != "openRead") {
                    answer
                } else {
                    library.encoded(JsonObject(document(answer) + substitution))
                }
            }
            val refusal =
                assertThrows(substitution.first, EngineContractException::class.java) {
                    host.openRead(binding, context).await()
                }
            assertEquals(substitution.first, EngineFault.FOREIGN_SESSION, refusal.fault)
            assertEquals(substitution.first, 0, refusal.suppressed.size)
        }

        val discarded = retiredHandles()
        assertEquals(openings().size, discarded.size)
        assertFalse(kept.handle in discarded)

        library.reply = { entry, request -> library.canonical(entry, request) }
        val opened =
            List(host.contract.limits.maxOpenSessions - 1) {
                host.openRead(binding, context).await()
            }
        for (session in opened) {
            assertEquals(answered("answered_status"), session.answer(ReadOperation.Status).await())
        }
        assertEquals(answered("answered_status"), kept.answer(ReadOperation.Status).await())

        host.close()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        assertEquals(host.contract.limits.maxOpenSessions, disposal.retirements.size)
        assertTrue(disposal.isComplete)
    }

    @Test
    fun aFailedOpenRetirementIsPreservedBesideTheRefusal() {
        val host = host()
        library.reply = { entry, request ->
            when (entry) {
                "openRead" ->
                    library.encoded(
                        JsonObject(
                            document(library.canonical(entry, request)) +
                                ("binding" to otherGeneration()),
                        ),
                    )

                "closeSession" -> null
                else -> library.canonical(entry, request)
            }
        }

        val refusal =
            assertThrows(EngineContractException::class.java) {
                host.openRead(binding, context).await()
            }

        assertEquals(EngineFault.FOREIGN_SESSION, refusal.fault)
        assertEquals(1, refusal.suppressed.size)
        assertEquals(
            EngineFault.NO_ANSWER,
            (refusal.suppressed.single() as EngineContractException).fault,
        )
        assertEquals(1, retiredHandles().size)
    }

    @Test
    fun aCallbackIsDeliveredOffTheCallersThreadAndRetiredWithItsSession() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val heard = Heard()

        session.answer(ReadOperation.Status, heard).await()

        assertEquals(listOf(answered("answered_status")), heard.answers.toList())
        assertFalse(Thread.currentThread().name in heard.threads)

        val arrival = held()
        val late = session.answer(ReadOperation.IndexedFiles, heard)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        session.retire()
        library.gate?.countDown()

        assertEquals(answered("answered_indexed_files"), late.await())
        assertEquals(1, heard.answers.size)
        assertTrue(heard.failures.toString(), heard.failures.isEmpty())
    }

    @Test
    fun aCallbackQueuedLateHearsNothingOnceTheSessionOrTheOwnerCloses() {
        val retiring = Heard()
        val session = host().openRead(binding, context).await()
        val retirement = held()
        val failing = session.answer(ReadOperation.Status, retiring)
        assertTrue(retirement.await(TIMEOUT_MILLIS, MILLISECONDS))
        session.retire()
        library.reply = { _, _ -> null }
        library.gate?.countDown()

        assertEquals(
            EngineFault.NO_ANSWER,
            assertThrows(EngineContractException::class.java) { failing.await() }.fault,
        )
        assertTrue(retiring.failures.toString(), retiring.failures.isEmpty())
        assertTrue(retiring.answers.toString(), retiring.answers.isEmpty())

        val disposing = Heard()
        library.reply = { entry, request -> library.canonical(entry, request) }
        val host = host()
        val disposed = host.openRead(binding, context).await()
        val arrival = held()
        val late = disposed.answer(ReadOperation.IndexedFiles, disposing)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        host.close()
        library.gate?.countDown()

        assertEquals(answered("answered_indexed_files"), late.await())
        assertTrue(disposing.answers.toString(), disposing.answers.isEmpty())
        assertTrue(disposing.failures.toString(), disposing.failures.isEmpty())
    }

    @Test
    fun aCallbackThatFailsDoesNotMaskTheFailureItWasToldAbout() {
        val host = host()
        val session = host.openRead(binding, context).await()
        val callback = Failing()
        library.reply = { _, _ -> null }

        val failure =
            assertThrows(EngineContractException::class.java) {
                session.answer(ReadOperation.Status, callback).await()
            }

        assertEquals(EngineFault.NO_ANSWER, failure.fault)
        assertEquals(
            EngineSecondaryFault.FAILURE_CALLBACK,
            (failure.suppressed.single() as EngineSecondaryException).fault,
        )
        assertCarriesNothingBuiltOutside("a failure a callback could not report", failure)

        library.reply = { entry, request -> library.canonical(entry, request) }
        val reported =
            assertThrows(EngineSecondaryException::class.java) {
                session.answer(ReadOperation.Status, callback).await()
            }

        assertEquals(EngineSecondaryFault.ANSWER_CALLBACK, reported.fault)
        assertCarriesNothingBuiltOutside("an answer a callback could not take", reported)

        val arrival = held()
        val late = session.answer(ReadOperation.IndexedFiles, callback)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        session.retire()
        library.gate?.countDown()

        assertEquals(answered("answered_indexed_files"), late.await())
    }

    @Test
    fun aRetirementFailingOutsideThisContractIsReportedAsACategoryOnly() {
        val raised = built()
        val host = host()
        val arrival = held()
        val opening = host.openRead(binding, context)
        assertTrue(arrival.await(TIMEOUT_MILLIS, MILLISECONDS))
        host.close()
        library.reply = { entry, request ->
            if (entry == "closeSession") throw raised else library.canonical(entry, request)
        }
        library.gate?.countDown()

        val discarded = assertThrows(EngineClosedException::class.java) { opening.await() }

        assertEquals(
            EngineSecondaryFault.RETIREMENT,
            (discarded.suppressed.single() as EngineSecondaryException).fault,
        )
        assertCarriesNothingBuiltOutside("a discarded session", discarded)

        // Primary seam exceptions remain unchanged; disposal uses a safe category.
        val ordering = host()
        val session = ordering.openRead(binding, context).await()
        assertSame(
            raised,
            assertThrows(IllegalStateException::class.java) { session.retire().await() },
        )

        ordering.close()
        val disposal = requireNotNull(ordering.awaitDisposal(TIMEOUT_MILLIS))
        val retirement = disposal.retirements.single()

        assertFalse(disposal.isComplete)
        assertFalse(retirement.retired)
        assertEquals(
            EngineSecondaryFault.RETIREMENT,
            (retirement.failure as EngineSecondaryException).fault,
        )
        assertCarriesNothingBuiltOutside("a retirement nobody took", disposal.failures.single())
    }

    private fun host(seam: NativeSeam = library): SlipboxEngineHost =
        SlipboxEngineHost.over(seam).also { hosts += it }

    private fun refusedAtConstruction(): EngineContractException =
        assertThrows(EngineContractException::class.java) { host() }

    /** Holds the next call in the library until the returned arrival is awaited. */
    private fun held(): CountDownLatch {
        val arrival = CountDownLatch(1)
        library.gate = CountDownLatch(1)
        library.arrival = arrival
        return arrival
    }

    /** The typed answer one shared response carries. */
    private fun answered(key: String): EngineAnswer =
        (EngineWire.decode(library.response(key)) as AdapterResponse.Answered).answer

    private fun requested(admitted: List<AdmittedOperation<*>>): List<JsonObject> =
        admitted.map { EngineFixtures.request(it.request) }

    private fun retiredHandles(): List<Long> =
        library.requests("closeSession").map { it.getValue("handle").jsonPrimitive.long }

    private fun substitutions(session: EngineSession): List<Pair<String, JsonElement>> =
        listOf(
            "handle" to JsonPrimitive(session.handle + 1),
            "binding" to otherGeneration(),
        )

    private fun openings(): List<Pair<String, JsonElement>> =
        listOf(
            "binding" to otherGeneration(),
            "capability" to JsonPrimitive("maintenance"),
        )

    private fun otherGeneration(): JsonObject =
        buildJsonObject {
            put("source", EngineOperations.SOURCE)
            put("generation", "fixture-03")
        }

    private fun declaring(limit: String, bound: Int) {
        val declared = library.contractDocument.getValue("limits").jsonObject
        val limits = JsonObject(declared + (limit to JsonPrimitive(bound)))
        library.contractDocument = JsonObject(library.contractDocument + ("limits" to limits))
    }

    private fun document(answer: ByteArray?): JsonObject =
        library.parsed(requireNotNull(answer) { "the library allocated no answer" })

    private fun assertCarriesNothingBuiltOutside(case: String, failure: Throwable) {
        val written = StringWriter()
        failure.printStackTrace(PrintWriter(written))
        val surfaced =
            graph(failure).flatMap { listOf(it.message.orEmpty(), it.toString()) } +
                written.toString()

        for (route in surfaced) {
            for (said in listOf(SAID, UNDERNEATH, BESIDE)) {
                assertFalse(case, said in route)
            }
        }
        for (carried in graph(failure)) {
            assertTrue(
                case,
                carried is EngineContractException ||
                    carried is EngineRefusedException ||
                    carried is EngineClosedException ||
                    carried is EngineSecondaryException,
            )
        }
    }

    private fun graph(failure: Throwable): List<Throwable> =
        listOf(failure) +
            failure.suppressed.flatMap { graph(it) } +
            (failure.cause?.let { graph(it) } ?: emptyList())

    private companion object {

        /** Long enough to settle a disposal that is not waiting on a native call. */
        const val BRIEF_MILLIS = 100L
    }
}
