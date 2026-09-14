/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.security.StoredCredential
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

internal object GithubEndpoint {

    const val DEVICE_CODE = "https://github.com/login/device/code"

    const val ACCESS_TOKEN = "https://github.com/login/oauth/access_token"

    const val USER = "https://api.github.com/user"

    const val INSTALLATIONS = "https://api.github.com/user/installations"
}

internal class RecordedTransport : AuthorizationTransport {

    private val queued = mutableMapOf<String, ArrayDeque<AuthorizationReply>>()

    private val exchanged = CopyOnWriteArrayList<AuthorizationRequest>()


    var beforeExchange: (AuthorizationRequest) -> Unit = {}


    val requests: List<AuthorizationRequest>
        get() = exchanged.toList()


    fun answers(url: String, vararg replies: AuthorizationReply): RecordedTransport {
        synchronized(queued) { queued.getOrPut(url) { ArrayDeque() }.addAll(replies) }
        return this
    }


    fun requestsFor(url: String): List<AuthorizationRequest> = requests.filter { it.url == url }


    fun exchangesOf(url: String): Int = requestsFor(url).size

    override fun exchange(request: AuthorizationRequest): AuthorizationReply {
        exchanged.add(request)
        beforeExchange(request)
        return synchronized(queued) {
            val replies =
                queued[request.url] ?: throw AssertionError("no reply prepared for ${request.url}")
            if (replies.size > 1) replies.removeFirst() else replies.first()
        }
    }
}

internal class SteppedClock(private val epoch: Long = EPOCH_SECONDS) : AuthorizationClock {

    private val elapsed = AtomicLong()

    private val waited = CopyOnWriteArrayList<Long>()


    var waitsHonoured: Int = Int.MAX_VALUE


    var pause: CountDownLatch? = null


    val waits: List<Long>
        get() = waited.toList()

    override fun elapsedMillis(): Long = elapsed.get()

    override fun epochSeconds(): Long = epoch

    override fun waitFor(millis: Long): Boolean {
        if (waited.size >= waitsHonoured) {
            return false
        }
        waited.add(millis)
        elapsed.addAndGet(millis)
        val parked = pause ?: return true
        return try {
            parked.await()
            true
        } catch (interrupted: InterruptedException) {
            false
        }
    }

    internal companion object {


        const val EPOCH_SECONDS = 1_780_000_000L
    }
}

internal class RecordingListener : AuthorizationListener {

    private val grants = CopyOnWriteArrayList<DeviceGrant>()

    private val outcomes = CopyOnWriteArrayList<AuthorizationOutcome>()


    val waiting: List<DeviceGrant>
        get() = grants.toList()


    val settled: List<AuthorizationOutcome>
        get() = outcomes.toList()


    val outcome: AuthorizationOutcome
        get() =
            outcomes.singleOrNull()
                ?: throw AssertionError("expected one outcome, got ${outcomes.toList()}")

    override fun onVerificationWaiting(grant: DeviceGrant) {
        grants.add(grant)
    }

    override fun onSettled(outcome: AuthorizationOutcome) {
        outcomes.add(outcome)
    }
}

internal class QueuedDelivery : AuthorizationDelivery {

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


    val posted: Int
        get() = lock.withLock { posts }


    fun awaitPosts(count: Int, millis: Long = WAIT_MILLIS): Boolean {
        val deadline = System.currentTimeMillis() + millis
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

internal val DirectDelivery: AuthorizationDelivery = AuthorizationDelivery { it() }

internal fun testApp(
    clientId: String = CLIENT_ID,
    installationUrl: String = INSTALLATION_URL,
): GithubApp = requireNotNull(GithubApp.of(clientId, installationUrl))

internal fun testGrant(
    userCode: String = USER_CODE,
    verificationUri: String = VERIFICATION_URI,
    expiresInSeconds: Long = LIFETIME_SECONDS,
    intervalSeconds: Long = INTERVAL_SECONDS,
    deviceCode: String = DEVICE_CODE,
): DeviceGrant =
    DeviceGrant(userCode, verificationUri, expiresInSeconds, intervalSeconds, deviceCode)

internal fun testAuthorization(
    login: String = LOGIN,
    accountId: String = ACCOUNT_ID.toString(),
    installations: Int = 1,
    selection: RepositorySelection = RepositorySelection.Selected,
): GithubAuthorization =
    GithubAuthorization(
        VerifiedAccount(accountId, login),
        InstallationAccess(installations, selection),
        StoredCredential(
            accessToken = ACCESS_TOKEN,
            refreshToken = REFRESH_TOKEN,
            expiresAtEpochSeconds = SteppedClock.EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS,
        ),
    )

internal fun deviceCodeBody(
    deviceCode: String = DEVICE_CODE,
    userCode: String = USER_CODE,
    verificationUri: String = VERIFICATION_URI,
    expiresIn: Long? = LIFETIME_SECONDS,
    interval: Long? = INTERVAL_SECONDS,
): String =
    fields(
        "device_code" to quoted(deviceCode),
        "user_code" to quoted(userCode),
        "verification_uri" to quoted(verificationUri),
        "expires_in" to expiresIn?.toString(),
        "interval" to interval?.toString(),
    )

internal fun grantBody(
    accessToken: String = ACCESS_TOKEN,
    tokenType: String? = "bearer",
    expiresIn: Long? = TOKEN_LIFETIME_SECONDS,
    refreshToken: String? = REFRESH_TOKEN,
): String =
    fields(
        "access_token" to quoted(accessToken),
        "token_type" to tokenType?.let { quoted(it) },
        "expires_in" to expiresIn?.toString(),
        "refresh_token" to refreshToken?.let { quoted(it) },
    )

internal fun errorBody(error: String): String = fields("error" to quoted(error))

internal fun accountBody(login: String? = LOGIN, id: Long? = ACCOUNT_ID): String =
    fields("login" to login?.let { quoted(it) }, "id" to id?.toString())

internal fun installationsBody(count: Int?, selections: List<String> = emptyList()): String =
    fields(
        "total_count" to count?.toString(),
        "installations" to
            selections.joinToString(",", "[", "]") {
                fields("repository_selection" to quoted(it))
            },
    )

internal fun ok(body: String): AuthorizationReply = AuthorizationReply.Answered(200, body)

internal fun refused(body: String, status: Int = 400): AuthorizationReply =
    AuthorizationReply.Answered(status, body)

internal fun pending(): AuthorizationReply = refused(errorBody("authorization_pending"))

internal fun grantingTransport(
    deviceCode: AuthorizationReply = ok(deviceCodeBody()),
    polls: List<AuthorizationReply> = listOf(ok(grantBody())),
    account: AuthorizationReply = ok(accountBody()),
    installations: AuthorizationReply = ok(installationsBody(1, listOf("selected"))),
): RecordedTransport =
    RecordedTransport()
        .answers(GithubEndpoint.DEVICE_CODE, deviceCode)
        .answers(GithubEndpoint.ACCESS_TOKEN, *polls.toTypedArray())
        .answers(GithubEndpoint.USER, account)
        .answers(GithubEndpoint.INSTALLATIONS, installations)

internal fun AuthorizationOutcome.authorized(): GithubAuthorization =
    when (this) {
        is AuthorizationOutcome.Authorized -> authorization
        else -> throw AssertionError("expected an authorization, got $this")
    }

internal fun AuthorizationOutcome.unavailable(): AuthorizationFault =
    when (this) {
        is AuthorizationOutcome.Unavailable -> fault
        else -> throw AssertionError("expected a fault, got $this")
    }

internal fun DeviceCodeOutcome.requested(): DeviceGrant =
    when (this) {
        is DeviceCodeOutcome.Requested -> grant
        is DeviceCodeOutcome.Unavailable -> throw AssertionError("expected a grant, got $fault")
    }

internal fun DeviceCodeOutcome.refusal(): AuthorizationFault =
    when (this) {
        is DeviceCodeOutcome.Requested -> throw AssertionError("expected a fault, got $grant")
        is DeviceCodeOutcome.Unavailable -> fault
    }

private fun fields(vararg entries: Pair<String, String?>): String =
    entries
        .filter { it.second != null }
        .joinToString(",", "{", "}") { "${quoted(it.first)}:${it.second}" }

private fun quoted(value: String): String =
    "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

internal const val CLIENT_ID = "Iv23liSlipboxTestClient"

internal const val INSTALLATION_URL = "https://github.com/apps/slipbox-test/installations/new"

internal const val USER_CODE = "WDJB-MJHT"

internal const val VERIFICATION_URI = "https://github.com/login/device"

internal const val DEVICE_CODE = "synthetic-device-code-0123456789abcdef"

internal const val ACCESS_TOKEN = "synthetic-access-token-0123456789abcdef"

internal const val REFRESH_TOKEN = "synthetic-refresh-token-0123456789abcdef"

internal const val LOGIN = "synthetic-account"

internal const val ACCOUNT_ID = 42_000_001L

internal const val LIFETIME_SECONDS = 900L

internal const val INTERVAL_SECONDS = 5L

internal const val TOKEN_LIFETIME_SECONDS = 28_800L

internal const val WAIT_MILLIS = 10_000L
