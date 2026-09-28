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
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadNodeSourceResult
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.RefusalReason
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.navigation.BoundNote
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.ui.content.DocumentSource
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
}

internal data class ReaderDocument(
    val anchor: NodeRecord,
    val source: DocumentSource,
)

internal interface BoundDocumentSource : AutoCloseable {

    val maxLines: Int

    fun read(nodeKey: String): ReadNodeSourceResult

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

                override fun read(nodeKey: String): ReadNodeSourceResult {
                    val operation =
                        ReadOperation.ReadNodeSource(
                            nodeKey = nodeKey,
                            contextBefore = 0,
                            contextAfter = 0,
                            maxLines = maxLines,
                        )
                    return (session.answer(operation).await() as EngineAnswer.ReadNodeSource).result
                }

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
        if (!live.get() || phase !is DocumentReaderPhase.Ready) return
        val serial = linkRequests.incrementAndGet()
        linkPhase = ReaderLinkPhase.Resolving
        Thread(
                {
                    val outcome = runCatching {
                        checkNotNull(opened.get()).resolve(note.nodeKey, target)
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
                        selected.read(note.nodeKey).also(::validate)
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

    private fun validate(answer: ReadNodeSourceResult) {
        require(answer.anchor.nodeKey == note.nodeKey)
        require(answer.source.filePath == answer.anchor.filePath)
        require(answer.source.startLine == answer.nodeStartLine)
        require(
            answer.source.lineCount == answer.nodeLineCount ||
                (answer.source.totalLines == 0L &&
                    answer.source.lineCount == 0L &&
                    answer.nodeLineCount == 1L),
        )
        require(!answer.source.truncatedBefore && !answer.source.truncatedAfter)
    }

    private fun ready(answer: ReadNodeSourceResult): DocumentReaderPhase =
        DocumentReaderPhase.Ready(
            ReaderDocument(
                anchor = answer.anchor,
                source =
                    DocumentSource(
                        source = note.binding.source,
                        generation = note.binding.generation,
                        id = note.nodeKey,
                        org = answer.source.content,
                    ),
            ),
        )

    private fun failed(failure: Throwable): DocumentReaderPhase =
        when {
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

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        linkRequests.incrementAndGet()
        opened.getAndSet(null)?.close()
    }

    private companion object {
        const val WORKER_NAME = "slipbox-document-reader"
        const val LINK_WORKER_NAME = "slipbox-document-link"
    }
}

@Composable
internal fun rememberDocumentReaderState(
    note: BoundNote,
    ready: ReadySource,
): DocumentReaderState {
    val state =
        remember(note, ready.binding, ready.contentRoot, ready.database) {
            DocumentReaderState(note, ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
