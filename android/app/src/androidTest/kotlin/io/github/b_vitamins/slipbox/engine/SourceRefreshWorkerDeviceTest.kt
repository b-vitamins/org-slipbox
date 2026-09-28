/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import android.annotation.SuppressLint
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.work.BackoffPolicy
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters
import androidx.work.testing.SynchronousExecutor
import androidx.work.testing.TestWorkerBuilder
import io.github.b_vitamins.slipbox.sync.CancellableSourceRefreshExecutor
import io.github.b_vitamins.slipbox.sync.RefreshDisposition
import io.github.b_vitamins.slipbox.sync.RefreshProgress
import io.github.b_vitamins.slipbox.sync.RefreshProgressUnit
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshState
import io.github.b_vitamins.slipbox.sync.RefreshStatus
import io.github.b_vitamins.slipbox.sync.RefreshTrigger
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.sync.RefreshWorkInput
import io.github.b_vitamins.slipbox.sync.RefreshWorkNames
import io.github.b_vitamins.slipbox.sync.RefreshWorkRequests
import io.github.b_vitamins.slipbox.sync.RefreshWorkWire
import io.github.b_vitamins.slipbox.sync.SourceRefresh
import io.github.b_vitamins.slipbox.sync.SourceRefreshOutcome
import io.github.b_vitamins.slipbox.sync.SourceRefreshPaths
import io.github.b_vitamins.slipbox.sync.SourceRefreshRuntime
import io.github.b_vitamins.slipbox.sync.SourceRefreshStorage
import io.github.b_vitamins.slipbox.sync.SourceRefreshWorker
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SourceRefreshWorkerDeviceTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun actualWorkerRunsOffTheCallerWithoutProductionCredentials() {
        val executor = ReadyDeviceExecutor()
        val runtime = runtime(executor)
        val worker =
            worker(
                runtime,
                RefreshWorkInput(RefreshTrigger.MANUAL, source()),
                SynchronousExecutor(),
            )

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success()::class.java, result::class.java)
        assertEquals(1L, executor.requests.size.toLong())
        assertEquals(source(), executor.requests.single().source)
    }

    @Test
    @SuppressLint("RestrictedApi") // The fixture inspects the WorkRequest that production builds.
    fun workRequestsCarryTheSupportedBackgroundConstraintsAndBackoff() {
        val startup =
            RefreshWorkRequests.oneShot(RefreshWorkInput(RefreshTrigger.STARTUP, source()))
        val manual =
            RefreshWorkRequests.oneShot(RefreshWorkInput(RefreshTrigger.MANUAL, source()))
        val periodic =
            RefreshWorkRequests.periodic(RefreshWorkInput(RefreshTrigger.PERIODIC, source()))

        for (request in listOf(startup, manual, periodic)) {
            val constraints = request.workSpec.constraints
            assertEquals(NetworkType.CONNECTED, constraints.requiredNetworkType)
            assertTrue(constraints.requiresStorageNotLow())
            assertEquals(BackoffPolicy.EXPONENTIAL, request.workSpec.backoffPolicy)
            assertEquals(
                TimeUnit.SECONDS.toMillis(RefreshWorkRequests.MIN_BACKOFF_SECONDS),
                request.workSpec.backoffDelayDuration,
            )
            assertTrue(request.tags.contains(RefreshWorkNames.ALL))
            assertTrue(request.tags.contains(RefreshWorkNames.sourceTag(source().id)))
        }
        assertTrue(startup.workSpec.constraints.requiresBatteryNotLow())
        assertFalse(manual.workSpec.constraints.requiresBatteryNotLow())
        assertTrue(periodic.workSpec.constraints.requiresBatteryNotLow())
        assertEquals(
            TimeUnit.HOURS.toMillis(RefreshWorkRequests.PERIODIC_HOURS),
            periodic.workSpec.intervalDuration,
        )
        assertEquals(
            TimeUnit.HOURS.toMillis(RefreshWorkRequests.PERIODIC_FLEX_HOURS),
            periodic.workSpec.flexDuration,
        )
    }

    @Test
    fun stoppingAWorkerCancelsItsActiveNativeOperation() {
        val executor = BlockingDeviceExecutor()
        val runtime = runtime(executor)
        val background = Executors.newSingleThreadExecutor()
        try {
            val worker =
                worker(
                    runtime,
                    RefreshWorkInput(RefreshTrigger.MANUAL, source()),
                    background,
                )
            worker.startWork()
            assertTrue(executor.started.await(5, TimeUnit.SECONDS))

            worker.onStopped()

            assertTrue(executor.cancelled.await(5, TimeUnit.SECONDS))
            assertTrue(executor.cancelledOperation > 0)
            executor.release.countDown()
        } finally {
            executor.release.countDown()
            background.shutdownNow()
        }
    }

    private fun worker(
        runtime: SourceRefreshRuntime,
        input: RefreshWorkInput,
        executor: java.util.concurrent.Executor,
    ): SourceRefreshWorker =
        TestWorkerBuilder.from(context, SourceRefreshWorker::class.java, executor)
            .setInputData(checkNotNull(RefreshWorkWire.data(input)))
            .setWorkerFactory(DeviceWorkerFactory(runtime))
            .build()

    private fun runtime(executor: CancellableSourceRefreshExecutor): SourceRefreshRuntime {
        val root = File(context.cacheDir, "refresh-worker-fixture")
        return SourceRefreshRuntime(
            executor,
            SourceRefreshStorage { requested ->
                check(requested.id == SOURCE)
                SourceRefreshPaths(File(root, "repository.git"), File(root, "store"))
            },
        )
    }

    private companion object {

        const val SOURCE = "0123456789abcdef0123456789abcdef"

        fun source(): RefreshSource =
            RefreshSource(
                id = SOURCE,
                displayName = "Fixture notes",
                provider = RefreshProvider.GENERIC_HTTPS,
                visibility = RefreshVisibility.PUBLIC,
                remote = "https://example.com/fixture.git",
                branch = "main",
                notesFolder = "",
            )
    }
}

private class DeviceWorkerFactory(private val runtime: SourceRefreshRuntime) : WorkerFactory() {

    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: WorkerParameters,
    ): ListenableWorker? =
        if (workerClassName == SourceRefreshWorker::class.java.name) {
            SourceRefreshWorker(appContext, workerParameters, runtime)
        } else {
            null
        }
}

private open class ReadyDeviceExecutor : CancellableSourceRefreshExecutor {

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

private class BlockingDeviceExecutor : ReadyDeviceExecutor() {

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
