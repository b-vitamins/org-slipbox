/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import io.github.b_vitamins.slipbox.engine.ADAPTER_PROTOCOL_VERSION
import io.github.b_vitamins.slipbox.engine.AdapterBound
import io.github.b_vitamins.slipbox.engine.AdapterResponse
import io.github.b_vitamins.slipbox.engine.DocumentLinkResolution
import io.github.b_vitamins.slipbox.engine.EngineRefusal
import io.github.b_vitamins.slipbox.engine.EngineRefusalKind
import io.github.b_vitamins.slipbox.engine.EngineRefusedException
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadNodeSourceResult
import io.github.b_vitamins.slipbox.engine.RefusalReason
import io.github.b_vitamins.slipbox.engine.SourceSlice
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.ReadySourceStats
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentReaderStateTest {

    @Test
    fun aCompleteReadKeepsContentMetadataAndAssetBindingOnOneGeneration() {
        val ready = ready("generation-a")
        var opened: ReadySource? = null
        var asked: String? = null
        val state =
            state(
                ready,
                factory =
                    BoundDocumentSourceFactory { source ->
                        opened = source
                        source(answer = answer()).also { document ->
                            document.onRead = { asked = it }
                        }
                    },
            )

        await { state.phase is DocumentReaderPhase.Ready }

        val document = (state.phase as DocumentReaderPhase.Ready).document
        assertEquals(ready, opened)
        assertEquals(NODE_KEY, asked)
        assertEquals("A complete note.\n", document.source.org)
        assertEquals("A note", document.anchor.title)
        assertEquals(SOURCE, document.source.binding.source)
        assertEquals("generation-a", document.source.binding.generation)
        assertEquals(NODE_KEY, document.source.binding.id)
        state.close()
    }

    @Test
    fun aMissingNodeAndBothExplicitSizeBoundsHaveDistinctStates() {
        val missing = state(factory = failing(notFound()))
        await { missing.phase == DocumentReaderPhase.NotFound }

        val lines = state(factory = failing(outOfBounds(AdapterBound.NOTE_SOURCE_LINES), 837))
        await { lines.phase is DocumentReaderPhase.UnsupportedSize }
        assertEquals(837, (lines.phase as DocumentReaderPhase.UnsupportedSize).maxLines)

        val bytes = state(factory = failing(outOfBounds(AdapterBound.RESPONSE_BYTES)))
        await { bytes.phase is DocumentReaderPhase.UnsupportedSize }
        assertEquals(null, (bytes.phase as DocumentReaderPhase.UnsupportedSize).maxLines)

        missing.close()
        lines.close()
        bytes.close()
    }

    @Test
    fun anIncompleteOrStructurallyMismatchedAnswerIsNeverRendered() {
        val truncated = answer().copy(source = answer().source.copy(truncatedAfter = true))
        val state = state(factory = BoundDocumentSourceFactory { source(answer = truncated) })

        await { state.phase == DocumentReaderPhase.Failed }

        state.close()
    }

    @Test
    fun anEmptyFileIsACompleteReadableDocument() {
        val empty =
            answer().copy(
                source =
                    answer().source.copy(
                        lineCount = 0,
                        totalLines = 0,
                        content = "",
                    ),
            )
        val state = state(factory = BoundDocumentSourceFactory { source(empty) })

        await { state.phase is DocumentReaderPhase.Ready }

        assertEquals("", (state.phase as DocumentReaderPhase.Ready).document.source.org)
        state.close()
    }

    @Test
    fun closingAWithdrawnNoteSuppressesItsLateReply() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val old =
            state(
                factory =
                    BoundDocumentSourceFactory {
                        source(answer()).also { source ->
                            source.beforeRead = {
                                entered.countDown()
                                release.await(5, TimeUnit.SECONDS)
                            }
                        }
                    },
            )
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        old.close()
        release.countDown()

        val currentAnswer = answer().copy(source = answer().source.copy(content = "Current.\n"))
        val current =
            state(
                ready = ready("generation-b"),
                factory = BoundDocumentSourceFactory { source(currentAnswer) },
            )
        await { current.phase is DocumentReaderPhase.Ready }

        assertEquals(DocumentReaderPhase.Loading, old.phase)
        assertEquals(
            "Current.\n",
            (current.phase as DocumentReaderPhase.Ready).document.source.org,
        )
        current.close()
    }

    @Test
    fun aLinkResolvesAgainstTheReaderSourceAndOriginNode() {
        val opened = AtomicReference<ReadySource>()
        var request: Pair<String, String>? = null
        val source = source(answer())
        source.onResolve = { nodeKey, target ->
            request = nodeKey to target
            DocumentLinkResolution.Note("heading:other.org:4")
        }
        val state =
            state(
                ready = ready("generation-bound"),
                factory = BoundDocumentSourceFactory { ready ->
                    opened.set(ready)
                    source
                },
            )
        await { state.phase is DocumentReaderPhase.Ready }
        var followed: DocumentLinkResolution? = null

        state.follow("file:other.org::*Target") { followed = it }
        await { followed != null }

        assertEquals("generation-bound", opened.get().binding.generation)
        assertEquals(NODE_KEY to "file:other.org::*Target", request)
        assertEquals(DocumentLinkResolution.Note("heading:other.org:4"), followed)
        assertEquals(ReaderLinkPhase.Idle, state.linkPhase)
        state.close()
    }

    @Test
    fun missingUnsupportedAndFailedLinksHaveDistinctFeedback() {
        val missingSource = source(answer())
        missingSource.onResolve = { _, _ -> DocumentLinkResolution.Missing }
        val missing = state(factory = BoundDocumentSourceFactory { missingSource })
        await { missing.phase is DocumentReaderPhase.Ready }
        missing.follow("missing.org") {}
        await { missing.linkPhase is ReaderLinkPhase.Missing }

        val unsupportedSource = source(answer())
        unsupportedSource.onResolve = { _, _ -> DocumentLinkResolution.Unsupported }
        val unsupported = state(factory = BoundDocumentSourceFactory { unsupportedSource })
        await { unsupported.phase is DocumentReaderPhase.Ready }
        unsupported.follow("javascript:alert(1)") {}
        await { unsupported.linkPhase is ReaderLinkPhase.Unsupported }

        val failedSource = source(answer())
        failedSource.onResolve = { _, _ -> error("offline") }
        val failed = state(factory = BoundDocumentSourceFactory { failedSource })
        await { failed.phase is DocumentReaderPhase.Ready }
        failed.follow("id:later") {}
        await { failed.linkPhase is ReaderLinkPhase.Failed }

        missing.close()
        unsupported.close()
        failed.close()
    }

    @Test
    fun aLaterLinkSuppressesTheEarlierLateResolution() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val source = source(answer())
        source.onResolve = { _, target ->
            if (target == "id:old") {
                entered.countDown()
                release.await(5, TimeUnit.SECONDS)
                DocumentLinkResolution.Note("heading:old.org:1")
            } else {
                DocumentLinkResolution.Note("heading:new.org:1")
            }
        }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }
        val followed = mutableListOf<DocumentLinkResolution>()

        state.follow("id:old", followed::add)
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        state.follow("id:new", followed::add)
        await { followed.isNotEmpty() }
        release.countDown()

        assertEquals(listOf(DocumentLinkResolution.Note("heading:new.org:1")), followed)
        state.close()
    }

    private fun state(
        ready: ReadySource = ready("generation-a"),
        factory: BoundDocumentSourceFactory,
    ): DocumentReaderState =
        DocumentReaderState(
            note = BoundNote(ready.binding, NODE_KEY),
            ready = ready,
            factory = factory,
            delivery = ImportDelivery { it() },
        )

    private fun failing(failure: Throwable, maxLines: Int = 1000): BoundDocumentSourceFactory =
        BoundDocumentSourceFactory { source(failure = failure, maxLines = maxLines) }

    private fun source(
        answer: ReadNodeSourceResult? = null,
        failure: Throwable? = null,
        maxLines: Int = 1000,
    ): StubDocumentSource = StubDocumentSource(answer, failure, maxLines)

    private class StubDocumentSource(
        private val answer: ReadNodeSourceResult?,
        private val failure: Throwable?,
        override val maxLines: Int,
    ) : BoundDocumentSource {

        var beforeRead: () -> Unit = {}
        var onRead: (String) -> Unit = {}
        var onResolve: (String, String) -> DocumentLinkResolution = { _, _ ->
            DocumentLinkResolution.Unsupported
        }

        override fun read(nodeKey: String): ReadNodeSourceResult {
            beforeRead()
            onRead(nodeKey)
            failure?.let { throw it }
            return checkNotNull(answer)
        }

        override fun resolve(sourceNodeKey: String, target: String): DocumentLinkResolution =
            onResolve(sourceNodeKey, target)

        override fun close() = Unit
    }

    private fun notFound(): EngineRefusedException =
        EngineRefusedException(
            AdapterResponse.Refused(
                reason = RefusalReason.ENGINE_REFUSED,
                engine = EngineRefusal(-32004, EngineRefusalKind.NOT_FOUND),
                version = ADAPTER_PROTOCOL_VERSION,
            ),
        )

    private fun outOfBounds(bound: AdapterBound): EngineRefusedException =
        EngineRefusedException(
            AdapterResponse.Refused(
                reason = RefusalReason.OUT_OF_BOUNDS,
                bound = bound,
                version = ADAPTER_PROTOCOL_VERSION,
            ),
        )

    private fun answer(): ReadNodeSourceResult =
        ReadNodeSourceResult(
            anchor = node(),
            source =
                SourceSlice(
                    filePath = "note.org",
                    startLine = 1,
                    lineCount = 1,
                    totalLines = 1,
                    content = "A complete note.\n",
                    truncatedBefore = false,
                    truncatedAfter = false,
                ),
            nodeStartLine = 1,
            nodeLineCount = 1,
        )

    private fun node(): NodeRecord =
        NodeRecord(
            nodeKey = NODE_KEY,
            explicitId = null,
            filePath = "note.org",
            title = "A note",
            outlinePath = "",
            aliases = emptyList(),
            tags = listOf("proof"),
            refs = emptyList(),
            todoKeyword = null,
            scheduledFor = null,
            deadlineFor = null,
            closedAt = null,
            glossary = false,
            glossaryStatus = null,
            srDue = null,
            srEase = null,
            srInterval = null,
            srReps = null,
            srLast = null,
            level = 0,
            line = 1,
            kind = NodeKind.FILE,
            fileMtimeNs = 0,
            backlinkCount = 2,
            forwardLinkCount = 3,
        )

    private fun ready(generation: String): ReadySource =
        ReadySource(
            source =
                RefreshSource(
                    id = SOURCE,
                    displayName = "Notes",
                    provider = RefreshProvider.GENERIC_HTTPS,
                    visibility = RefreshVisibility.PUBLIC,
                    remote = "https://example.com/notes.git",
                    branch = "main",
                    notesFolder = "",
                ),
            binding = GenerationBinding(SOURCE, generation),
            revision = "0123456789abcdef",
            contentRoot = "/private/source",
            database = "/private/index.sqlite",
            stats = ReadySourceStats(1, 1, 5),
        )

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) Thread.yield()
        assertTrue(condition())
    }

    private companion object {
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val NODE_KEY = "file:note.org"
    }
}
