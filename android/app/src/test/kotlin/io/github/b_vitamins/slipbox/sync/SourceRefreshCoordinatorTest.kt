/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.refused
import io.github.b_vitamins.slipbox.auth.renewal.MovingClock
import io.github.b_vitamins.slipbox.auth.renewal.ROTATED_ACCESS_TOKEN
import io.github.b_vitamins.slipbox.auth.renewal.TestRenewalStorage
import io.github.b_vitamins.slipbox.auth.renewal.keptCredential
import io.github.b_vitamins.slipbox.auth.renewal.rotatedBody
import io.github.b_vitamins.slipbox.auth.renewal.scopeOf
import io.github.b_vitamins.slipbox.auth.renewal.testOwner
import io.github.b_vitamins.slipbox.auth.renewal.testRequest
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.git.NativeGitSeam
import io.github.b_vitamins.slipbox.security.ForegroundThread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class SourceRefreshCoordinatorTest {

    @get:Rule
    val temporary = TemporaryFolder()

    @Test
    fun privateRefreshCarriesSourceConfigurationAndErasesTheRenewedCredential() {
        val storage = TestRenewalStorage(temporary.newFolder("private"))
        try {
            val clock = MovingClock()
            val authorization = RecordedTransport()
            val owner = owner(storage, authorization, clock)
            storage.keep(scopeOf(owner.request), keptCredential(expiresAt = clock.epoch))
            authorization.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
            val seam = RecordingRefreshSeam(ready())

            val outcome = coordinator(seam).refresh(refresh(privateSource()), owner)

            val answered = outcome as SourceRefreshOutcome.Answered
            assertEquals(RefreshDisposition.STARTED, answered.disposition)
            assertEquals(REVISION_FETCHED, answered.status.fetchedRevision)
            assertEquals(REVISION_READY, answered.status.readyRevision)
            assertEquals(ROTATED_ACCESS_TOKEN, seam.credentialText)
            assertTrue(requireNotNull(seam.credential).all { it == 0.toByte() })
            assertEquals(1, authorization.exchangesOf(GithubEndpoint.ACCESS_TOKEN))

            val document = requireNotNull(seam.refreshRequest).toString(Charsets.UTF_8)
            assertTrue(document.contains("\"id\":\"$SOURCE\""))
            assertTrue(document.contains("\"provider_repository_id\":\"$REPOSITORY\""))
            assertTrue(document.contains("\"account\":\"$ACCOUNT\""))
            assertTrue(document.contains("\"credential\":\"$CREDENTIAL\""))
            assertTrue(document.contains("\"branch\":\"release/reader\""))
            assertTrue(document.contains("\"notes_folder\":\"slipbox\""))
            assertFalse(document.contains(ROTATED_ACCESS_TOKEN))
        } finally {
            storage.closeAll()
        }
    }

    @Test
    fun aForeignCredentialOwnerIsRefusedBeforeVaultOrNativeAccess() {
        val storage = TestRenewalStorage(temporary.newFolder("foreign"))
        try {
            storage.beforeRead = { throw AssertionError("the foreign vault was opened") }
            val foreign =
                testOwner(
                    storage,
                    RecordedTransport(),
                    request =
                        testRequest(
                            sourceId = "fedcba9876543210fedcba9876543210",
                            accountId = ACCOUNT,
                            credentialRef = CREDENTIAL,
                        ),
                )
            val seam = RecordingRefreshSeam(ready())

            assertEquals(
                SourceRefreshOutcome.Refused(
                    RefreshFailure(
                        RefreshFailureReason.AUTHORIZATION_FAILED,
                        RefreshRetry.Never,
                    ),
                ),
                coordinator(seam).refresh(refresh(privateSource()), foreign),
            )
            assertNull(seam.refreshRequest)
        } finally {
            storage.closeAll()
        }
    }

    @Test
    fun foregroundAndTlsFailuresDoNotReachCredentialsOrNativeCode() {
        val foreground = RecordingRefreshSeam(ready())
        assertEquals(
            SourceRefreshOutcome.Refused(
                RefreshFailure(RefreshFailureReason.FOREGROUND_REFUSED, RefreshRetry.Never),
            ),
            SourceRefreshCoordinator(foreground, ForegroundThread { true })
                .refresh(refresh(publicSource())),
        )
        assertNull(foreground.refreshRequest)

        val tls = RecordingRefreshSeam(ready())
        assertEquals(
            SourceRefreshOutcome.Refused(
                RefreshFailure(
                    RefreshFailureReason.TLS_INITIALIZATION_FAILED,
                    RefreshRetry.Never,
                ),
            ),
            SourceRefreshCoordinator(tls, ForegroundThread.None, initialized = false)
                .refresh(refresh(publicSource(), attempt = 2)),
        )
        assertNull(tls.refreshRequest)
    }

    @Test
    fun rateLimitsAreClassifiedWithBoundedBackoffBeforeNativeAccess() {
        val storage = TestRenewalStorage(temporary.newFolder("limited"))
        try {
            val clock = MovingClock()
            val authorization = RecordedTransport()
            val owner = owner(storage, authorization, clock)
            storage.keep(scopeOf(owner.request), keptCredential(expiresAt = clock.epoch))
            authorization.answers(GithubEndpoint.ACCESS_TOKEN, refused("{}", status = 429))
            val seam = RecordingRefreshSeam(ready())

            assertEquals(
                SourceRefreshOutcome.Refused(
                    RefreshFailure(
                        RefreshFailureReason.RATE_LIMITED,
                        RefreshRetry.Backoff(afterSeconds = 120),
                    ),
                ),
                coordinator(seam).refresh(refresh(privateSource(), attempt = 1), owner),
            )
            assertNull(seam.refreshRequest)
        } finally {
            storage.closeAll()
        }
    }

    @Test
    fun statusLookupIsCredentialFreeAndKeepsFetchedAndReadyRevisionsDistinct() {
        val seam = RecordingRefreshSeam(answer = null, statusAnswer = knownStatus())

        val outcome = coordinator(seam).status(SOURCE)

        val known = outcome as SourceRefreshStatusOutcome.Known
        assertEquals(SOURCE, known.status.source.id)
        assertEquals(REVISION_FETCHED, known.status.fetchedRevision)
        assertEquals(REVISION_READY, known.status.readyRevision)
        assertEquals(GENERATION, known.status.readyGeneration)
        assertEquals(SOURCE, seam.statusSource())
        assertNull(seam.refreshRequest)
        assertNull(seam.credential)
    }

    @Test
    fun aCompletionForAnOlderSourceConfigurationIsRejected() {
        val seam = RecordingRefreshSeam(ready(branch = "main"))

        assertEquals(
            SourceRefreshOutcome.ContractFailed(RefreshContractFault.FOREIGN_SOURCE),
            coordinator(seam).refresh(refresh(privateSource())),
        )
    }

    private fun coordinator(seam: NativeGitSeam): SourceRefreshCoordinator =
        SourceRefreshCoordinator(seam, ForegroundThread.None)

    private fun refresh(source: RefreshSource, attempt: Int = 0): SourceRefresh =
        SourceRefresh(
            operation = OPERATION,
            attempt = attempt,
            source = source,
            repository = temporary.root.resolve("source/repository.git"),
            store = temporary.root.resolve("source/store"),
        )

    private fun owner(
        storage: TestRenewalStorage,
        authorization: RecordedTransport,
        clock: MovingClock,
    ) =
        testOwner(
            storage,
            authorization,
            clock,
            testRequest(
                sourceId = SOURCE,
                accountId = ACCOUNT,
                credentialRef = CREDENTIAL,
            ),
        )

    private companion object {

        const val OPERATION = 41L
        const val SOURCE = "0123456789abcdef0123456789abcdef"
        const val REPOSITORY = "R_kgDOAbCdEf"
        const val ACCOUNT = "U_kgDOAbCdEf"
        const val CREDENTIAL = "slipbox.source.vault-handle-1"
        const val REVISION_FETCHED = "0123456789abcdef0123456789abcdef01234567"
        const val REVISION_READY = "89abcdef0123456789abcdef0123456789abcdef"
        const val GENERATION = "refresh-41"

        fun publicSource(): RefreshSource =
            RefreshSource(
                id = SOURCE,
                displayName = "Public notes",
                provider = RefreshProvider.GENERIC_HTTPS,
                visibility = RefreshVisibility.PUBLIC,
                remote = "https://example.com/notes.git",
                branch = "main",
                notesFolder = "",
            )

        fun privateSource(): RefreshSource =
            RefreshSource(
                id = SOURCE,
                displayName = "Private notes",
                provider = RefreshProvider.GITHUB,
                visibility = RefreshVisibility.PRIVATE,
                providerRepositoryId = REPOSITORY,
                account = ACCOUNT,
                remote = "https://github.com/example/notes.git",
                branch = "release/reader",
                notesFolder = "slipbox",
                credential = CREDENTIAL,
            )

        fun ready(branch: String = "release/reader"): ByteArray =
            """{"version":1,"outcome":"answered","operation":$OPERATION,"disposition":"started","status":${status(branch)}}"""
                .toByteArray(Charsets.UTF_8)

        fun knownStatus(): ByteArray =
            """{"version":1,"outcome":{"kind":"known","status":${status("release/reader")}}}"""
                .toByteArray(Charsets.UTF_8)

        fun status(branch: String): String =
            """{"source":{"id":"$SOURCE","display_name":"Private notes","provider":"github","visibility":"private","provider_repository_id":"$REPOSITORY","account":"$ACCOUNT","remote":"https://github.com/example/notes.git","branch":"$branch","notes_folder":"slipbox","credential":"$CREDENTIAL"},"operation":$OPERATION,"state":"ready","fetched_revision":"$REVISION_FETCHED","ready_revision":"$REVISION_READY","ready_generation":"$GENERATION","progress":{"completed":7,"total":7,"unit":"files"},"failure":null}"""
    }
}

private class RecordingRefreshSeam(
    private val answer: ByteArray?,
    private val statusAnswer: ByteArray? = null,
) : NativeGitSeam {

    var refreshRequest: ByteArray? = null
    var statusRequest: ByteArray? = null
    var credential: ByteArray? = null
    var credentialText: String? = null

    override fun synchronize(request: ByteArray, credential: ByteArray?): ByteArray? =
        throw AssertionError("the legacy Git seam was used")

    override fun materialize(request: ByteArray): ByteArray? =
        throw AssertionError("the legacy materialization seam was used")

    override fun refresh(request: ByteArray, credential: ByteArray?): ByteArray? {
        refreshRequest = request.copyOf()
        this.credential = credential
        credentialText = credential?.toString(Charsets.UTF_8)
        return answer
    }

    override fun refreshStatus(request: ByteArray): ByteArray? {
        statusRequest = request.copyOf()
        return statusAnswer
    }

    override fun cancel(operation: Long): Boolean = false

    fun statusSource(): String? {
        val document = statusRequest?.toString(Charsets.UTF_8) ?: return null
        return Regex("\\\"source\\\":\\\"([^\\\"]+)\\\"").find(document)?.groupValues?.get(1)
    }
}
