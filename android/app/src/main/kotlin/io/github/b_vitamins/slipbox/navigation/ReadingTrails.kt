/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.engine.GenerationBinding
import io.github.b_vitamins.slipbox.security.AtomicVaultRecordFile
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.sync.PackagedSourceRefreshStorage
import java.io.File
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.util.concurrent.Executors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

private const val READING_TRAIL_VERSION = 1
private const val MAX_READING_TRAIL_ENTRIES = 64
private const val MAX_READING_TRAIL_BYTES = 256 * 1024

internal fun interface ReadingTrailSink {
    fun save(source: String, routes: List<SlipboxRoute.Reader>)

    companion object {
        val None = ReadingTrailSink { _, _ -> }
    }
}

internal data class ReadingTrailSession(
    val restored: List<SlipboxRoute>,
    val sink: ReadingTrailSink,
)

@Serializable
private data class StoredReadingTrail(
    val version: Int = READING_TRAIL_VERSION,
    val source: String,
    val entries: List<StoredReadingEntry>,
)

@Serializable
private data class StoredReadingEntry(
    val nodeKey: String,
    val explicitId: String? = null,
    val filePath: String = "",
    val anchor: ReadingAnchor = ReadingAnchor.Start,
)

/** Strict, bounded source-owned state. Note text and index generations never cross this boundary. */
internal class ReadingTrailStore(private val privateRoot: File) {

    fun load(binding: GenerationBinding): List<SlipboxRoute> {
        if (!binding.isCanonical()) return emptyList()
        val file = PackagedSourceRefreshStorage.readingTrailFile(privateRoot, binding.source)
            ?: return emptyList()
        val bytes = boundedBytes(file) ?: return emptyList()
        val text = strictUtf8(bytes) ?: return emptyList()
        val stored =
            try {
                FORMAT.decodeFromString<StoredReadingTrail>(text)
            } catch (_: SerializationException) {
                return emptyList()
            }
        if (stored.version != READING_TRAIL_VERSION ||
            stored.source != binding.source ||
            stored.entries.size > MAX_READING_TRAIL_ENTRIES
        ) {
            return emptyList()
        }
        return stored.entries.mapNotNull { entry ->
            val route =
                SlipboxRoute.Reader(
                    note =
                        BoundNote(
                            binding = binding,
                            nodeKey = entry.nodeKey,
                            explicitId = entry.explicitId,
                            filePath = entry.filePath,
                        ),
                    anchor = entry.anchor,
                )
            route.takeIf(SlipboxRoute::isCanonical)
        }
    }

    fun save(source: String, routes: List<SlipboxRoute.Reader>) {
        if (!isSourceIdentity(source)) return
        val selected =
            routes
                .asSequence()
                .filter { it.note.binding.source == source && it.isCanonical() }
                .takeLast(MAX_READING_TRAIL_ENTRIES)
                .map { route ->
                    StoredReadingEntry(
                        nodeKey = route.note.nodeKey,
                        explicitId = route.note.explicitId,
                        filePath = route.note.filePath,
                        anchor = route.anchor,
                    )
                }
                .toList()
        val existing = PackagedSourceRefreshStorage.readingTrailFile(privateRoot, source) ?: return
        if (selected.isEmpty()) {
            AtomicVaultRecordFile(existing).delete()
            return
        }
        val encoded =
            FORMAT.encodeToString(
                StoredReadingTrail(source = source, entries = selected),
            ).toByteArray(Charsets.UTF_8)
        if (encoded.size > MAX_READING_TRAIL_BYTES) return
        val destination =
            PackagedSourceRefreshStorage.prepareReadingTrailFile(privateRoot, source) ?: return
        AtomicVaultRecordFile(destination).replace(encoded)
    }

    private fun boundedBytes(file: File): ByteArray? =
        try {
            if (!file.isFile || file.length() !in 1..MAX_READING_TRAIL_BYTES.toLong()) null
            else file.readBytes().takeIf { it.size <= MAX_READING_TRAIL_BYTES }
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

private class AsyncReadingTrailSink(private val store: ReadingTrailStore) :
    ReadingTrailSink,
    AutoCloseable {

    private val executor =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "slipbox-reading-trail").apply { isDaemon = true }
        }
    private val pending = mutableMapOf<String, List<SlipboxRoute.Reader>>()
    private var draining = false

    override fun save(source: String, routes: List<SlipboxRoute.Reader>) {
        synchronized(pending) {
            pending[source] = routes.toList()
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

    fun load(binding: GenerationBinding): List<SlipboxRoute> =
        runCatching { executor.submit<List<SlipboxRoute>> { store.load(binding) }.get() }
            .getOrDefault(emptyList())

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

@Composable
internal fun rememberReadingTrailSession(binding: GenerationBinding): ReadingTrailSession? {
    val context = LocalContext.current.applicationContext
    val store = remember(context) { packagedReadingTrailStore(context) }
    val sink = remember(store) { store?.let(::AsyncReadingTrailSink) }
    var restored by remember(binding) { mutableStateOf<List<SlipboxRoute>?>(null) }
    LaunchedEffect(store, binding) {
        restored =
            if (store == null) emptyList()
            else withContext(Dispatchers.IO) { sink?.load(binding) ?: store.load(binding) }
    }
    DisposableEffect(sink) { onDispose { sink?.close() } }
    return restored?.let { ReadingTrailSession(it, sink ?: ReadingTrailSink.None) }
}

private fun packagedReadingTrailStore(context: Context): ReadingTrailStore? =
    when (val root = SlipboxVault.privateRoot(context)) {
        is VaultOutcome.Completed -> ReadingTrailStore(root.value)
        is VaultOutcome.Failed -> null
    }

private fun <T> Sequence<T>.takeLast(limit: Int): List<T> {
    val retained = ArrayDeque<T>(limit)
    for (entry in this) {
        if (retained.size == limit) retained.removeFirst()
        retained.addLast(entry)
    }
    return retained.toList()
}
