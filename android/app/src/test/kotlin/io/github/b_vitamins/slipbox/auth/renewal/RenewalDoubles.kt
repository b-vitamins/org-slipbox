/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.ACCESS_TOKEN
import io.github.b_vitamins.slipbox.auth.ACCOUNT_ID
import io.github.b_vitamins.slipbox.auth.AuthorizationClock
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.LOGIN
import io.github.b_vitamins.slipbox.auth.REFRESH_TOKEN
import io.github.b_vitamins.slipbox.auth.SteppedClock
import io.github.b_vitamins.slipbox.auth.TOKEN_LIFETIME_SECONDS
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.auth.WAIT_MILLIS
import io.github.b_vitamins.slipbox.auth.grantBody
import io.github.b_vitamins.slipbox.auth.testApp
import io.github.b_vitamins.slipbox.security.AtomicRecordFile
import io.github.b_vitamins.slipbox.security.CLOSE_TIMEOUT_MILLIS
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.FakeRecordFile
import io.github.b_vitamins.slipbox.security.FakeRecordSource
import io.github.b_vitamins.slipbox.security.FakeVaultKeys
import io.github.b_vitamins.slipbox.security.ForegroundThread
import io.github.b_vitamins.slipbox.security.OpenVaults
import io.github.b_vitamins.slipbox.security.PrivateStore
import io.github.b_vitamins.slipbox.security.PrivateStoragePolicy
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.VaultNamespace
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultScope
import io.github.b_vitamins.slipbox.security.completed
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

internal class MovingClock(epoch: Long = SteppedClock.EPOCH_SECONDS) : AuthorizationClock {

    @Volatile
    var epoch: Long = epoch

    override fun elapsedMillis(): Long = 0

    override fun epochSeconds(): Long = epoch

    override fun waitFor(millis: Long): Boolean = true
}

internal class TestRenewalStorage(root: File) : RenewalStorage {

    private val vaults = OpenVaults()

    private val files = mutableMapOf<VaultScope, FakeRecordFile>()

    private val sources = mutableMapOf<VaultScope, FakeRecordSource>()

    val keys: FakeVaultKeys = FakeVaultKeys()

    var beforeRead: () -> Unit = {}

    val privateStorage: PrivateStoragePolicy =
        PrivateStoragePolicy(root, VaultNamespace.Application)

    fun file(scope: VaultScope): FakeRecordFile =
        synchronized(files) { files.getOrPut(scope) { FakeRecordFile() } }

    override fun open(scope: VaultScope): VaultOutcome<CredentialVault> =
        vaults.open(scope = scope, keys = keys, file = file(scope), source = source(scope))

    override fun policy(): VaultOutcome<PrivateStoragePolicy> =
        VaultOutcome.Completed(privateStorage)

    fun keep(scope: VaultScope, credential: StoredCredential) {
        holding(scope) { it.reauthorize(credential).completed() }
    }

    fun stored(scope: VaultScope): StoredCredential? = holding(scope) { it.read().completed() }

    fun cached(scope: VaultScope): File {
        val file =
            privateStorage.fileFor(PrivateStore.Reading, scope, CACHED_NAME).completed()
        file.parentFile?.mkdirs()
        file.writeBytes(CACHED_BYTES)
        return file
    }

    fun closeAll() {
        vaults.closeAll()
    }

    private fun <T> holding(scope: VaultScope, work: (CredentialVault) -> T): T {
        val vault = open(scope).completed()
        return try {
            work(vault)
        } finally {
            vault.close()
            if (!vault.awaitClosed(CLOSE_TIMEOUT_MILLIS)) {
                throw AssertionError("a vault of this test never released its record")
            }
        }
    }

    private fun source(scope: VaultScope): FakeRecordSource =
        synchronized(sources) {
            sources.getOrPut(scope) {
                FakeRecordSource(
                    object : AtomicRecordFile by file(scope) {
                        override fun read(limit: Int): VaultOutcome<ByteArray?> {
                            beforeRead()
                            return file(scope).read(limit)
                        }
                    },
                )
            }
        }
}

internal class Answering<T : Any>(work: () -> T) {

    private val done = CountDownLatch(1)

    @Volatile
    private var answered: T? = null

    @Volatile
    private var failure: Throwable? = null

    private val thread =
        Thread {
            try {
                answered = work()
            } catch (fault: Throwable) {
                failure = fault
            } finally {
                done.countDown()
            }
        }

    fun start(): Answering<T> = also { thread.start() }

    fun awaitQueued() {
        val deadline = System.currentTimeMillis() + WAIT_MILLIS
        while (thread.state !in PARKED) {
            if (thread.state == Thread.State.TERMINATED) {
                throw AssertionError("a call of this test answered instead of queueing")
            }
            if (System.currentTimeMillis() > deadline) {
                throw AssertionError("a call of this test never queued")
            }
            Thread.sleep(POLL_MILLIS)
        }
    }

    fun answer(): T {
        if (!done.await(WAIT_MILLIS, TimeUnit.MILLISECONDS)) {
            throw AssertionError("a call of this test never answered")
        }
        failure?.let { throw AssertionError("a call of this test failed as ${it.javaClass.name}") }
        return requireNotNull(answered) { "a call of this test answered nothing" }
    }

    private companion object {

        val PARKED =
            setOf(Thread.State.WAITING, Thread.State.TIMED_WAITING, Thread.State.BLOCKED)

        const val POLL_MILLIS = 5L
    }
}

internal fun testRequest(
    sourceId: String = SOURCE_ID,
    accountId: String = ACCOUNT_ID.toString(),
    credentialRef: String = CREDENTIAL_REF,
    login: String = LOGIN,
): RenewalRequest = RenewalRequest(sourceId, VerifiedAccount(accountId, login), credentialRef)

internal fun testOwner(
    storage: TestRenewalStorage,
    transport: AuthorizationTransport,
    clock: AuthorizationClock = MovingClock(),
    request: RenewalRequest = testRequest(),
    app: GithubApp? = testApp(),
    foreground: ForegroundThread = ForegroundThread.None,
): CredentialRenewalOwner =
    CredentialRenewalOwner(request, app, storage, transport, clock, foreground)

internal fun scopeOf(request: RenewalRequest): VaultScope = request.scope().completed()

internal fun keptCredential(
    expiresAt: Long? = SteppedClock.EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS,
    accessToken: String = ACCESS_TOKEN,
    refreshToken: String? = REFRESH_TOKEN,
): StoredCredential = StoredCredential(accessToken, refreshToken, expiresAt)

internal fun rotatedBody(
    accessToken: String = ROTATED_ACCESS_TOKEN,
    refreshToken: String? = ROTATED_REFRESH_TOKEN,
    expiresIn: Long? = TOKEN_LIFETIME_SECONDS,
    tokenType: String? = "bearer",
): String = grantBody(accessToken, tokenType, expiresIn, refreshToken)

internal fun rotatedCredential(epoch: Long): StoredCredential =
    StoredCredential(ROTATED_ACCESS_TOKEN, ROTATED_REFRESH_TOKEN, epoch + TOKEN_LIFETIME_SECONDS)

internal const val SOURCE_ID = "source-1"

internal const val CREDENTIAL_REF = "credential-1"

internal const val ROTATED_ACCESS_TOKEN = "synthetic-rotated-access-0123456789abcdef"

internal const val ROTATED_REFRESH_TOKEN = "synthetic-rotated-refresh-0123456789abcdef"

internal const val CACHED_NAME = "reading.bin"

private val CACHED_BYTES = "synthetic-reading-state".toByteArray()
