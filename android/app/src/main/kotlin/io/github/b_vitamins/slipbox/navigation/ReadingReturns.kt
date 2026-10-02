/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.security.AtomicVaultRecordFile
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sync.PackagedSourceRefreshStorage
import java.io.File
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

private const val READING_RETURNS_VERSION = 1
private const val MAX_BOOKMARKS = 128
private const val MAX_RECENTS = 64
private const val MAX_READING_RETURNS_BYTES = 512 * 1024

internal enum class ReadingReturnAvailability {
    Checking,
    Available,
    Missing,
    Unavailable,
}

/** Device-local return path. Repository content is never copied into this record. */
internal data class ReadingReturn(
    val note: BoundNote,
    val title: String,
    val anchor: ReadingAnchor,
    val availability: ReadingReturnAvailability = ReadingReturnAvailability.Checking,
)

internal data class ReadingReturnsSnapshot(
    val bookmarks: List<ReadingReturn> = emptyList(),
    val recents: List<ReadingReturn> = emptyList(),
)

@Serializable
private data class StoredReadingReturns(
    val version: Int = READING_RETURNS_VERSION,
    val source: String,
    val bookmarks: List<StoredReadingReturn>,
    val recents: List<StoredReadingReturn>,
)

@Serializable
private data class StoredReadingReturn(
    val nodeKey: String,
    val explicitId: String? = null,
    val filePath: String = "",
    val anchor: ReadingAnchor = ReadingAnchor.Start,
)

internal fun interface ReadingReturnsSink {
    fun save(source: String, snapshot: ReadingReturnsSnapshot)

    companion object {
        val None = ReadingReturnsSink { _, _ -> }
    }
}

/** Strict, bounded source-owned bookmarks and recents outside replaceable index generations. */
internal class ReadingReturnsStore(private val privateRoot: File) {

    fun load(binding: GenerationBinding): ReadingReturnsSnapshot {
        if (!binding.isCanonical()) return ReadingReturnsSnapshot()
        val file = PackagedSourceRefreshStorage.readingReturnsFile(privateRoot, binding.source)
            ?: return ReadingReturnsSnapshot()
        val bytes = boundedBytes(file) ?: return ReadingReturnsSnapshot()
        val text = strictUtf8(bytes) ?: return ReadingReturnsSnapshot()
        val stored =
            try {
                FORMAT.decodeFromString<StoredReadingReturns>(text)
            } catch (_: SerializationException) {
                return ReadingReturnsSnapshot()
            }
        if (stored.version != READING_RETURNS_VERSION ||
            stored.source != binding.source ||
            stored.bookmarks.size > MAX_BOOKMARKS ||
            stored.recents.size > MAX_RECENTS
        ) {
            return ReadingReturnsSnapshot()
        }
        val bookmarks = stored.bookmarks.mapNotNull { it.restore(binding) }
        val recents = stored.recents.mapNotNull { it.restore(binding) }
        if (bookmarks.size != stored.bookmarks.size || recents.size != stored.recents.size) {
            return ReadingReturnsSnapshot()
        }
        return ReadingReturnsSnapshot(bookmarks.distinctReturns(), recents.distinctReturns())
    }

    fun save(source: String, snapshot: ReadingReturnsSnapshot) {
        if (!isSourceIdentity(source)) return
        val bookmarks =
            snapshot.bookmarks
                .asSequence()
                .validFor(source)
                .take(MAX_BOOKMARKS)
                .map(ReadingReturn::stored)
                .toList()
        val recents =
            snapshot.recents
                .asSequence()
                .validFor(source)
                .take(MAX_RECENTS)
                .map(ReadingReturn::stored)
                .toList()
        val existing = PackagedSourceRefreshStorage.readingReturnsFile(privateRoot, source) ?: return
        if (bookmarks.isEmpty() && recents.isEmpty()) {
            AtomicVaultRecordFile(existing).delete()
            return
        }
        val encoded =
            FORMAT.encodeToString(
                StoredReadingReturns(source = source, bookmarks = bookmarks, recents = recents),
            ).toByteArray(Charsets.UTF_8)
        if (encoded.size > MAX_READING_RETURNS_BYTES) return
        val destination =
            PackagedSourceRefreshStorage.prepareReadingReturnsFile(privateRoot, source) ?: return
        AtomicVaultRecordFile(destination).replace(encoded)
    }

    private fun StoredReadingReturn.restore(
        binding: GenerationBinding,
    ): ReadingReturn? {
        val note = BoundNote(binding, nodeKey, explicitId, filePath)
        val label = filePath.takeIf(::isKeyText) ?: nodeKey
        return ReadingReturn(note, label, anchor)
            .takeIf { note.isCanonical() && anchor.isCanonical() }
    }

    private fun Sequence<ReadingReturn>.validFor(source: String): Sequence<ReadingReturn> =
        filter {
            it.note.binding.source == source &&
                it.note.isCanonical() &&
                it.anchor.isCanonical()
        }.distinctReturns()

    private fun boundedBytes(file: File): ByteArray? =
        try {
            if (!file.isFile || file.length() !in 1..MAX_READING_RETURNS_BYTES.toLong()) null
            else file.readBytes().takeIf { it.size <= MAX_READING_RETURNS_BYTES }
        } catch (_: Exception) {
            null
        }

    private fun strictUtf8(bytes: ByteArray): String? =
        try {
            Charsets.UTF_8
                .newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (_: CharacterCodingException) {
            null
        }

    private companion object {
        val FORMAT = Json {
            encodeDefaults = true
            ignoreUnknownKeys = false
            isLenient = false
        }
    }
}

private fun BoundNote.isCanonical(): Boolean =
    SlipboxRoute.Reader(this).isCanonical()

private fun ReadingReturn.stored(): StoredReadingReturn =
    StoredReadingReturn(
        nodeKey = note.nodeKey,
        explicitId = note.explicitId,
        filePath = note.filePath,
        anchor = anchor,
    )

private fun Iterable<ReadingReturn>.distinctReturns(): List<ReadingReturn> {
    val retained = mutableListOf<ReadingReturn>()
    for (entry in this) {
        if (retained.none { it.note.sameIdentity(entry.note) }) retained += entry
    }
    return retained
}

private fun Sequence<ReadingReturn>.distinctReturns(): Sequence<ReadingReturn> = sequence {
    val retained = mutableListOf<BoundNote>()
    for (entry in this@distinctReturns) {
        if (retained.none { it.sameIdentity(entry.note) }) {
            retained += entry.note
            yield(entry)
        }
    }
}

private fun BoundNote.sameIdentity(other: BoundNote): Boolean =
    binding.source == other.binding.source &&
        if (explicitId != null || other.explicitId != null) {
            explicitId != null && explicitId == other.explicitId
        } else {
            nodeKey == other.nodeKey
        }

private class AsyncReadingReturnsSink(private val store: ReadingReturnsStore) :
    ReadingReturnsSink,
    AutoCloseable {

    private val executor =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "slipbox-reading-returns").apply { isDaemon = true }
        }
    private val pending = mutableMapOf<String, ReadingReturnsSnapshot>()
    private var draining = false

    override fun save(source: String, snapshot: ReadingReturnsSnapshot) {
        synchronized(pending) {
            pending[source] = snapshot
            if (draining) return
            draining = true
        }
        runCatching { executor.execute(::drain) }
            .onFailure {
                synchronized(pending) {
                    pending.clear()
                    draining = false
                }
            }
    }

    fun load(binding: GenerationBinding): ReadingReturnsSnapshot =
        runCatching { executor.submit<ReadingReturnsSnapshot> { store.load(binding) }.get() }
            .getOrDefault(ReadingReturnsSnapshot())

    private fun drain() {
        while (true) {
            val next =
                synchronized(pending) {
                    val entry = pending.entries.firstOrNull()
                    if (entry == null) {
                        draining = false
                        return
                    }
                    pending.remove(entry.key)
                    entry.key to entry.value
                }
            store.save(next.first, next.second)
        }
    }

    override fun close() {
        executor.shutdown()
    }
}

internal interface ReadingReturnResolver : AutoCloseable {
    fun resolve(note: BoundNote): NodeRecord?
}

internal fun interface ReadingReturnResolverFactory {
    fun open(ready: ReadySource): ReadingReturnResolver
}

private object NativeReadingReturnResolverFactory : ReadingReturnResolverFactory {
    override fun open(ready: ReadySource): ReadingReturnResolver {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : ReadingReturnResolver {
                override fun resolve(note: BoundNote): NodeRecord? =
                    if (note.explicitId != null) {
                        (session.answer(ReadOperation.NodeFromId(note.explicitId)).await()
                                as EngineAnswer.NodeFromId)
                            .result
                    } else {
                        (session.answer(ReadOperation.NodeFromKey(note.nodeKey)).await()
                                as EngineAnswer.NodeFromKey)
                            .result
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

private data class ReconciledReturn(
    val original: BoundNote,
    val node: NodeRecord? = null,
    val failed: Boolean = false,
)

/** Owns one source's local reading returns and reconciles them against its ready generation. */
@Stable
internal class ReadingReturnsState(
    private val ready: ReadySource,
    restored: ReadingReturnsSnapshot,
    private val sink: ReadingReturnsSink = ReadingReturnsSink.None,
    private val resolverFactory: ReadingReturnResolverFactory = NativeReadingReturnResolverFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val requests = AtomicLong()

    var snapshot: ReadingReturnsSnapshot by mutableStateOf(restored)
        private set

    val source: String
        get() = ready.binding.source

    val continueReading: ReadingReturn?
        get() = snapshot.recents.firstOrNull()

    init {
        require(ready.binding.isCanonical())
        require((restored.bookmarks + restored.recents).all { it.note.binding == ready.binding })
        reconcile()
    }

    fun recordRecent(node: NodeRecord, anchor: ReadingAnchor) {
        if (!live.get() || !anchor.isCanonical()) return
        val entry = node.returnEntry(anchor)
        snapshot =
            snapshot.copy(
                recents =
                    (listOf(entry) + snapshot.recents.filterNot { it.note.sameIdentity(entry.note) })
                        .take(MAX_RECENTS),
            )
        persist()
    }

    fun rememberReadingPlace(note: BoundNote, anchor: ReadingAnchor) {
        if (!live.get() || !anchor.isCanonical()) return
        val bookmarks = snapshot.bookmarks.updateAnchor(note, anchor)
        val recents = snapshot.recents.updateAnchor(note, anchor)
        if (bookmarks === snapshot.bookmarks && recents === snapshot.recents) return
        snapshot = snapshot.copy(bookmarks = bookmarks, recents = recents)
        persist()
    }

    fun isBookmarked(note: BoundNote): Boolean =
        snapshot.bookmarks.any { it.note.sameIdentity(note) }

    fun toggleBookmark(node: NodeRecord) {
        if (!live.get()) return
        val note = node.bound()
        val existing = snapshot.bookmarks.indexOfFirst { it.note.sameIdentity(note) }
        val bookmarks =
            if (existing >= 0) {
                snapshot.bookmarks.filterIndexed { index, _ -> index != existing }
            } else {
                val anchor =
                    snapshot.recents.firstOrNull { it.note.sameIdentity(note) }?.anchor
                        ?: ReadingAnchor.Start
                (listOf(node.returnEntry(anchor)) + snapshot.bookmarks).take(MAX_BOOKMARKS)
            }
        snapshot = snapshot.copy(bookmarks = bookmarks)
        persist()
    }

    fun removeBookmark(entry: ReadingReturn) {
        mutate { it.copy(bookmarks = it.bookmarks.filterNot { held -> held.note.sameIdentity(entry.note) }) }
    }

    fun removeRecent(entry: ReadingReturn) {
        mutate { it.copy(recents = it.recents.filterNot { held -> held.note.sameIdentity(entry.note) }) }
    }

    fun clearBookmarks() {
        mutate { it.copy(bookmarks = emptyList()) }
    }

    fun clearRecents() {
        mutate { it.copy(recents = emptyList()) }
    }

    fun clearAll() {
        mutate { ReadingReturnsSnapshot() }
    }

    private fun mutate(transform: (ReadingReturnsSnapshot) -> ReadingReturnsSnapshot) {
        if (!live.get()) return
        val changed = transform(snapshot)
        if (changed == snapshot) return
        snapshot = changed
        persist()
    }

    private fun reconcile() {
        val entries = (snapshot.bookmarks + snapshot.recents).distinctReturns()
        if (entries.isEmpty()) return
        val serial = requests.incrementAndGet()
        Thread(
                {
                    val answers =
                        runCatching {
                            resolverFactory.open(ready).use { resolver ->
                                entries.map { entry ->
                                    runCatching { resolver.resolve(entry.note) }
                                        .fold(
                                            onSuccess = { ReconciledReturn(entry.note, node = it) },
                                            onFailure = { ReconciledReturn(entry.note, failed = true) },
                                        )
                                }
                            }
                        }.getOrElse {
                            entries.map { ReconciledReturn(it.note, failed = true) }
                        }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        accept(answers)
                    }
                },
                RESOLVER_THREAD,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun accept(answers: List<ReconciledReturn>) {
        fun resolve(entry: ReadingReturn): ReadingReturn {
            val answer = answers.firstOrNull { it.original.sameIdentity(entry.note) }
                ?: return entry
            if (answer.failed) return entry.copy(availability = ReadingReturnAvailability.Unavailable)
            val node = answer.node
                ?: return entry.copy(availability = ReadingReturnAvailability.Missing)
            return node.returnEntry(entry.anchor)
        }
        snapshot =
            ReadingReturnsSnapshot(
                bookmarks = snapshot.bookmarks.map(::resolve).distinctReturns(),
                recents = snapshot.recents.map(::resolve).distinctReturns(),
            )
        persist()
    }

    private fun NodeRecord.bound(): BoundNote =
        BoundNote(ready.binding, nodeKey, explicitId, filePath)

    private fun NodeRecord.returnEntry(anchor: ReadingAnchor): ReadingReturn =
        ReadingReturn(
            note = bound(),
            title = title.takeIf(::isKeyText) ?: filePath.takeIf(::isKeyText) ?: nodeKey,
            anchor = anchor,
            availability = ReadingReturnAvailability.Available,
        )

    private fun List<ReadingReturn>.updateAnchor(
        note: BoundNote,
        anchor: ReadingAnchor,
    ): List<ReadingReturn> {
        var changed = false
        val updated =
            map { entry ->
                if (entry.note.sameIdentity(note) && entry.anchor != anchor) {
                    changed = true
                    entry.copy(anchor = anchor)
                } else {
                    entry
                }
            }
        return if (changed) updated else this
    }

    private fun persist() {
        sink.save(ready.binding.source, snapshot)
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
    }

    private companion object {
        const val RESOLVER_THREAD = "slipbox-reading-returns-resolver"
    }
}

@Composable
internal fun rememberReadingReturnsState(ready: ReadySource): ReadingReturnsState? {
    val context = LocalContext.current.applicationContext
    val store = remember(context) { packagedReadingReturnsStore(context) }
    val sink = remember(store) { store?.let(::AsyncReadingReturnsSink) }
    var restored by remember(ready.binding) { mutableStateOf<ReadingReturnsSnapshot?>(null) }
    LaunchedEffect(store, ready.binding) {
        restored =
            if (store == null) ReadingReturnsSnapshot()
            else withContext(Dispatchers.IO) { sink?.load(ready.binding) ?: store.load(ready.binding) }
    }
    val state =
        restored?.let { snapshot ->
            remember(ready.binding, ready.contentRoot, ready.database, snapshot, sink) {
                ReadingReturnsState(ready, snapshot, sink ?: ReadingReturnsSink.None)
            }
        }
    DisposableEffect(state) { onDispose { state?.close() } }
    DisposableEffect(sink) { onDispose { sink?.close() } }
    return state
}

private fun packagedReadingReturnsStore(context: Context): ReadingReturnsStore? =
    when (val root = SlipboxVault.privateRoot(context)) {
        is VaultOutcome.Completed -> ReadingReturnsStore(root.value)
        is VaultOutcome.Failed -> null
    }
