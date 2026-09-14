/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.os.Handler
import android.os.Looper
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

internal fun interface AuthorizationDelivery {

    fun post(action: () -> Unit)

    companion object {


        val MainThread: AuthorizationDelivery =
            object : AuthorizationDelivery {

                private val handler by lazy { Handler(Looper.getMainLooper()) }

                override fun post(action: () -> Unit) {
                    handler.post(action)
                }
            }
    }
}

/** Owns one attempt; replacement and disposal withdraw its polling and queued delivery. */
class GithubAuthorizationOwner internal constructor(
    private val app: GithubApp?,
    private val transport: AuthorizationTransport,
    private val clock: AuthorizationClock,
    private val delivery: AuthorizationDelivery,
) : AutoCloseable, DefaultLifecycleObserver {

    private val attempts = AtomicLong()

    private val active = AtomicReference<AuthorizationAttempt?>(null)

    private val closed = AtomicBoolean()


    val isConfigured: Boolean = app != null


    fun authorize(listener: AuthorizationListener): AuthorizationAttempt {
        val attempt =
            AuthorizationAttempt(attempts.incrementAndGet(), listener, delivery) { settled ->
                active.compareAndSet(settled, null)
            }
        active.getAndSet(attempt)?.withdraw()
        if (closed.get()) {
            attempt.withdraw()
            return attempt
        }
        val registration = app
        if (registration == null) {
            attempt.settle(
                AuthorizationOutcome.Unavailable(AuthorizationFault.ConfigurationUnavailable),
            )
            return attempt
        }
        attempt.start(GithubDeviceFlow(registration, transport, clock))
        return attempt
    }


    fun running(): AuthorizationAttempt? = active.get()


    override fun close() {
        closed.set(true)
        active.getAndSet(null)?.withdraw()
    }

    override fun onDestroy(owner: LifecycleOwner) {
        close()
    }

    companion object {


        fun packaged(): GithubAuthorizationOwner =
            GithubAuthorizationOwner(
                GithubApp.packaged(),
                HttpsAuthorizationTransport(),
                SystemAuthorizationClock,
                AuthorizationDelivery.MainThread,
            )
    }
}

/** Background authorization attempt with callbacks on the owner's delivery thread. */
class AuthorizationAttempt internal constructor(
    internal val serial: Long,
    private val listener: AuthorizationListener,
    private val delivery: AuthorizationDelivery,
    private val onSettled: (AuthorizationAttempt) -> Unit,
) {

    private val state = AtomicReference(State.Running)

    private val worker = AtomicReference<Thread?>(null)


    val isLive: Boolean
        get() = state.get() == State.Running

    override fun toString(): String = "AuthorizationAttempt($serial, ${state.get()})"


    fun cancel() {
        withdraw()
    }

    internal fun withdraw() {
        if (state.compareAndSet(State.Running, State.Withdrawn)) {
            worker.getAndSet(null)?.interrupt()
            onSettled(this)
        }
    }


    internal fun start(flow: GithubDeviceFlow) {
        val thread =
            Thread({
                try {
                    run(flow)
                } catch (fault: Throwable) {
                    settle(
                        AuthorizationOutcome.Unavailable(
                            AuthorizationFault.AttemptFailed(fault.javaClass.name),
                        ),
                    )
                }
            }, THREAD_NAME)
        thread.isDaemon = true
        worker.set(thread)
        if (isLive) {
            thread.start()
        }
    }

    private fun run(flow: GithubDeviceFlow) {
        if (!isLive) {
            return
        }
        val grant =
            when (val requested = flow.requestGrant()) {
                is DeviceCodeOutcome.Unavailable ->
                    return settle(AuthorizationOutcome.Unavailable(requested.fault))
                is DeviceCodeOutcome.Requested -> requested.grant
            }
        report { listener.onVerificationWaiting(grant) }
        settle(flow.awaitGrant(grant) { isLive })
    }


    internal fun settle(outcome: AuthorizationOutcome) {
        report {
            if (state.compareAndSet(State.Running, State.Settled)) {
                worker.set(null)
                onSettled(this)
                listener.onSettled(outcome)
            }
        }
    }


    private fun report(action: () -> Unit) {
        delivery.post {
            if (isLive) {
                action()
            }
        }
    }

    private enum class State {
        Running,
        Withdrawn,
        Settled,
    }

    private companion object {

        const val THREAD_NAME = "slipbox-authorization"
    }
}
