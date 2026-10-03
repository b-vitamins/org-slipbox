/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import io.github.b_vitamins.slipbox.engine.AdapterBound
import io.github.b_vitamins.slipbox.engine.DocumentLinkResolution
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.EngineRefusalKind
import io.github.b_vitamins.slipbox.engine.EngineRefusedException
import io.github.b_vitamins.slipbox.engine.NodeKind
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.NoteContextResult
import io.github.b_vitamins.slipbox.engine.NotePlace
import io.github.b_vitamins.slipbox.engine.ReadNodeSourceResult
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.RefusalReason
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.ui.content.AttachmentOpenResult
import io.github.b_vitamins.slipbox.ui.content.DocumentFocusRequest
import io.github.b_vitamins.slipbox.ui.content.DocumentGesture
import io.github.b_vitamins.slipbox.ui.content.DocumentPosition
import io.github.b_vitamins.slipbox.ui.content.DocumentSource
import io.github.b_vitamins.slipbox.ui.content.ReaderAttachment
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal sealed interface DocumentReaderPhase {

    data object Loading : DocumentReaderPhase

    data class Ready(val document: ReaderDocument) : DocumentReaderPhase

    data object NotFound : DocumentReaderPhase

    data object SourceUnavailable : DocumentReaderPhase

    data class UnsupportedSize(val maxLines: Int?) : DocumentReaderPhase

    data object Failed : DocumentReaderPhase
}

internal sealed interface ReaderLinkPhase {
    data object Idle : ReaderLinkPhase

    data object Resolving : ReaderLinkPhase

    data class Missing(val target: String) : ReaderLinkPhase

    data class Unsupported(val target: String) : ReaderLinkPhase

    data class Failed(val target: String) : ReaderLinkPhase

    data object ExternalUnavailable : ReaderLinkPhase

    data class AssetMissing(val label: String) : ReaderLinkPhase

    data class AssetUnsupported(val label: String) : ReaderLinkPhase

    data class AssetOversized(val label: String, val maxBytes: Long) : ReaderLinkPhase

    data class AssetViewerUnavailable(val label: String) : ReaderLinkPhase

    data class AssetFailed(val label: String) : ReaderLinkPhase
}

internal data class ReaderDocument(
    val anchor: NodeRecord,
    val source: DocumentSource,
    val addressedAnchor: NodeRecord = anchor,
    val outline: List<NodeRecord> = emptyList(),
    val place: NotePlace? = null,
    val sourceName: String = "",
    val revision: String = "",
)

internal data class ReaderPreviewRequest(
    val target: String,
    val gesture: DocumentGesture,
    val originProgress: Float,
    val origin: String,
    val originPosition: DocumentPosition? = null,
)

internal data class ReaderPreview(
    val request: ReaderPreviewRequest,
    val anchor: NodeRecord,
    val source: DocumentSource,
    val excerptLines: Int,
    val shortened: Boolean,
)

internal sealed interface ReaderPreviewPhase {
    data object Hidden : ReaderPreviewPhase

    data class Loading(val request: ReaderPreviewRequest) : ReaderPreviewPhase

    data class Ready(val preview: ReaderPreview) : ReaderPreviewPhase
}

internal interface BoundDocumentSource : AutoCloseable {

    val maxLines: Int

    fun read(nodeKey: String, maxLines: Int = this.maxLines): ReadNodeSourceResult

    fun context(nodeKey: String, maxLines: Int = this.maxLines): NoteContextResult

    fun findById(id: String): NodeRecord? = null

    fun findByKey(nodeKey: String): NodeRecord? = null

    fun resolve(sourceNodeKey: String, target: String): DocumentLinkResolution
}

internal fun interface BoundDocumentSourceFactory {

    fun open(ready: ReadySource): BoundDocumentSource
}

private object NativeBoundDocumentSourceFactory : BoundDocumentSourceFactory {

    override fun open(ready: ReadySource): BoundDocumentSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : BoundDocumentSource {
                override val maxLines: Int = host.contract.limits.maxNoteSourceLines

                override fun read(nodeKey: String, maxLines: Int): ReadNodeSourceResult {
                    val operation =
                        ReadOperation.ReadNodeSource(
                            nodeKey = nodeKey,
                            contextBefore = 0,
                            contextAfter = 0,
                            maxLines = maxLines.coerceAtMost(this.maxLines),
                        )
                    return (session.answer(operation).await() as EngineAnswer.ReadNodeSource).result
                }

                override fun context(nodeKey: String, maxLines: Int): NoteContextResult {
                    val operation =
                        ReadOperation.NoteContext(
                            nodeKey = nodeKey,
                            sourceContextBefore = 0,
                            sourceContextAfter = 0,
                            sourceMaxLines = maxLines.coerceAtMost(this.maxLines),
                            relationLimit = 1,
                        )
                    return (session.answer(operation).await() as EngineAnswer.NoteContext).result
                }

                override fun findById(id: String): NodeRecord? =
                    (session.answer(ReadOperation.NodeFromId(id)).await() as EngineAnswer.NodeFromId)
                        .result

                override fun findByKey(nodeKey: String): NodeRecord? =
                    (session.answer(ReadOperation.NodeFromKey(nodeKey)).await() as EngineAnswer.NodeFromKey)
                        .result

                override fun resolve(
                    sourceNodeKey: String,
                    target: String,
                ): DocumentLinkResolution {
                    val operation = ReadOperation.ResolveDocumentLink(sourceNodeKey, target)
                    return (session.answer(operation).await() as EngineAnswer.ResolveDocumentLink).result
                }

                override fun close() {
                    host.close()
                }
            }
        } catch (failure: Throwable) {
            host.close()
            throw failure
        }
    }
}

/** Owns one complete document read for exactly one note in one ready generation. */
@Stable
internal class DocumentReaderState(
    private val note: BoundNote,
    private val ready: ReadySource,
    private val factory: BoundDocumentSourceFactory = NativeBoundDocumentSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val requests = AtomicLong()
    private val linkRequests = AtomicLong()
    private val opened = AtomicReference<BoundDocumentSource?>()

    var phase: DocumentReaderPhase by mutableStateOf(DocumentReaderPhase.Loading)
        private set

    var linkPhase: ReaderLinkPhase by mutableStateOf(ReaderLinkPhase.Idle)
        private set

    var previewPhase: ReaderPreviewPhase by mutableStateOf(ReaderPreviewPhase.Hidden)
        private set

    var focusRequest: DocumentFocusRequest? by mutableStateOf(null)
        private set

    init {
        require(note.binding == ready.binding) { "the document and ready source generations differ" }
        request()
    }

    fun retry() {
        if (phase != DocumentReaderPhase.Failed || !live.get()) return
        opened.getAndSet(null)?.close()
        phase = DocumentReaderPhase.Loading
        request()
    }

    /** Resolve against this reader's exact source generation, never the later active source. */
    fun follow(target: String, completed: (DocumentLinkResolution) -> Unit) {
        val origin = (phase as? DocumentReaderPhase.Ready)?.document?.anchor?.nodeKey ?: return
        if (!live.get()) return
        val serial = linkRequests.incrementAndGet()
        previewPhase = ReaderPreviewPhase.Hidden
        linkPhase = ReaderLinkPhase.Resolving
        Thread(
                {
                    val outcome = runCatching {
                        checkNotNull(opened.get()).resolve(origin, target)
                    }
                    delivery.post {
                        if (!live.get() || linkRequests.get() != serial) return@post
                        outcome.fold(
                            onSuccess = { resolution ->
                                when (resolution) {
                                    is DocumentLinkResolution.Note,
                                    is DocumentLinkResolution.External,
                                    -> {
                                        linkPhase = ReaderLinkPhase.Idle
                                        completed(resolution)
                                    }
                                    DocumentLinkResolution.Missing ->
                                        linkPhase = ReaderLinkPhase.Missing(target)
                                    DocumentLinkResolution.Unsupported ->
                                        linkPhase = ReaderLinkPhase.Unsupported(target)
                                }
                            },
                            onFailure = { linkPhase = ReaderLinkPhase.Failed(target) },
                        )
                    }
                },
                LINK_WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    /** Open one explicit attachment from this mounted document's exact generation. */
    fun openAttachment(target: String, attachment: ReaderAttachment) {
        val document = (phase as? DocumentReaderPhase.Ready)?.document ?: return
        if (!live.get()) return
        val serial = linkRequests.incrementAndGet()
        previewPhase = ReaderPreviewPhase.Hidden
        linkPhase = ReaderLinkPhase.Resolving
        Thread(
                {
                    val outcome = runCatching { attachment.open(document.source.binding, target) }
                    delivery.post {
                        if (!live.get() || linkRequests.get() != serial) return@post
                        linkPhase =
                            outcome.fold(
                                onSuccess = { result ->
                                    when (result) {
                                        AttachmentOpenResult.Opened -> ReaderLinkPhase.Idle
                                        is AttachmentOpenResult.Missing ->
                                            ReaderLinkPhase.AssetMissing(result.label)
                                        is AttachmentOpenResult.Unsupported ->
                                            ReaderLinkPhase.AssetUnsupported(result.label)
                                        is AttachmentOpenResult.Oversized ->
                                            ReaderLinkPhase.AssetOversized(
                                                result.label,
                                                result.maxBytes,
                                            )
                                        is AttachmentOpenResult.ViewerUnavailable ->
                                            ReaderLinkPhase.AssetViewerUnavailable(result.label)
                                        is AttachmentOpenResult.Failed ->
                                            ReaderLinkPhase.AssetFailed(result.label)
                                    }
                                },
                                onFailure = { ReaderLinkPhase.AssetFailed(target) },
                            )
                    }
                },
                ASSET_WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    /** Resolve and read a bounded excerpt through this reader's source-bound session. */
    fun preview(
        target: String,
        gesture: DocumentGesture,
        originProgress: Float,
        origin: String,
        originPosition: DocumentPosition? = null,
        onExternal: (DocumentLinkResolution.External) -> Boolean = { false },
    ) {
        val sourceNodeKey = (phase as? DocumentReaderPhase.Ready)?.document?.anchor?.nodeKey ?: return
        if (!live.get()) return
        val request = ReaderPreviewRequest(target, gesture, originProgress, origin, originPosition)
        requestPreview(request, onExternal) { selected ->
            when (val resolution = selected.resolve(sourceNodeKey, target)) {
                is DocumentLinkResolution.Note -> selected.preview(resolution.nodeKey)
                is DocumentLinkResolution.External -> PreviewResolution.External(resolution)
                DocumentLinkResolution.Missing -> PreviewResolution.Missing
                DocumentLinkResolution.Unsupported -> PreviewResolution.Unsupported
            }
        }
    }

    /** Read a known relation target through this reader's source-bound session. */
    fun previewNode(
        nodeKey: String,
        gesture: DocumentGesture,
        originProgress: Float,
        origin: String,
        originPosition: DocumentPosition? = null,
    ) {
        if (phase !is DocumentReaderPhase.Ready || !live.get()) return
        val request = ReaderPreviewRequest(nodeKey, gesture, originProgress, origin, originPosition)
        requestPreview(request) { selected -> selected.preview(nodeKey) }
    }

    private fun requestPreview(
        request: ReaderPreviewRequest,
        onExternal: (DocumentLinkResolution.External) -> Boolean = { false },
        answer: (BoundDocumentSource) -> PreviewResolution,
    ) {
        val serial = linkRequests.incrementAndGet()
        linkPhase = ReaderLinkPhase.Idle
        previewPhase = ReaderPreviewPhase.Loading(request)
        Thread(
                {
                    val outcome = runCatching { answer(checkNotNull(opened.get())) }
                    delivery.post {
                        if (!live.get() || linkRequests.get() != serial) return@post
                        outcome.fold(
                            onSuccess = { resolution ->
                                acceptPreview(request, resolution, onExternal)
                            },
                            onFailure = {
                                previewPhase = ReaderPreviewPhase.Hidden
                                restorePreviewFocus(request)
                                linkPhase = ReaderLinkPhase.Failed(request.target)
                            },
                        )
                    }
                },
                PREVIEW_WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun BoundDocumentSource.preview(nodeKey: String): PreviewResolution.Note {
        val answer = read(nodeKey, PREVIEW_MAX_LINES)
        validatePreview(nodeKey, answer)
        return PreviewResolution.Note(answer)
    }

    fun dismissPreview() {
        val request = previewRequest() ?: return
        linkRequests.incrementAndGet()
        previewPhase = ReaderPreviewPhase.Hidden
        linkPhase = ReaderLinkPhase.Idle
        restorePreviewFocus(request)
    }

    fun openPreview(completed: (ReaderPreview) -> Unit) {
        val preview = (previewPhase as? ReaderPreviewPhase.Ready)?.preview ?: return
        linkRequests.incrementAndGet()
        previewPhase = ReaderPreviewPhase.Hidden
        linkPhase = ReaderLinkPhase.Idle
        completed(preview)
    }

    fun externalUnavailable() {
        if (live.get()) linkPhase = ReaderLinkPhase.ExternalUnavailable
    }

    private fun request() {
        val serial = requests.incrementAndGet()
        Thread(
                {
                    val outcome = runCatching {
                        val source = factory.open(ready)
                        if (!live.get() || requests.get() != serial) {
                            source.close()
                            throw IllegalStateException("the document reader was withdrawn while opening")
                        }
                        if (!opened.compareAndSet(null, source)) {
                            source.close()
                        }
                        val selected = checkNotNull(opened.get())
                        val resolved =
                            if (note.explicitId != null) {
                                selected.findById(note.explicitId)
                                    ?: throw MissingStableDocumentIdentity()
                            } else {
                                selected.findByKey(note.nodeKey)
                            }
                        val nodeKey = resolved?.nodeKey ?: note.nodeKey
                        selected.context(nodeKey).also { validate(nodeKey, it) }
                    }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        phase = outcome.fold(::ready, ::failed)
                    }
                },
                WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun validate(nodeKey: String, answer: NoteContextResult) {
        require(answer.anchor.nodeKey == nodeKey)
        require(answer.source.filePath == answer.note.filePath)
        require(answer.source.startLine == answer.nodeStartLine)
        require(
            answer.source.lineCount == answer.nodeLineCount ||
                (answer.source.totalLines == 0L &&
                    answer.source.lineCount == 0L &&
                    answer.nodeLineCount == 1L),
        )
        require(!answer.source.truncatedBefore && !answer.source.truncatedAfter)
        require(answer.place.total > 0 && answer.place.ordinal in 1..answer.place.total)
        require(
            answer.outline.zipWithNext().all { (left, right) -> left.line <= right.line } &&
                answer.outline.all { heading ->
                    heading.kind == NodeKind.HEADING &&
                        heading.filePath == answer.source.filePath &&
                        heading.line >= answer.source.startLine &&
                        heading.line < answer.source.startLine + answer.source.lineCount
                },
        )
    }

    private fun validatePreview(nodeKey: String, answer: ReadNodeSourceResult) {
        require(answer.anchor.nodeKey == nodeKey)
        require(answer.source.filePath == answer.anchor.filePath)
        require(answer.source.startLine == answer.nodeStartLine)
        require(!answer.source.truncatedBefore)
        val expected = minOf(answer.nodeLineCount, PREVIEW_MAX_LINES.toLong())
        require(
            answer.source.lineCount == expected ||
                (answer.source.totalLines == 0L &&
                    answer.source.lineCount == 0L &&
                    answer.nodeLineCount == 1L),
        )
    }

    private fun acceptPreview(
        request: ReaderPreviewRequest,
        resolution: PreviewResolution,
        onExternal: (DocumentLinkResolution.External) -> Boolean,
    ) {
        when (resolution) {
            is PreviewResolution.Note -> {
                previewPhase =
                    ReaderPreviewPhase.Ready(
                        ReaderPreview(
                            request = request,
                            anchor = resolution.answer.anchor,
                            source =
                                DocumentSource(
                                    source = note.binding.source,
                                    generation = note.binding.generation,
                                    id = resolution.answer.anchor.nodeKey,
                                    filePath = resolution.answer.anchor.filePath,
                                    org = resolution.answer.source.content,
                                ),
                            excerptLines = resolution.answer.source.lineCount.toInt(),
                            shortened = resolution.answer.source.truncatedAfter,
                        ),
                    )
            }
            is PreviewResolution.External -> {
                previewPhase = ReaderPreviewPhase.Hidden
                if (onExternal(resolution.resolution)) {
                    linkPhase = ReaderLinkPhase.Idle
                } else {
                    restorePreviewFocus(request)
                    linkPhase = ReaderLinkPhase.ExternalUnavailable
                }
            }
            PreviewResolution.Missing -> {
                previewPhase = ReaderPreviewPhase.Hidden
                restorePreviewFocus(request)
                linkPhase = ReaderLinkPhase.Missing(request.target)
            }
            PreviewResolution.Unsupported -> {
                previewPhase = ReaderPreviewPhase.Hidden
                restorePreviewFocus(request)
                linkPhase = ReaderLinkPhase.Unsupported(request.target)
            }
        }
    }

    private fun previewRequest(): ReaderPreviewRequest? =
        when (val preview = previewPhase) {
            ReaderPreviewPhase.Hidden -> null
            is ReaderPreviewPhase.Loading -> preview.request
            is ReaderPreviewPhase.Ready -> preview.preview.request
        }

    private fun restorePreviewFocus(request: ReaderPreviewRequest) {
        focusRequest = DocumentFocusRequest(request.origin)
    }

    private fun ready(answer: NoteContextResult): DocumentReaderPhase =
        DocumentReaderPhase.Ready(
            ReaderDocument(
                anchor = answer.note,
                source =
                    DocumentSource(
                        source = note.binding.source,
                        generation = note.binding.generation,
                        id = answer.note.nodeKey,
                        filePath = answer.note.filePath,
                        org = answer.source.content,
                    ),
                addressedAnchor = answer.anchor,
                outline = answer.outline,
                place = answer.place,
                sourceName = ready.source.displayName,
                revision = ready.revision,
            ),
        )

    private fun failed(failure: Throwable): DocumentReaderPhase =
        when {
            failure is MissingStableDocumentIdentity -> DocumentReaderPhase.NotFound

            failure is EngineRefusedException &&
                failure.refusal.reason == RefusalReason.ENGINE_REFUSED &&
                failure.refusal.engine?.kind == EngineRefusalKind.NOT_FOUND ->
                DocumentReaderPhase.NotFound

            failure is EngineRefusedException &&
                failure.refusal.reason == RefusalReason.OUT_OF_BOUNDS &&
                failure.refusal.bound == AdapterBound.NOTE_SOURCE_LINES ->
                DocumentReaderPhase.UnsupportedSize(opened.get()?.maxLines)

            failure is EngineRefusedException &&
                failure.refusal.reason == RefusalReason.OUT_OF_BOUNDS &&
                failure.refusal.bound == AdapterBound.RESPONSE_BYTES ->
                DocumentReaderPhase.UnsupportedSize(null)

            else -> DocumentReaderPhase.Failed
        }

    private class MissingStableDocumentIdentity : Exception()

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        linkRequests.incrementAndGet()
        previewPhase = ReaderPreviewPhase.Hidden
        opened.getAndSet(null)?.close()
    }

    private companion object {
        const val WORKER_NAME = "slipbox-document-reader"
        const val LINK_WORKER_NAME = "slipbox-document-link"
        const val PREVIEW_WORKER_NAME = "slipbox-document-preview"
        const val ASSET_WORKER_NAME = "slipbox-document-asset"
        const val PREVIEW_MAX_LINES = 12
    }
}

private sealed interface PreviewResolution {
    data class Note(
        val answer: ReadNodeSourceResult,
    ) : PreviewResolution

    data class External(
        val resolution: DocumentLinkResolution.External,
    ) : PreviewResolution

    data object Missing : PreviewResolution

    data object Unsupported : PreviewResolution
}

@Composable
internal fun rememberDocumentReaderState(
    note: BoundNote,
    ready: ReadySource,
): DocumentReaderState {
    val state =
        remember(note.reference, ready.binding, ready.contentRoot, ready.database) {
            DocumentReaderState(note, ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
