/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import android.os.Handler
import android.os.Looper
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal fun interface GithubDelivery {

    fun post(action: () -> Unit)

    companion object {

        val MainThread: GithubDelivery =
            object : GithubDelivery {

                private val handler by lazy { Handler(Looper.getMainLooper()) }

                override fun post(action: () -> Unit) {
                    handler.post(action)
                }
            }
    }
}

fun interface GithubRead<out T> {

    fun read(provider: GithubProvider): GithubOutcome<T>
}

/** Owns one traversal; replacement and disposal cancel requests and queued delivery. */
class GithubBrowsing internal constructor(
    private val authorization: GithubAuthorization,
    private val app: GithubApp?,
    private val transport: GithubApiTransport,
    private val delivery: GithubDelivery,
) : AutoCloseable, DefaultLifecycleObserver {

    private val traversals = AtomicLong()

    private val active = AtomicReference<GithubTraversal?>(null)

    private val closed = AtomicBoolean()

    fun <T> browse(
        read: GithubRead<T>,
        listener: (GithubOutcome<T>) -> Unit,
    ): GithubTraversal {
        val traversal =
            GithubTraversal(traversals.incrementAndGet(), delivery) { settled ->
                active.compareAndSet(settled, null)
            }
        active.getAndSet(traversal)?.withdraw()
        if (closed.get()) {
            traversal.withdraw()
            return traversal
        }
        val provider =
            GithubProvider(authorization, app, transport) { traversal.isLive }
        traversal.start(provider, read, listener)
        return traversal
    }

    fun running(): GithubTraversal? = active.get()

    override fun close() {
        closed.set(true)
        active.getAndSet(null)?.withdraw()
    }

    override fun onDestroy(owner: LifecycleOwner) {
        close()
    }

    companion object {

        fun packaged(authorization: GithubAuthorization): GithubBrowsing =
            GithubBrowsing(
                authorization,
                GithubApp.packaged(),
                HttpsGithubApiTransport(),
                GithubDelivery.MainThread,
            )
    }
}

class GithubTraversal internal constructor(
    internal val serial: Long,
    private val delivery: GithubDelivery,
    private val onSettled: (GithubTraversal) -> Unit,
) {

    private val state = AtomicReference(State.Running)

    private val worker = AtomicReference<Thread?>(null)

    val isLive: Boolean
        get() = state.get() == State.Running

    override fun toString(): String = "GithubTraversal($serial, ${state.get()})"

    fun cancel() {
        withdraw()
    }

    internal fun withdraw() {
        if (state.compareAndSet(State.Running, State.Withdrawn)) {
            worker.getAndSet(null)?.interrupt()
            onSettled(this)
        }
    }

    internal fun <T> start(
        provider: GithubProvider,
        read: GithubRead<T>,
        listener: (GithubOutcome<T>) -> Unit,
    ) {
        val thread =
            Thread({
                val outcome =
                    try {
                        read.read(provider)
                    } catch (fault: Throwable) {
                        GithubOutcome.Refused(
                            GithubRefusal.TransportFailed(
                                GithubStage.Traversal,
                                fault.javaClass.name,
                            ),
                        )
                    }
                settle(outcome, listener)
            }, THREAD_NAME)
        thread.isDaemon = true
        worker.set(thread)
        if (isLive) {
            thread.start()
        }
    }

    private fun <T> settle(outcome: GithubOutcome<T>, listener: (GithubOutcome<T>) -> Unit) {
        delivery.post {
            if (state.compareAndSet(State.Running, State.Settled)) {
                worker.set(null)
                onSettled(this)
                listener(outcome)
            }
        }
    }

    private enum class State {
        Running,
        Withdrawn,
        Settled,
    }

    private companion object {

        const val THREAD_NAME = "slipbox-github"
    }
}
