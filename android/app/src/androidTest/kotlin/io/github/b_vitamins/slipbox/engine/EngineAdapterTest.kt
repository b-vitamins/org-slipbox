/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import android.os.Looper
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit.MILLISECONDS

/** Adapter integration tests against the packaged library. */
@RunWith(AndroidJUnit4::class)
class EngineAdapterTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val seam = SlipboxNativeEngine.seam
    private val workspace = File(context.noBackupFilesDir, "engine-adapter-${System.nanoTime()}")
    private val corpus = File(workspace, "corpus")
    private val database = File(workspace, "slipbox.db")
    private val binding = GenerationBinding(SOURCE, "device-01")
    private val hosts = mutableListOf<SlipboxEngineHost>()
    private val releases = mutableListOf<CountDownLatch>()
    private val exercised = mutableSetOf<String>()
    private val carried = mutableSetOf<String>()

    @Before
    fun writeSyntheticCorpus() {
        assertTrue("$corpus is not available", corpus.mkdirs())
        for ((name, text) in CORPUS) {
            File(corpus, name).writeText(text)
        }
    }

    @After
    fun disposeHostsAndCorpus() {
        releases.forEach { it.countDown() }
        for (host in hosts) {
            host.close()
            val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS)) { "no disposal" }
            assertTrue(disposal.failures.toString(), disposal.failures.isEmpty())
        }
        assertTrue("$workspace survived the test", workspace.deleteRecursively())
    }

    @Test
    fun thePackagedLibraryDeclaresTheContractThisMirrorSpeaks() {
        val declared = requireNotNull(seam.contract()) { "the library declared no contract" }

        assertEquals(EngineFixtures.response("contract"), parsed(declared))
        val host = host()
        assertEquals(EngineWire.readOperations, host.contract.readOperations.toSet())
        assertEquals(EngineWire.maintenanceOperations, host.contract.maintenanceOperations.toSet())
        assertEquals(ADMITTED_LIMITS, host.contract.limits)
    }

    @Test
    fun everyReadingOperationAnswersItsCanonicalTypeOnDevice() {
        val read = indexedRead(host())

        val status = answered<EngineAnswer.Status>(read, ReadOperation.Status).result
        assertEquals(3L, status.filesIndexed)
        assertEquals(5L, status.nodesIndexed)
        assertEquals(1L, status.linksIndexed)
        assertTrue("$status", status.notesIndexed in 1..status.nodesIndexed)
        assertTrue(status.version, status.version.isNotBlank())
        assertEquals(corpus.canonicalPath, File(status.root).canonicalPath)
        assertEquals(database.canonicalPath, File(status.db).canonicalPath)

        assertEquals(CORPUS.map { it.first }, files(read))

        val beta = note(read, "beta-target")
        val alpha = note(read, "alpha-first")
        val riemann = note(read, "riemann-integral")
        assertEquals("Target heading", beta.title)
        assertEquals("beta.org", beta.filePath)
        assertEquals(NodeKind.HEADING, beta.kind)
        // The same record through the other lookup, field for field.
        val again = answered<EngineAnswer.NodeFromKey>(read, ReadOperation.NodeFromKey(beta.nodeKey))
        assertEquals(beta, again.result)
        val absent = answered<EngineAnswer.NodeFromId>(read, ReadOperation.NodeFromId("no-such"))
        assertNull(absent.result)

        assertEquals(listOf("Target heading"), titles(search(read, "target")))
        val content = ReadOperation.SearchNodeContent("Target body", 10)
        val hits = answered<EngineAnswer.SearchNodeContent>(read, content).result.hits
        assertEquals(listOf(beta.nodeKey), hits.map { it.node.nodeKey })
        val segments = hits.single().snippet.segments
        assertTrue("$segments", segments.any { it.matched })

        val whole = ReadOperation.ReadNodeSource(beta.nodeKey, 0, 0, 1_000)
        val source = answered<EngineAnswer.ReadNodeSource>(read, whole).result
        assertEquals(beta.nodeKey, source.anchor.nodeKey)
        assertEquals("beta.org", source.source.filePath)
        assertTrue(source.source.content, source.source.content.contains("Target body."))
        assertFalse(source.source.truncatedBefore)
        assertFalse(source.source.truncatedAfter)
        assertTrue("$source", source.source.lineCount in 1..source.source.totalLines)
        assertTrue("$source", source.nodeLineCount in 1..source.source.lineCount)

        val listing = ReadOperation.ListGlossaryTerms(50, null)
        val listed = answered<EngineAnswer.ListGlossaryTerms>(read, listing).result
        assertEquals(listOf("Riemann integral"), titles(listed.terms))
        assertEquals(1L, listed.total)
        assertFalse(listed.hasMore)
        assertNull(listed.nextPosition)
        val glossary = ReadOperation.SearchGlossary("Riemann", 10)
        val searched = answered<EngineAnswer.SearchGlossary>(read, glossary).result
        assertEquals(listOf("Riemann integral"), titles(searched.terms))
        assertFalse(searched.hasMore)
        val lookup = ReadOperation.GlossaryTerm(riemann.nodeKey)
        val term = answered<EngineAnswer.GlossaryTerm>(read, lookup).result.term
        assertEquals(riemann, term)
        assertTrue("$term", requireNotNull(term).glossary)
        assertEquals("confirmed", term.glossaryStatus)

        val incoming = ReadOperation.Backlinks(beta.nodeKey, 20, true)
        val backlink = answered<EngineAnswer.Backlinks>(read, incoming).result.backlinks.single()
        assertEquals(alpha.nodeKey, backlink.sourceNote.nodeKey)
        assertEquals(ExplorationExplanation.Backlink, backlink.explanation)
        assertTrue(backlink.preview, backlink.preview.isNotBlank())
        val outgoing = ReadOperation.ForwardLinks(alpha.nodeKey, 20, false)
        val forward = answered<EngineAnswer.ForwardLinks>(read, outgoing).result
        val destination = forward.forwardLinks.single()
        assertEquals(beta.nodeKey, destination.destinationNote.nodeKey)
        assertEquals(ExplorationExplanation.ForwardLink, destination.explanation)

        val lens = ReadOperation.Explore(beta.nodeKey, ExplorationLens.STRUCTURE, 20, true)
        val exploration = answered<EngineAnswer.Explore>(read, lens).result
        assertEquals(ExplorationLens.STRUCTURE, exploration.lens)
        val kinds = exploration.sections.map { it.kind }
        assertTrue("$kinds", kinds.all { it in STRUCTURE_SECTIONS })
        val section = exploration.sections.single { it.kind == ExplorationSectionKind.BACKLINKS }
        val entry = typed<ExplorationEntry.Backlink>(section.entries.single())
        assertEquals(alpha.nodeKey, entry.record.sourceNote.nodeKey)
        assertEquals(ExplorationExplanation.Backlink, entry.record.explanation)

        assertEquals(EngineWire.readOperations, exercised)
    }

    @Test
    fun aFileIsRefreshedOrRemovedOnDemandAndNoReadingScansTheCorpus() {
        val host = host()
        val maintenance = indexer(host)
        val read = host.openRead(binding, sessionContext()).settled()
        assertEquals(emptyList<String>(), titles(search(read, "fresh")))

        File(corpus, "beta.org").appendText(FRESH)
        // A reading session answers the index, so an unindexed edit is invisible
        // to it until maintenance takes it in.
        assertEquals(emptyList<String>(), titles(search(read, "fresh")))

        val refresh = MaintenanceOperation.IndexFile("beta.org")
        val refreshed = carriedOut<EngineAnswer.IndexFile>(maintenance, refresh).result
        assertEquals("beta.org", refreshed.filePath)
        assertEquals(listOf("Fresh heading"), titles(search(read, "fresh")))

        assertTrue(File(corpus, "riemann.org").delete())
        val removal = MaintenanceOperation.IndexFile("riemann.org")
        carriedOut<EngineAnswer.IndexFile>(maintenance, removal)
        assertEquals(listOf("alpha.org", "beta.org"), files(read))
        val listing = ReadOperation.ListGlossaryTerms(50, null)
        val listed = answered<EngineAnswer.ListGlossaryTerms>(read, listing).result
        assertEquals(emptyList<String>(), titles(listed.terms))
        assertEquals(EngineWire.maintenanceOperations, carried)
    }

    @Test
    fun anAnswerNamesTheBindingItsHandleRetainsRatherThanTheRequestField() {
        val host = host()
        val other = GenerationBinding(SOURCE, "device-02")
        val elsewhere = GenerationBinding(OTHER_SOURCE, "device-01")
        val otherDatabase = File(workspace, "other.db").absolutePath
        val first = host.openRead(binding, sessionContext()).settled()
        val second =
            host.openRead(other, SessionContext(corpus.absolutePath, otherDatabase)).settled()
        assertNotEquals(first.handle, second.handle)

        for (session in listOf(first, second)) {
            val request = ReadRequest(session.handle, session.binding, ReadOperation.Status)
            val answer = answeredRaw(request)
            assertEquals(session.handle, answer.handle)
            assertEquals(session.binding, answer.binding)
        }
        for (substituted in listOf(other, elsewhere)) {
            val refusal = refusedRead(ReadRequest(first.handle, substituted, ReadOperation.Status))
            assertEquals(RefusalReason.BINDING_MISMATCH, refusal.reason)
        }
        // A refused binding retires nothing: the session it named still answers.
        assertNotNull(answered<EngineAnswer.Status>(first, ReadOperation.Status))
    }

    @Test
    fun aHandleOutsideTheTableOrItsCapabilityIsRefused() {
        val host = host()
        val read = host.openRead(binding, sessionContext()).settled()
        val maintenance = host.openMaintenance(binding, sessionContext()).settled()
        val retired = host.openRead(binding, sessionContext()).settled()
        assertEquals(true, retired.retire().settled())

        for (unknown in listOf(0L, retired.handle + 1_000L, Long.MAX_VALUE)) {
            val refusal = refusedRead(ReadRequest(unknown, binding, ReadOperation.Status))
            assertEquals("$unknown", RefusalReason.UNKNOWN_HANDLE, refusal.reason)
        }
        assertEquals(
            RefusalReason.RETIRED_HANDLE,
            refusedRead(ReadRequest(retired.handle, binding, ReadOperation.Status)).reason,
        )
        assertEquals(
            RefusalReason.CAPABILITY_MISMATCH,
            refusedRead(ReadRequest(maintenance.handle, binding, ReadOperation.Status)).reason,
        )
        val crossed = MaintenanceRequest(read.handle, binding, MaintenanceOperation.Index)
        assertEquals(RefusalReason.CAPABILITY_MISMATCH, refusedMaintenance(crossed).reason)

        // The table reports the retirement once and refuses an identity it never
        // allocated, so no answer can reach a session other than the one asked.
        val repeat = closed(retired.handle)
        assertFalse(repeat.retired)
        assertNull(repeat.binding)
        assertEquals(RefusalReason.UNKNOWN_HANDLE, refusedClose(Long.MAX_VALUE).reason)
    }

    @Test
    fun everySlotIsOpenedAndASessionBeyondThemIsRefused() {
        val host = host()
        val slots = host.contract.limits.maxOpenSessions
        val held = List(slots) { host.openRead(binding, sessionContext()).settled() }
        assertEquals(slots, held.map { it.handle }.toSet().size)

        val refusal =
            assertThrows(EngineRefusedException::class.java) {
                host.openRead(binding, sessionContext()).settled()
            }

        assertEquals(RefusalReason.SESSIONS_EXHAUSTED, refusal.refusal.reason)
        assertEquals(AdapterBound.OPEN_SESSIONS, refusal.refusal.bound)
        // The refused open took no slot, and a retirement returns the one it held.
        assertEquals(true, held.first().retire().settled())
        val reopened = host.openRead(binding, sessionContext()).settled()
        assertNotEquals(held.first().handle, reopened.handle)
        assertNotNull(answered<EngineAnswer.Status>(reopened, ReadOperation.Status))
        assertNotNull(answered<EngineAnswer.Status>(held.last(), ReadOperation.Status))
    }

    @Test
    fun aDocumentOutsideTheContractIsRefusedAtTheBoundary() {
        val bounds = host().contract.limits
        val versionless = """{"handle":1,"operation":{"kind":"status"}}"""
        val malformed =
            mapOf(
                "empty" to ByteArray(0),
                "truncated" to "{".toByteArray(Charsets.UTF_8),
                "not-utf8" to byteArrayOf(0x7B, 0xC3.toByte(), 0x22),
                "no-version" to versionless.toByteArray(Charsets.UTF_8),
                // At the bound the document is read rather than refused for length.
                "at-the-bound" to ByteArray(bounds.maxRequestBytes),
            )
        for ((name, document) in malformed) {
            assertEquals(name, RefusalReason.MALFORMED_REQUEST, refused(seam.read(document)).reason)
        }

        for (version in listOf(0, 2, 99)) {
            val refusal = refused(seam.read(operation(1, "status", version = version)))
            assertEquals("version $version", RefusalReason.UNSUPPORTED_VERSION, refusal.reason)
        }

        // Duplicate-key refusal precedes version validation.
        val declared = operation(1, "status", version = 2).decodeToString()
        val source = "\"source\":\"${binding.source}\""
        val ambiguous =
            mapOf(
                "version" to declared.replace("\"version\":2", "\"version\":2,\"version\":1"),
                "reversed version" to
                    declared.replace("\"version\":2", "\"version\":1,\"version\":2"),
                "escaped version" to
                    declared.replace("\"version\":2", "\"\\u0076ersion\":1,\"version\":2"),
                "handle" to declared.replace("\"handle\":1", "\"handle\":1,\"handle\":2"),
                "binding source" to declared.replace(source, "$source,$source"),
                "operation kind" to
                    declared.replace(
                        "\"kind\":\"status\"",
                        "\"kind\":\"status\",\"kind\":\"indexedFiles\"",
                    ),
            )
        for ((name, document) in ambiguous) {
            val refusal = refused(seam.read(document.toByteArray(Charsets.UTF_8)))
            assertEquals(name, RefusalReason.MALFORMED_REQUEST, refusal.reason)
        }

        val oversize = refused(seam.read(ByteArray(bounds.maxRequestBytes + 1)))
        assertEquals(RefusalReason.OUT_OF_BOUNDS, oversize.reason)
        assertEquals(AdapterBound.REQUEST_BYTES, oversize.bound)
    }

    @Test
    fun aNullArgumentIsRefusedAtEveryExportedEntry() {
        for (entry in ADAPTER_ENTRIES) {
            val refusal = refused(invoked(entry, null))
            assertEquals(entry, RefusalReason.MALFORMED_REQUEST, refusal.reason)
        }

        val absent = aborted(null)
        val empty = aborted(ByteArray(0))
        assertEquals("argument", absent.getValue("stage").jsonPrimitive.content)
        assertEquals("argument", empty.getValue("stage").jsonPrimitive.content)
        // Two refusals of their own: the non-null array was read, not assumed.
        assertNotEquals(absent.getValue("detail"), empty.getValue("detail"))
        val declared = SlipboxNativeEngine::class.java
            .getDeclaredMethod("nativeAdapterContract")
            .apply { isAccessible = true }
            .invoke(SlipboxNativeEngine) as ByteArray?
        assertEquals(EngineFixtures.response("contract"), parsed(requireNotNull(declared)))
    }

    @Test
    fun aNoteTheEngineCutShortIsRefusedRatherThanTruncated() {
        val read = indexedRead(host())
        val beta = note(read, "beta-target").nodeKey

        val refusal =
            assertThrows(EngineRefusedException::class.java) {
                read.answer(ReadOperation.ReadNodeSource(beta, 0, 0, 1)).settled()
            }

        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.NOTE_SOURCE_LINES, refusal.refusal.bound)
    }

    @Test
    fun anEngineRefusalRepeatsNothingOfTheRequest() {
        val read = indexedRead(host())
        val absent = ReadOperation.ReadNodeSource(ABSENT_KEY, 0, 0, 1_000)
        val request = ReadRequest(read.handle, binding, absent)

        val answer =
            requireNotNull(seam.read(EngineWire.encode(request))) {
                "the library allocated no answer"
            }
        val refusal = refused(answer)

        assertEquals(RefusalReason.ENGINE_REFUSED, refusal.reason)
        assertEquals(EngineRefusalKind.NOT_FOUND, refusal.engine?.kind)
        assertNotNull(refusal.engine?.code)
        // The engine's prose quotes the key it was asked for; the refusal carries
        // neither the prose nor the key.
        assertFalse(answer.decodeToString(), answer.decodeToString().contains(ABSENT_TOKEN))
        val reported =
            assertThrows(EngineRefusedException::class.java) { read.answer(absent).settled() }
        assertFalse(reported.message.orEmpty(), reported.message.orEmpty().contains(ABSENT_TOKEN))
        assertFalse(reported.toString(), reported.toString().contains(ABSENT_TOKEN))
        assertNull(reported.cause)
    }

    @Test
    fun neitherCapabilityAdmitsAWriteOrAnUnknownOperation() {
        val host = host()
        val read = host.openRead(binding, sessionContext()).settled()
        val maintenance = host.openMaintenance(binding, sessionContext()).settled()

        for (denied in DENIED + EngineWire.maintenanceOperations) {
            val refusal = refused(seam.read(operation(read.handle, denied)))
            assertEquals(denied, RefusalReason.UNKNOWN_OPERATION, refusal.reason)
        }
        for (denied in DENIED + EngineWire.readOperations) {
            val refusal = refused(seam.maintain(operation(maintenance.handle, denied)))
            assertEquals(denied, RefusalReason.UNKNOWN_OPERATION, refusal.reason)
        }
    }

    @Test
    fun aCallSubmittedFromTheMainThreadCrossesOffIt() {
        val read = indexedRead(host())
        val heard = Heard()

        val main = Looper.getMainLooper().thread.name
        instrumentation.runOnMainSync { read.answer(ReadOperation.IndexedFiles, heard) }

        assertTrue(heard.awaitSettled())
        assertTrue(heard.failures.toString(), heard.failures.isEmpty())
        val listed = typed<EngineAnswer.IndexedFiles>(heard.answers.single()).result
        assertEquals(CORPUS.map { it.first }, listed.files)
        assertNotEquals(main, heard.threads.single())
        assertNotEquals(Thread.currentThread().name, heard.threads.single())
    }

    @Test
    fun aQueueBeyondTheDeclaredDepthIsRefusedOnDevice() {
        val host = host()
        val read = indexedRead(host)
        val release = holding(read)

        val queued = (2..host.contract.limits.maxQueuedRequests).map { read.answer(STATUS) }

        val refusal =
            assertThrows(EngineRefusedException::class.java) { read.answer(STATUS) }
        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.QUEUED_REQUESTS, refusal.refusal.bound)
        release.countDown()
        for (call in queued) {
            assertNotNull(call.settled())
        }
    }

    @Test
    fun abandonedCallsBeyondThePhysicalQueueNeitherRejectNorLeakAdmission() {
        val read = indexedRead(host())
        val release = holding(read)

        repeat(CHURN) { round ->
            val abandoned = read.answer(STATUS)
            assertEquals("$round", EngineCancellation.DISCARDED, abandoned.cancel())
        }

        val queued = read.answer(ReadOperation.IndexedFiles)
        release.countDown()
        assertNotNull(queued.settled())
    }

    @Test
    fun disposalOnTheMainThreadDoesNotWaitForTheWorkItOrders() {
        val host = host()
        val read = indexedRead(host)
        val release = holding(read)

        instrumentation.runOnMainSync { host.close() }

        assertNull(host.awaitDisposal(BRIEF_MILLIS))
        release.countDown()
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        assertEquals(listOf(read.handle), disposal.retirements.map { it.handle })
        assertTrue(disposal.failures.toString(), disposal.isComplete)
    }

    @Test
    fun disposalThroughTheLifecycleRetiresEverySessionAndRejectsFurtherWork() {
        val host = host()
        val held = List(2) { host.openRead(binding, sessionContext()).settled() }

        instrumentation.runOnMainSync {
            val owner = RecordingOwner()
            owner.registry.addObserver(host)
            owner.registry.currentState = Lifecycle.State.CREATED
            owner.registry.currentState = Lifecycle.State.DESTROYED
        }

        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        assertFalse(host.isOpen)
        assertEquals(held.map { it.handle }, disposal.retirements.map { it.handle })
        assertTrue(disposal.failures.toString(), disposal.isComplete)
        for (session in held) {
            assertFalse(session.isOpen)
            // The library, not only the host, holds the session retired.
            assertFalse(closed(session.handle).retired)
            assertEquals(
                RefusalReason.RETIRED_HANDLE,
                refusedRead(ReadRequest(session.handle, binding, ReadOperation.Status)).reason,
            )
            assertThrows(EngineClosedException::class.java) { session.answer(STATUS) }
        }
        assertThrows(EngineClosedException::class.java) {
            host.openRead(binding, sessionContext())
        }
        assertThrows(EngineClosedException::class.java) {
            host.openMaintenance(binding, sessionContext())
        }
        host.close()
        assertSame(disposal, host.awaitDisposal(TIMEOUT_MILLIS))
    }

    @Test
    fun aConcurrentRetirementAndDisposalRetireOneSessionOnce() {
        val host = host()
        val session = host.openRead(binding, sessionContext()).settled()
        val start = CountDownLatch(1)
        val retirements = CopyOnWriteArrayList<Boolean>()
        val racers =
            listOf(
                Thread {
                    start.await()
                    retirements += session.retire().settled()
                },
                Thread {
                    start.await()
                    host.close()
                },
            )

        racers.forEach { it.start() }
        start.countDown()
        racers.forEach { it.join(TIMEOUT_MILLIS) }

        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        val ordered = retirements.single()
        // Caller and disposal may both report the same native retirement.
        assertTrue(
            "$ordered ${disposal.retirements}",
            ordered || disposal.retirements.any { it.retired },
        )
        assertTrue(
            disposal.retirements.toString(),
            disposal.retirements.all { it.handle == session.handle && it.retired },
        )
        assertTrue(disposal.failures.toString(), disposal.failures.isEmpty())
        assertFalse(closed(session.handle).retired)
    }

    @Test
    fun aCallbackQueuedLateHearsNothingOnceTheSessionIsRetired() {
        val read = indexedRead(host())
        val release = holding(read)
        val heard = Heard()

        val late = read.answer(ReadOperation.IndexedFiles, heard)
        val retirement = read.retire()

        release.countDown()
        assertNotNull(late.settled())
        assertEquals(true, retirement.settled())
        assertEquals(emptyList<ReadAnswer>(), heard.answers.toList())
        assertEquals(emptyList<Throwable>(), heard.failures.toList())
    }

    @Test
    fun aCallbackQueuedLateHearsNothingOnceTheOwnerCloses() {
        val host = host()
        val read = indexedRead(host)
        val release = holding(read)
        val heard = Heard()

        val late = read.answer(ReadOperation.IndexedFiles, heard)
        host.close()

        release.countDown()
        assertNotNull(late.settled())
        val disposal = requireNotNull(host.awaitDisposal(TIMEOUT_MILLIS))
        assertTrue(disposal.failures.toString(), disposal.isComplete)
        assertEquals(emptyList<ReadAnswer>(), heard.answers.toList())
        assertEquals(emptyList<Throwable>(), heard.failures.toList())
    }

    private fun host(): SlipboxEngineHost = SlipboxEngineHost.packaged().also { hosts += it }

    private fun sessionContext(): SessionContext =
        SessionContext(corpus.absolutePath, database.absolutePath)

    /** A maintenance session that has taken the fixture corpus in. */
    private fun indexer(host: SlipboxEngineHost): EngineMaintenanceSession {
        val session = host.openMaintenance(binding, sessionContext()).settled()
        val stats = carriedOut<EngineAnswer.Index>(session, MaintenanceOperation.Index).result
        assertEquals(CORPUS.size.toLong(), stats.filesIndexed)
        return session
    }

    /** A reading session over the indexed fixture, with no maintenance of its own. */
    private fun indexedRead(host: SlipboxEngineHost): EngineReadSession {
        assertEquals(true, indexer(host).retire().settled())
        return host.openRead(binding, sessionContext()).settled()
    }

    /** Occupies the lane's one thread until the returned latch is counted down. */
    private fun holding(session: EngineReadSession): CountDownLatch {
        val arrived = CountDownLatch(1)
        val release = CountDownLatch(1).also { releases += it }
        session.answer(STATUS, HoldingCallback(arrived, release))
        assertTrue("the lane took no call", arrived.await(TIMEOUT_MILLIS, MILLISECONDS))
        return release
    }

    private inline fun <reified T : ReadAnswer> answered(
        session: EngineReadSession,
        operation: ReadOperation,
    ): T {
        exercised += operation.kind()
        return typed(session.answer(operation).settled())
    }

    private inline fun <reified T : MaintenanceAnswer> carriedOut(
        session: EngineMaintenanceSession,
        operation: MaintenanceOperation,
    ): T {
        carried += operation.kind()
        return typed(session.carryOut(operation).settled())
    }

    private fun note(session: EngineReadSession, id: String): NodeRecord {
        val answer = answered<EngineAnswer.NodeFromId>(session, ReadOperation.NodeFromId(id))
        return requireNotNull(answer.result) { "the index holds no node of that identity" }
    }

    private fun search(session: EngineReadSession, query: String): List<NodeRecord> {
        val operation = ReadOperation.SearchNodes(query, 25, SearchNodesSort.TITLE)
        return answered<EngineAnswer.SearchNodes>(session, operation).result.nodes
    }

    private fun files(session: EngineReadSession): List<String> =
        answered<EngineAnswer.IndexedFiles>(session, ReadOperation.IndexedFiles).result.files

    private fun titles(records: List<NodeRecord>): List<String> = records.map { it.title }

    /** One request document naming [kind], which no typed mirror can encode. */
    private fun operation(
        handle: Long,
        kind: String,
        version: Int = ADAPTER_PROTOCOL_VERSION,
    ): ByteArray =
        """
        {"version":$version,"handle":$handle,
         "binding":{"source":"${binding.source}","generation":"${binding.generation}"},
         "operation":{"kind":"$kind"}}
        """.trimIndent().toByteArray(Charsets.UTF_8)

    /** The failure of a probe run that reported one, through the exported entry. */
    private fun aborted(argument: ByteArray?): JsonObject {
        val document = requireNotNull(invoked(PROBE_ENTRY, argument)) { "no report" }
        val report = parsed(document)
        assertFalse(report.toString(), report.getValue("passed").jsonPrimitive.boolean)
        return report.getValue("failure").jsonObject
    }

    private fun invoked(entry: String, argument: ByteArray?): ByteArray? =
        SlipboxNativeEngine::class.java
            .getDeclaredMethod(entry, ByteArray::class.java)
            .apply { isAccessible = true }
            .invoke(SlipboxNativeEngine, argument) as ByteArray?

    private fun answeredRaw(request: ReadRequest): AdapterResponse.Answered =
        decoded(seam.read(EngineWire.encode(request)))

    private fun refusedRead(request: ReadRequest): AdapterResponse.Refused =
        refused(seam.read(EngineWire.encode(request)))

    private fun refusedMaintenance(request: MaintenanceRequest): AdapterResponse.Refused =
        refused(seam.maintain(EngineWire.encode(request)))

    private fun refusedClose(handle: Long): AdapterResponse.Refused =
        refused(seam.closeSession(EngineWire.encode(CloseRequest(handle, binding))))

    private fun closed(handle: Long): AdapterResponse.Closed =
        decoded(seam.closeSession(EngineWire.encode(CloseRequest(handle, binding))))

    private fun refused(answer: ByteArray?): AdapterResponse.Refused = decoded(answer)

    private inline fun <reified T : AdapterResponse> decoded(answer: ByteArray?): T {
        val document = requireNotNull(answer) { "the library allocated no answer" }
        return typed(EngineWire.decode(document))
    }

    private fun parsed(document: ByteArray): JsonObject =
        Json.parseToJsonElement(document.decodeToString()).jsonObject
}

private const val TIMEOUT_MILLIS = 30_000L

/** Long enough to prove a caller was not made to wait, short enough not to. */
private const val BRIEF_MILLIS = 100L

/** Far more abandoned calls than the lane's queue could ever hold at once. */
private const val CHURN = 200

private const val SOURCE = "0102030405060708090a0b0c0d0e0f10"
private const val OTHER_SOURCE = "1112131415161718191a1b1c1d1e1f20"

private const val ABSENT_TOKEN = "gramarye"
private const val ABSENT_KEY = "heading:$ABSENT_TOKEN.org:1"

private const val PROBE_ENTRY = "nativeRunFixtureProbe"

private val STATUS = ReadOperation.Status

/** Every entry point that carries one adapter request. */
private val ADAPTER_ENTRIES =
    listOf(
        "nativeOpenReadSession",
        "nativeOpenMaintenanceSession",
        "nativeReadSession",
        "nativeMaintainSession",
        "nativeCloseSession",
    )

private val STRUCTURE_SECTIONS =
    setOf(ExplorationSectionKind.BACKLINKS, ExplorationSectionKind.FORWARD_LINKS)

/** Operations neither capability admits, whatever the engine gains later. */
private val DENIED = listOf("captureNode", "rewriteFile", "gradeGlossaryTerm", "invented")

private val CORPUS =
    listOf(
        "alpha.org" to
            """
            #+title: Alpha

            * First heading
            :PROPERTIES:
            :ID: alpha-first
            :END:
            See [[id:beta-target][Beta]].
            """.trimIndent(),
        "beta.org" to
            """
            #+title: Beta

            * Target heading
            :PROPERTIES:
            :ID: beta-target
            :END:
            Target body.
            """.trimIndent(),
        "riemann.org" to
            """
            #+title: Riemann integral
            #+glossary: t
            :PROPERTIES:
            :ID: riemann-integral
            :GLOSSARY_STATUS: confirmed
            :END:

            A definite integral.
            """.trimIndent(),
    )

private val FRESH =
    "\n" +
        """
        * Fresh heading
        :PROPERTIES:
        :ID: beta-fresh
        :END:
        Fresh body.
        """.trimIndent() + "\n"

/** The answer of one call, which every call here is given time to settle. */
private fun <T> EngineTask<T>.settled(): T =
    requireNotNull(await(TIMEOUT_MILLIS)) { "the call did not settle" }

private inline fun <reified T> typed(value: Any): T {
    assertTrue("$value is no ${T::class.simpleName}", value is T)
    return value as T
}

/** What one callback was told, and on which threads. */
private class Heard : EngineCallback<ReadAnswer> {

    val answers: MutableList<ReadAnswer> = CopyOnWriteArrayList()

    val failures: MutableList<Throwable> = CopyOnWriteArrayList()

    val threads: MutableSet<String> = CopyOnWriteArraySet()

    private val settled = CountDownLatch(1)

    override fun onAnswered(answer: ReadAnswer) {
        answers += answer
        heard()
    }

    override fun onFailed(failure: Throwable) {
        failures += failure
        heard()
    }

    fun awaitSettled(): Boolean = settled.await(TIMEOUT_MILLIS, MILLISECONDS)

    private fun heard() {
        threads += Thread.currentThread().name
        settled.countDown()
    }
}

/**
 * Holds the lane's one thread inside a callback, so a queue, a cancellation or a
 * disposal on a device races work that has not finished.
 */
private class HoldingCallback(
    private val arrived: CountDownLatch,
    private val release: CountDownLatch,
) : EngineCallback<ReadAnswer> {

    override fun onAnswered(answer: ReadAnswer) = hold()

    override fun onFailed(failure: Throwable) = hold()

    private fun hold() {
        arrived.countDown()
        release.await(TIMEOUT_MILLIS, MILLISECONDS)
    }
}

/** A lifecycle of this test's own, so disposal is exercised through a real one. */
private class RecordingOwner : LifecycleOwner {

    val registry = LifecycleRegistry(this)

    override val lifecycle: Lifecycle
        get() = registry
}
