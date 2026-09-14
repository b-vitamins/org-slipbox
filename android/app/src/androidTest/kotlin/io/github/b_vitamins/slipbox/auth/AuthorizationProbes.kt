/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.syntheticToken
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

internal object DeviceEndpoint {

    const val DEVICE_CODE = "https://github.com/login/device/code"

    const val ACCESS_TOKEN = "https://github.com/login/oauth/access_token"
}

internal class PendingTransport : AuthorizationTransport {

    private val exchanged = CopyOnWriteArrayList<AuthorizationRequest>()


    val requests: List<AuthorizationRequest>
        get() = exchanged.toList()


    fun exchangesOf(url: String): Int = requests.count { it.url == url }

    override fun exchange(request: AuthorizationRequest): AuthorizationReply {
        exchanged.add(request)
        return when (request.url) {
            DeviceEndpoint.DEVICE_CODE -> AuthorizationReply.Answered(OK, deviceCodeBody())
            DeviceEndpoint.ACCESS_TOKEN -> AuthorizationReply.Answered(REFUSED, pendingBody())
            else -> throw AssertionError("nothing prepared an exchange of ${request.url}")
        }
    }

    private companion object {

        const val OK = 200

        const val REFUSED = 400
    }
}

internal class ParkingClock : AuthorizationClock {

    private val elapsed = AtomicLong()

    private val parked = CountDownLatch(1)

    private val released = CountDownLatch(1)

    override fun elapsedMillis(): Long = elapsed.get()

    override fun epochSeconds(): Long = EPOCH_SECONDS

    override fun waitFor(millis: Long): Boolean {
        elapsed.addAndGet(millis)
        parked.countDown()
        return try {
            released.await(PARK_MILLIS, TimeUnit.MILLISECONDS)
            true
        } catch (interrupted: InterruptedException) {
            false
        }
    }


    fun awaitPolling(): Boolean = parked.await(ANSWER_MILLIS, TimeUnit.MILLISECONDS)


    fun release() {
        released.countDown()
    }

    private companion object {


        const val PARK_MILLIS = 30_000L
    }
}

internal class PostedDelivery : AuthorizationDelivery {

    private val lock = ReentrantLock()

    private val arrived = lock.newCondition()

    private val pending = ArrayDeque<() -> Unit>()

    private var posts = 0

    override fun post(action: () -> Unit) {
        lock.withLock {
            pending.addLast(action)
            posts++
            arrived.signalAll()
        }
    }


    fun awaitPosts(count: Int): Boolean {
        val deadline = System.currentTimeMillis() + ANSWER_MILLIS
        lock.withLock {
            while (posts < count) {
                val left = deadline - System.currentTimeMillis()
                if (left <= 0) {
                    return false
                }
                arrived.await(left, TimeUnit.MILLISECONDS)
            }
            return true
        }
    }


    fun drain(): Int {
        var ran = 0
        while (true) {
            val action = lock.withLock { pending.removeFirstOrNull() } ?: return ran
            action()
            ran++
        }
    }
}

internal class ProbeListener : AuthorizationListener {

    private val grants = CopyOnWriteArrayList<DeviceGrant>()

    private val outcomes = CopyOnWriteArrayList<AuthorizationOutcome>()


    val waiting: List<DeviceGrant>
        get() = grants.toList()


    val settled: List<AuthorizationOutcome>
        get() = outcomes.toList()

    override fun onVerificationWaiting(grant: DeviceGrant) {
        grants.add(grant)
    }

    override fun onSettled(outcome: AuthorizationOutcome) {
        outcomes.add(outcome)
    }
}

internal class HandoffContext(base: Context, private val accepts: Boolean = true) :
    ContextWrapper(base) {

    private val handed = mutableListOf<Intent>()


    val offered: List<Intent>
        get() = handed.toList()

    override fun getApplicationContext(): Context = this

    override fun startActivity(intent: Intent) {
        handed.add(intent)
        if (!accepts) {
            throw ActivityNotFoundException("no browser on this probe")
        }
    }
}

internal fun syntheticGrant(): DeviceGrant =
    DeviceGrant(
        userCode = PLACEHOLDER_CODE,
        verificationUri = VERIFICATION_PAGE,
        expiresInSeconds = LIFETIME_SECONDS,
        intervalSeconds = INTERVAL_SECONDS,
        deviceCode = "probe-device-code",
    )

internal fun syntheticAuthorization(
    login: String = PROBE_LOGIN,
    accountId: String = PROBE_ACCOUNT_ID,
    installations: Int = 1,
    selection: RepositorySelection = RepositorySelection.Selected,
): GithubAuthorization =
    GithubAuthorization(
        VerifiedAccount(accountId, login),
        InstallationAccess(installations, selection),
        StoredCredential(
            accessToken = syntheticToken("access"),
            refreshToken = syntheticToken("refresh"),
            expiresAtEpochSeconds = EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS,
        ),
    )

private fun deviceCodeBody(): String =
    "{\"device_code\":\"probe-device-code\",\"user_code\":\"$PLACEHOLDER_CODE\"," +
        "\"verification_uri\":\"$VERIFICATION_PAGE\",\"expires_in\":$LIFETIME_SECONDS," +
        "\"interval\":$INTERVAL_SECONDS}"

private fun pendingBody(): String = """{"error":"authorization_pending"}"""

internal const val PLACEHOLDER_CODE = "XXXX-XXXX"

internal const val VERIFICATION_PAGE = "https://github.com/login/device"

internal const val INSTALLATION_PAGE = "https://github.com/apps/slipbox-probe/installations/new"

internal const val PROBE_CLIENT_ID = "Iv23liSlipboxProbeClient"

internal const val PROBE_LOGIN = "synthetic-account"

internal const val PROBE_ACCOUNT_ID = "42000001"

internal const val LIFETIME_SECONDS = 900L

internal const val INTERVAL_SECONDS = 5L

internal const val TOKEN_LIFETIME_SECONDS = 28_800L

internal const val EPOCH_SECONDS = 1_780_000_000L

internal const val ANSWER_MILLIS = 30_000L
