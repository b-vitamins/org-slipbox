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
import io.github.b_vitamins.slipbox.engine.EngineAnswer
import io.github.b_vitamins.slipbox.engine.NodeRecord
import io.github.b_vitamins.slipbox.engine.RandomNodeResult
import io.github.b_vitamins.slipbox.engine.ReadOperation
import io.github.b_vitamins.slipbox.engine.SessionContext
import io.github.b_vitamins.slipbox.engine.SlipboxEngineHost
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.ReadySource
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal enum class RandomNotePhase {
    Idle,
    Loading,
    Empty,
    Failed,
}

internal interface RandomNoteSource : AutoCloseable {

    fun choose(): RandomNodeResult
}

internal fun interface RandomNoteSourceFactory {

    fun open(ready: ReadySource): RandomNoteSource
}

private object NativeRandomNoteSourceFactory : RandomNoteSourceFactory {

    override fun open(ready: ReadySource): RandomNoteSource {
        val host = SlipboxEngineHost.packaged()
        return try {
            val session =
                host
                    .openRead(
                        ready.binding,
                        SessionContext(root = ready.contentRoot, database = ready.database),
                    )
                    .await()
            object : RandomNoteSource {
                override fun choose(): RandomNodeResult {
                    val answer = session.answer(ReadOperation.RandomNode).await()
                    return (answer as EngineAnswer.RandomNode).result
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

/** Owns the bounded random-note request stream for one source generation. */
@Stable
internal class RandomNoteState(
    private val ready: ReadySource,
    private val factory: RandomNoteSourceFactory = NativeRandomNoteSourceFactory,
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {

    private val live = AtomicBoolean(true)
    private val loading = AtomicBoolean()
    private val requests = AtomicLong()
    private val source = AtomicReference<RandomNoteSource?>()

    var phase: RandomNotePhase by mutableStateOf(RandomNotePhase.Idle)
        private set

    fun choose(onChosen: (NodeRecord) -> Unit) {
        if (!live.get() || !loading.compareAndSet(false, true)) return
        phase = RandomNotePhase.Loading
        val serial = requests.incrementAndGet()
        Thread(
                {
                    val outcome = runCatching { (source.get() ?: openSource()).choose() }
                    delivery.post {
                        if (!live.get() || requests.get() != serial) return@post
                        loading.set(false)
                        outcome.fold(
                            onSuccess = { result ->
                                val node = result.node
                                if (node == null) {
                                    phase = RandomNotePhase.Empty
                                } else {
                                    phase = RandomNotePhase.Idle
                                    onChosen(node)
                                }
                            },
                            onFailure = {
                                source.getAndSet(null)?.close()
                                phase = RandomNotePhase.Failed
                            },
                        )
                    }
                },
                WORKER_NAME,
            )
            .apply {
                isDaemon = true
                start()
            }
    }

    private fun openSource(): RandomNoteSource {
        val opened = factory.open(ready)
        if (!live.get()) {
            opened.close()
            throw IllegalStateException("the random-note source was closed while opening")
        }
        if (!source.compareAndSet(null, opened)) opened.close()
        return checkNotNull(source.get())
    }

    override fun close() {
        if (!live.compareAndSet(true, false)) return
        requests.incrementAndGet()
        source.getAndSet(null)?.close()
    }

    private companion object {
        const val WORKER_NAME = "slipbox-random-note"
    }
}

@Composable
internal fun rememberRandomNoteState(ready: ReadySource): RandomNoteState {
    val state =
        remember(ready.binding, ready.contentRoot, ready.database) {
            RandomNoteState(ready)
        }
    DisposableEffect(state) { onDispose(state::close) }
    return state
}
