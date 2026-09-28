/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import android.annotation.SuppressLint
import java.io.File
import java.nio.file.Files
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Worker execution tests use neither Android credentials nor a foreground thread. */
class SourceRefreshWorkerTest {

    @get:Rule
    val temporary = TemporaryFolder()

    @Test
    fun runtimeAllocatesOperationsAndPassesOnlyResolvedPrivatePaths() {
        val executor = RecordingExecutor()
        val root = temporary.newFolder("private")
        val paths = SourceRefreshPaths(File(root, "repository.git"), File(root, "store"))
        val runtime = SourceRefreshRuntime(executor, SourceRefreshStorage { requested ->
            assertEquals(source(), requested)
            paths
        })

        val first = runtime.execute(source(), attempt = 0)
        val second = runtime.execute(source(), attempt = 1)

        assertTrue(first is SourceRefreshOutcome.Answered)
        assertTrue(second is SourceRefreshOutcome.Answered)
        assertEquals(2L, executor.requests.size.toLong())
        assertEquals(paths.repository, executor.requests[0].repository)
        assertEquals(paths.store, executor.requests[0].store)
        assertEquals(0L, executor.requests[0].attempt.toLong())
        assertEquals(1L, executor.requests[1].attempt.toLong())
        assertNotEquals(executor.requests[0].operation, executor.requests[1].operation)
    }

    @Test
    fun aStorageRefusalBacksOffOnlyWithinTheAttemptBound() {
        val executor = RecordingExecutor()
        val runtime = SourceRefreshRuntime(executor, SourceRefreshStorage { requested ->
            assertEquals(source(), requested)
            null
        })

        assertEquals(
            SourceRefreshOutcome.Refused(
                RefreshFailure(
                    RefreshFailureReason.STORAGE_FAILED,
                    RefreshRetry.Backoff(30),
                ),
            ),
            runtime.execute(source(), attempt = 0),
        )
        assertEquals(
            SourceRefreshOutcome.Refused(
                RefreshFailure(
                    RefreshFailureReason.STORAGE_FAILED,
                    RefreshRetry.FreeStorage,
                ),
            ),
            runtime.execute(source(), attempt = 2),
        )
        assertTrue(executor.requests.isEmpty())
    }

    @Test
    fun sourceCancellationReachesEveryActiveNativeOperation() {
        val executor = BlockingExecutor()
        val root = temporary.newFolder("cancel")
        val runtime =
            SourceRefreshRuntime(
                executor,
                SourceRefreshStorage { requested ->
                    assertEquals(source(), requested)
                    SourceRefreshPaths(File(root, "repository.git"), File(root, "store"))
                },
            )
        val worker = Thread { runtime.execute(source(), attempt = 0) }
        worker.start()
        assertTrue(executor.started.await(5, TimeUnit.SECONDS))

        runtime.cancel(source().id)

        assertTrue(executor.cancelled.await(5, TimeUnit.SECONDS))
        assertTrue(executor.cancelledOperation > 0)
        executor.release.countDown()
        worker.join(5_000)
        assertFalse(worker.isAlive)
    }

    @Test
    @SuppressLint("NewApi") // This local JVM test runs on the host JDK, not an API-23 device.
    fun privatePathsAreStableOpaqueSiblingsAndRefuseAnEscapingLink() {
        val privateRoot = temporary.newFolder("no-backup")
        val source = source()

        val first = requireNotNull(PackagedSourceRefreshStorage.prepare(privateRoot, source))
        val second = requireNotNull(PackagedSourceRefreshStorage.prepare(privateRoot, source))
        val changed =
            requireNotNull(
                PackagedSourceRefreshStorage.prepare(privateRoot, source.copy(branch = "next")),
            )

        assertEquals(first, second)
        assertEquals(first.repository.parentFile, first.store.parentFile)
        assertNotEquals(first.store.parentFile, changed.store.parentFile)
        assertEquals(first.store.parentFile?.parentFile, changed.store.parentFile?.parentFile)
        assertEquals(
            "da037c8e07ff443a2145a7b351974f36142a4dec6301bfb77674d09377729cf1",
            first.store.parentFile?.parentFile?.name,
        )
        assertTrue(first.store.isDirectory)
        assertTrue(first.repository.path.startsWith(privateRoot.path + File.separator))
        assertFalse(first.repository.path.contains(source.id))
        assertNull(PackagedSourceRefreshStorage.paths(privateRoot, source.copy(id = "../source")))

        val linkedSource = source.copy(id = "fedcba9876543210fedcba9876543210")
        val linkedPaths =
            requireNotNull(PackagedSourceRefreshStorage.paths(privateRoot, linkedSource))
        val outside = temporary.newFolder("outside")
        val sourceRoot = requireNotNull(linkedPaths.repository.parentFile?.parentFile)
        val refreshRoot = requireNotNull(sourceRoot.parentFile)
        assertTrue(refreshRoot.isDirectory || refreshRoot.mkdirs())
        Files.createSymbolicLink(sourceRoot.toPath(), outside.toPath())

        assertNull(PackagedSourceRefreshStorage.prepare(privateRoot, linkedSource))
    }

    @Test
    fun workerPolicyBoundsRetriesAndKeepsOnlyTransientPeriodicCadence() {
        val transient =
            SourceRefreshOutcome.Refused(
                RefreshFailure(
                    RefreshFailureReason.TRANSPORT_FAILED,
                    RefreshRetry.Backoff(30),
                ),
            )
        assertEquals(
            RefreshWorkerDecision.RETRY,
            RefreshWorkerPolicy.decide(RefreshTrigger.MANUAL, 0, transient),
        )
        assertEquals(
            RefreshWorkerDecision.FAILURE(RefreshFailureReason.TRANSPORT_FAILED),
            RefreshWorkerPolicy.decide(RefreshTrigger.MANUAL, 2, transient),
        )
        assertEquals(
            RefreshWorkerCompletion.SUCCESS,
            RefreshWorkerPolicy.terminal(
                RefreshTrigger.PERIODIC,
                RefreshFailureReason.TRANSPORT_FAILED,
            ),
        )
        assertEquals(
            RefreshWorkerCompletion.FAILURE,
            RefreshWorkerPolicy.terminal(
                RefreshTrigger.PERIODIC,
                RefreshFailureReason.AUTHORIZATION_REQUIRED,
            ),
        )
        assertEquals(
            RefreshWorkerCompletion.FAILURE,
            RefreshWorkerPolicy.terminal(
                RefreshTrigger.MANUAL,
                RefreshFailureReason.TRANSPORT_FAILED,
            ),
        )
    }

    @Test
    fun readyAndCancelledRefreshesCompleteWithoutRetry() {
        assertEquals(
            RefreshWorkerDecision.SUCCESS,
            RefreshWorkerPolicy.decide(
                RefreshTrigger.STARTUP,
                0,
                SourceRefreshOutcome.Answered(RefreshDisposition.STARTED, ready()),
            ),
        )
        assertEquals(
            RefreshWorkerDecision.SUCCESS,
            RefreshWorkerPolicy.decide(
                RefreshTrigger.MANUAL,
                0,
                SourceRefreshOutcome.Refused(
                    RefreshFailure(RefreshFailureReason.CANCELLED, RefreshRetry.Never),
                ),
            ),
        )
    }

    private companion object {

        const val SOURCE = "0123456789abcdef0123456789abcdef"

        fun source(): RefreshSource =
            RefreshSource(
                id = SOURCE,
                displayName = "Public notes",
                provider = RefreshProvider.GENERIC_HTTPS,
                visibility = RefreshVisibility.PUBLIC,
                remote = "https://example.com/notes.git",
                branch = "main",
                notesFolder = "",
            )

        fun ready(operation: Long = 1): RefreshStatus =
            RefreshStatus(
                source = source(),
                operation = operation,
                state = RefreshState.READY,
                fetchedRevision = "0123456789abcdef0123456789abcdef01234567",
                readyRevision = "0123456789abcdef0123456789abcdef01234567",
                readyGeneration = "refresh-$operation",
                progress = RefreshProgress(1, 1, RefreshProgressUnit.STEPS),
                failure = null,
            )
    }
}

private open class RecordingExecutor : CancellableSourceRefreshExecutor {

    val requests = mutableListOf<SourceRefresh>()

    override fun refresh(request: SourceRefresh): SourceRefreshOutcome {
        requests.add(request)
        return SourceRefreshOutcome.Answered(
            RefreshDisposition.STARTED,
            RefreshStatus(
                source = request.source,
                operation = request.operation,
                state = RefreshState.READY,
                fetchedRevision = REVISION,
                readyRevision = REVISION,
                readyGeneration = "refresh-${request.operation}",
                progress = RefreshProgress(1, 1, RefreshProgressUnit.STEPS),
                failure = null,
            ),
        )
    }

    override fun cancel(operation: Long): Boolean = false

    private companion object {

        const val REVISION = "0123456789abcdef0123456789abcdef01234567"
    }
}

private class BlockingExecutor : RecordingExecutor() {

    val started = CountDownLatch(1)

    val cancelled = CountDownLatch(1)

    val release = CountDownLatch(1)

    @Volatile
    var cancelledOperation = 0L

    override fun refresh(request: SourceRefresh): SourceRefreshOutcome {
        started.countDown()
        assertTrue(release.await(5, TimeUnit.SECONDS))
        return super.refresh(request)
    }

    override fun cancel(operation: Long): Boolean {
        cancelledOperation = operation
        cancelled.countDown()
        return true
    }
}
