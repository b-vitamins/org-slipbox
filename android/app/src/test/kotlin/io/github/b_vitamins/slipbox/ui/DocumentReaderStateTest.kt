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
import io.github.b_vitamins.slipbox.ui.content.DocumentGesture
import io.github.b_vitamins.slipbox.ui.content.DocumentPosition
import io.github.b_vitamins.slipbox.ui.content.AttachmentOpenResult
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
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
                            document.onRead = { nodeKey, _ -> asked = nodeKey }
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
        assertEquals("note.org", document.source.binding.filePath)
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
    fun aStableIdWinsAfterRenameAndLaterLinksUseTheResolvedKey() {
        val movedKey = "file:moved.org"
        val moved =
            answer().copy(
                anchor =
                    node().copy(
                        nodeKey = movedKey,
                        explicitId = "stable-id",
                        filePath = "moved.org",
                    ),
                source = answer().source.copy(filePath = "moved.org"),
            )
        var resolvedFrom: String? = null
        val source = source(moved)
        source.onFindId = { id -> moved.anchor.takeIf { id == "stable-id" } }
        source.onResolve = { nodeKey, _ ->
            resolvedFrom = nodeKey
            DocumentLinkResolution.Missing
        }
        val state =
            state(
                note = BoundNote(ready("generation-a").binding, NODE_KEY, "stable-id", "note.org"),
                factory = BoundDocumentSourceFactory { source },
            )
        await { state.phase is DocumentReaderPhase.Ready }

        assertEquals(movedKey, (state.phase as DocumentReaderPhase.Ready).document.anchor.nodeKey)
        state.follow("id:missing") {}
        await { state.linkPhase is ReaderLinkPhase.Missing }
        assertEquals(movedKey, resolvedFrom)
        state.close()
    }

    @Test
    fun aMissingStableIdNeverFallsThroughToAReusedFileKey() {
        var reads = 0
        val source = source(answer()).also { it.onRead = { _, _ -> reads += 1 } }
        val state =
            state(
                note = BoundNote(ready("generation-a").binding, NODE_KEY, "deleted-id", "note.org"),
                factory = BoundDocumentSourceFactory { source },
            )

        await { state.phase == DocumentReaderPhase.NotFound }

        assertEquals(0, reads)
        state.close()
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
    fun attachmentOutcomesStayBoundToTheMountedDocumentAndRemainDistinct() {
        val state = state(factory = BoundDocumentSourceFactory { source(answer()) })
        await { state.phase is DocumentReaderPhase.Ready }
        var asked: Pair<io.github.b_vitamins.slipbox.ui.content.DocumentBinding, String>? = null
        val target = "file:assets/diagram.png"

        state.openAttachment(target) { binding, openedTarget ->
            asked = binding to openedTarget
            AttachmentOpenResult.Missing("diagram.png")
        }
        await { state.linkPhase is ReaderLinkPhase.AssetMissing }

        assertEquals(target, asked?.second)
        assertEquals(SOURCE, asked?.first?.source)
        assertEquals("generation-a", asked?.first?.generation)
        assertEquals("note.org", asked?.first?.filePath)
        assertEquals(ReaderLinkPhase.AssetMissing("diagram.png"), state.linkPhase)

        state.openAttachment(target) { _, _ ->
            AttachmentOpenResult.Oversized("diagram.png", 32L * 1024L * 1024L)
        }
        await { state.linkPhase is ReaderLinkPhase.AssetOversized }
        assertEquals(
            ReaderLinkPhase.AssetOversized("diagram.png", 32L * 1024L * 1024L),
            state.linkPhase,
        )
        state.close()
    }

    @Test
    fun aLaterNavigationSuppressesALateAttachmentReply() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val source = source(answer())
        source.onResolve = { _, _ -> DocumentLinkResolution.Note("file:next.org") }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }

        state.openAttachment("file:slow.png") { _, _ ->
            entered.countDown()
            release.await(5, TimeUnit.SECONDS)
            AttachmentOpenResult.Missing("slow.png")
        }
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        var followed: DocumentLinkResolution? = null
        state.follow("file:next.org") { followed = it }
        await { followed != null }
        release.countDown()

        assertEquals(DocumentLinkResolution.Note("file:next.org"), followed)
        assertEquals(ReaderLinkPhase.Idle, state.linkPhase)
        state.close()
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

    @Test
    fun aPreviewResolvesAndReadsABoundedTargetInTheOriginatingGeneration() {
        val ready = ready("generation-preview")
        val target = "file:term.org"
        val term = previewAnswer(target, glossary = true, shortened = true)
        val reads = mutableListOf<Pair<String, Int>>()
        var resolution: Pair<String, String>? = null
        val source = source(answer())
        source.onResolve = { nodeKey, asked ->
            resolution = nodeKey to asked
            DocumentLinkResolution.Note(target)
        }
        source.onRead = { nodeKey, maxLines -> reads += nodeKey to maxLines }
        source.answerFor = { nodeKey, _ -> if (nodeKey == target) term else answer() }
        val state = state(ready = ready, factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }

        state.preview(
            target = "id:derivative",
            gesture = DocumentGesture.Touch,
            originProgress = 0.625f,
            origin = "$MOUNT:1",
            originPosition = DocumentPosition("paragraph:0", 0.625f, 0.25f),
        )
        await { state.previewPhase is ReaderPreviewPhase.Ready }

        val preview = (state.previewPhase as ReaderPreviewPhase.Ready).preview
        assertEquals(NODE_KEY to "id:derivative", resolution)
        assertEquals(target to 12, reads.last())
        assertEquals("generation-preview", preview.source.binding.generation)
        assertEquals(SOURCE, preview.source.binding.source)
        assertEquals(target, preview.source.binding.id)
        assertTrue(preview.anchor.glossary)
        assertEquals(12, preview.excerptLines)
        assertTrue(preview.shortened)
        assertEquals(0.625f, preview.request.originProgress)
        assertEquals("paragraph:0", preview.request.originPosition?.mark)
        state.close()
    }

    @Test
    fun dismissRestoresTheExactOriginAndOpenUsesTheResolvedTargetWithoutResolvingAgain() {
        val target = "heading:other.org:4"
        val source = source(answer())
        var resolutions = 0
        source.onResolve = { _, _ ->
            resolutions += 1
            DocumentLinkResolution.Note(target)
        }
        source.answerFor = { nodeKey, _ ->
            if (nodeKey == target) previewAnswer(target) else answer()
        }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }

        state.preview("file:other.org::*Target", DocumentGesture.Touch, 0.4f, "$MOUNT:7")
        await { state.previewPhase is ReaderPreviewPhase.Ready }
        state.dismissPreview()

        assertEquals(ReaderPreviewPhase.Hidden, state.previewPhase)
        assertEquals("$MOUNT:7", state.focusRequest?.origin)

        state.preview("file:other.org::*Target", DocumentGesture.Focus, 0.4f, "$MOUNT:8")
        await { state.previewPhase is ReaderPreviewPhase.Ready }
        var opened: ReaderPreview? = null
        state.openPreview { opened = it }

        assertEquals(target, opened?.anchor?.nodeKey)
        assertEquals(0.4f, opened?.request?.originProgress)
        assertEquals(2, resolutions)
        assertEquals(ReaderPreviewPhase.Hidden, state.previewPhase)
        assertEquals("$MOUNT:7", state.focusRequest?.origin)
        state.close()
    }

    @Test
    fun previewFailuresLeaveTheReaderReadyAndRestoreItsOrigin() {
        val source = source(answer())
        source.onResolve = { _, _ -> DocumentLinkResolution.Missing }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }

        state.preview("file:missing.org", DocumentGesture.Touch, 0.2f, "$MOUNT:3")
        await { state.linkPhase is ReaderLinkPhase.Missing }

        assertTrue(state.phase is DocumentReaderPhase.Ready)
        assertEquals(ReaderPreviewPhase.Hidden, state.previewPhase)
        assertEquals("$MOUNT:3", state.focusRequest?.origin)
        state.close()
    }

    @Test
    fun anExternalTouchLinkKeepsItsOrdinaryBrowserHandoff() {
        val external = DocumentLinkResolution.External("https://example.org/paper")
        val source = source(answer())
        source.onResolve = { _, _ -> external }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }
        var handedOff: DocumentLinkResolution.External? = null

        state.preview(
            target = external.url,
            gesture = DocumentGesture.Touch,
            originProgress = 0.3f,
            origin = "$MOUNT:5",
            onExternal = { resolution ->
                handedOff = resolution
                true
            },
        )
        await { handedOff != null }

        assertEquals(external, handedOff)
        assertEquals(ReaderPreviewPhase.Hidden, state.previewPhase)
        assertEquals(ReaderLinkPhase.Idle, state.linkPhase)
        assertNull(state.focusRequest)
        state.close()
    }

    @Test
    fun dismissingALoadingPreviewSuppressesItsLateRead() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val target = "file:later.org"
        val source = source(answer())
        source.onResolve = { _, _ ->
            entered.countDown()
            release.await(5, TimeUnit.SECONDS)
            DocumentLinkResolution.Note(target)
        }
        source.answerFor = { nodeKey, _ ->
            if (nodeKey == target) previewAnswer(target) else answer()
        }
        val state = state(factory = BoundDocumentSourceFactory { source })
        await { state.phase is DocumentReaderPhase.Ready }

        state.preview("id:later", DocumentGesture.Touch, 0.1f, "$MOUNT:4")
        assertTrue(entered.await(5, TimeUnit.SECONDS))
        state.dismissPreview()
        release.countDown()

        assertEquals(ReaderPreviewPhase.Hidden, state.previewPhase)
        assertEquals("$MOUNT:4", state.focusRequest?.origin)
        state.close()
    }

    private fun state(
        ready: ReadySource = ready("generation-a"),
        note: BoundNote = BoundNote(ready.binding, NODE_KEY),
        factory: BoundDocumentSourceFactory,
    ): DocumentReaderState =
        DocumentReaderState(
            note = note,
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
        var onRead: (String, Int) -> Unit = { _, _ -> }
        var answerFor: (String, Int) -> ReadNodeSourceResult = { _, _ -> checkNotNull(answer) }
        var onFindId: (String) -> NodeRecord? = { null }
        var onFindKey: (String) -> NodeRecord? = { null }
        var onResolve: (String, String) -> DocumentLinkResolution = { _, _ ->
            DocumentLinkResolution.Unsupported
        }

        override fun read(nodeKey: String, maxLines: Int): ReadNodeSourceResult {
            beforeRead()
            onRead(nodeKey, maxLines)
            failure?.let { throw it }
            return answerFor(nodeKey, maxLines)
        }

        override fun findById(id: String): NodeRecord? = onFindId(id)

        override fun findByKey(nodeKey: String): NodeRecord? = onFindKey(nodeKey)

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

    private fun previewAnswer(
        nodeKey: String,
        glossary: Boolean = false,
        shortened: Boolean = false,
    ): ReadNodeSourceResult {
        val lines = if (shortened) 12L else 1L
        val path =
            nodeKey.substringAfter(':').let { value ->
                if (nodeKey.startsWith("heading:")) value.substringBeforeLast(':') else value
            }
        val anchor =
            node().copy(
                nodeKey = nodeKey,
                filePath = path,
                title = if (glossary) "Derivative" else "Target",
                glossary = glossary,
            )
        return ReadNodeSourceResult(
            anchor = anchor,
            source =
                SourceSlice(
                    filePath = anchor.filePath,
                    startLine = 1,
                    lineCount = lines,
                    totalLines = if (shortened) 120 else 1,
                    content = if (glossary) "A rate of change.\n" else "Target body.\n",
                    truncatedBefore = false,
                    truncatedAfter = shortened,
                ),
            nodeStartLine = 1,
            nodeLineCount = if (shortened) 120 else 1,
        )
    }

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
        const val MOUNT = "3f2a9c81-4d5e-4f60-9a1b-0c2d3e4f5061"
    }
}
