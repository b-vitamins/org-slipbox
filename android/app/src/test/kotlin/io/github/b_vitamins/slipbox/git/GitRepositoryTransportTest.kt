/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git

import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.auth.renewal.MovingClock
import io.github.b_vitamins.slipbox.auth.renewal.ROTATED_ACCESS_TOKEN
import io.github.b_vitamins.slipbox.auth.renewal.TestRenewalStorage
import io.github.b_vitamins.slipbox.auth.renewal.keptCredential
import io.github.b_vitamins.slipbox.auth.renewal.rotatedBody
import io.github.b_vitamins.slipbox.auth.renewal.scopeOf
import io.github.b_vitamins.slipbox.auth.renewal.testOwner
import io.github.b_vitamins.slipbox.security.ForegroundThread
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class GitRepositoryTransportTest {

    @get:Rule
    val temporary = TemporaryFolder()

    @Test
    fun publicFetchCarriesNoCredentialInEitherChannel() {
        val seam = RecordingGitSeam(fetched(OPERATION))
        val transport = GitRepositoryTransport(seam, ForegroundThread.None)

        assertEquals(
            GitSynchronizationOutcome.Fetched(GitDisposition.CLONED, REVISION, 17),
            transport.synchronize(request()),
        )

        assertNull(seam.credential)
        val document = requireNotNull(seam.request).toString(Charsets.UTF_8)
        assertFalse(document.contains("credential"))
        assertFalse(document.contains("token"))
    }

    @Test
    fun renewedCredentialIsUsedByTheGitCallAndItsByteBufferIsErased() {
        val storage = TestRenewalStorage(temporary.newFolder("private"))
        try {
            val clock = MovingClock()
            val authorization = RecordedTransport()
            val owner = testOwner(storage, authorization, clock)
            storage.keep(scopeOf(owner.request), keptCredential(expiresAt = clock.epoch))
            authorization.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
            val seam = RecordingGitSeam(fetched(OPERATION))

            val outcome =
                GitRepositoryTransport(seam, ForegroundThread.None)
                    .synchronize(request(), owner)

            assertTrue(outcome is GitSynchronizationOutcome.Fetched)
            assertEquals(ROTATED_ACCESS_TOKEN, seam.credentialText)
            assertTrue(requireNotNull(seam.credential).all { it == 0.toByte() })
            assertEquals(1, authorization.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        } finally {
            storage.closeAll()
        }
    }

    @Test
    fun foregroundWorkIsRefusedBeforeCredentialOrNativeAccess() {
        val seam = RecordingGitSeam(fetched(OPERATION))
        val outcome =
            GitRepositoryTransport(seam, ForegroundThread { true })
                .synchronize(request())

        assertEquals(
            GitSynchronizationOutcome.ContractFailed(GitContractFault.FOREGROUND_REFUSED),
            outcome,
        )
        assertNull(seam.request)
    }

    @Test
    fun failedTrustInitializationIsRefusedBeforeNativeAccess() {
        val seam = RecordingGitSeam(fetched(OPERATION))
        val outcome =
            GitRepositoryTransport(seam, ForegroundThread.None, initialized = false)
                .synchronize(request())

        assertEquals(
            GitSynchronizationOutcome.ContractFailed(GitContractFault.TLS_INITIALIZATION_FAILED),
            outcome,
        )
        assertNull(seam.request)
    }

    @Test
    fun aForeignAnswerAndAnOversizedAnswerAreNotAccepted() {
        val foreign = RecordingGitSeam(fetched(OPERATION + 1))
        assertEquals(
            GitSynchronizationOutcome.ContractFailed(GitContractFault.FOREIGN_OPERATION),
            GitRepositoryTransport(foreign, ForegroundThread.None).synchronize(request()),
        )

        val oversized = RecordingGitSeam(ByteArray(MAX_GIT_RESPONSE_BYTES + 1))
        assertEquals(
            GitSynchronizationOutcome.ContractFailed(GitContractFault.RESPONSE_OVERSIZED),
            GitRepositoryTransport(oversized, ForegroundThread.None).synchronize(request()),
        )
    }

    @Test
    fun cancellationUsesOnlyTheOperationIdentity() {
        val seam = RecordingGitSeam(fetched(OPERATION), cancellable = OPERATION)
        val transport = GitRepositoryTransport(seam, ForegroundThread.None)

        assertTrue(transport.cancel(OPERATION))
        assertFalse(transport.cancel(0))
        assertEquals(listOf(OPERATION), seam.cancelled)
    }

    private fun request(): GitSynchronization =
        GitSynchronization(
            operation = OPERATION,
            remote = "https://github.com/example/notes.git",
            branch = "main",
            repository = temporary.root.resolve("notes.git"),
        )

    private companion object {

        const val OPERATION = 41L
        const val REVISION = "0123456789abcdef0123456789abcdef01234567"

        fun fetched(operation: Long): ByteArray =
            """{"version":1,"outcome":"fetched","operation":$operation,"disposition":"cloned","revision":"$REVISION","received_objects":17}"""
                .toByteArray(Charsets.UTF_8)
    }
}

private class RecordingGitSeam(
    private val answer: ByteArray?,
    private val cancellable: Long? = null,
) : NativeGitSeam {

    var request: ByteArray? = null
    var credential: ByteArray? = null
    var credentialText: String? = null
    val cancelled = mutableListOf<Long>()

    override fun synchronize(request: ByteArray, credential: ByteArray?): ByteArray? {
        this.request = request.copyOf()
        this.credential = credential
        credentialText = credential?.toString(Charsets.UTF_8)
        return answer
    }

    override fun cancel(operation: Long): Boolean {
        cancelled += operation
        return operation == cancellable
    }
}
