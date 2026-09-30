/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import android.content.Context
import androidx.work.Data
import androidx.work.Worker
import androidx.work.WorkerParameters
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.auth.renewal.CredentialRenewalOwner
import io.github.b_vitamins.slipbox.auth.renewal.RenewalRequest
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import java.io.File
import java.nio.ByteBuffer
import java.security.MessageDigest

/** WorkManager entry point for a complete native source refresh. */
class SourceRefreshWorker internal constructor(
    context: Context,
    parameters: WorkerParameters,
    private val runtime: SourceRefreshRuntime,
) : Worker(context, parameters) {

    constructor(context: Context, parameters: WorkerParameters) :
        this(context, parameters, SourceRefreshRuntime.packaged(context.applicationContext))

    @Volatile
    private var operation: Long? = null

    override fun doWork(): Result {
        val input =
            RefreshWorkWire.decode(inputData)
                ?: return Result.failure(output(RefreshFailureReason.MALFORMED_REQUEST))
        if (runAttemptCount !in 0 until MAX_REFRESH_ATTEMPTS) {
            return terminal(
                input.trigger,
                RefreshFailureReason.OPERATIONS_EXHAUSTED,
            )
        }
        val outcome =
            try {
                runtime.execute(input.source, runAttemptCount) { started -> operation = started }
            } finally {
                operation = null
            }
        return when (val decision = RefreshWorkerPolicy.decide(input.trigger, runAttemptCount, outcome)) {
            RefreshWorkerDecision.SUCCESS -> Result.success()
            RefreshWorkerDecision.RETRY -> Result.retry()
            is RefreshWorkerDecision.FAILURE -> terminal(input.trigger, decision.reason)
        }
    }

    override fun onStopped() {
        operation?.let(runtime::cancel)
        super.onStopped()
    }

    private fun terminal(trigger: RefreshTrigger, reason: RefreshFailureReason): Result =
        when (RefreshWorkerPolicy.terminal(trigger, reason)) {
            RefreshWorkerCompletion.SUCCESS -> Result.success(output(reason))
            RefreshWorkerCompletion.FAILURE -> Result.failure(output(reason))
        }

    private fun output(reason: RefreshFailureReason): Data =
        Data.Builder().putString(OUTPUT_FAILURE, reason.name).build()

    private companion object {

        const val OUTPUT_FAILURE = "refresh_failure"
    }
}

internal sealed interface RefreshWorkerDecision {

    data object SUCCESS : RefreshWorkerDecision

    data object RETRY : RefreshWorkerDecision

    data class FAILURE(val reason: RefreshFailureReason) : RefreshWorkerDecision
}

internal enum class RefreshWorkerCompletion {
    SUCCESS,
    FAILURE,
}

internal object RefreshWorkerPolicy {

    /** A later period remains eligible after this period exhausts transient retries. */
    fun terminal(
        trigger: RefreshTrigger,
        reason: RefreshFailureReason,
    ): RefreshWorkerCompletion =
        if (trigger == RefreshTrigger.PERIODIC && reason.transient) {
            RefreshWorkerCompletion.SUCCESS
        } else {
            RefreshWorkerCompletion.FAILURE
        }

    fun decide(
        trigger: RefreshTrigger,
        attempt: Int,
        outcome: SourceRefreshOutcome,
    ): RefreshWorkerDecision =
        when (outcome) {
            is SourceRefreshOutcome.Answered -> answered(trigger, attempt, outcome.status)
            is SourceRefreshOutcome.Refused -> failure(attempt, outcome.failure)
            is SourceRefreshOutcome.ContractFailed ->
                if (attempt + 1 < MAX_REFRESH_ATTEMPTS) {
                    RefreshWorkerDecision.RETRY
                } else {
                    RefreshWorkerDecision.FAILURE(RefreshFailureReason.ENCODING_FAILED)
                }
        }

    private fun answered(
        trigger: RefreshTrigger,
        attempt: Int,
        status: RefreshStatus,
    ): RefreshWorkerDecision =
        when (status.state) {
            RefreshState.READY -> RefreshWorkerDecision.SUCCESS
            RefreshState.CANCELLED -> RefreshWorkerDecision.SUCCESS
            RefreshState.FAILED ->
                status.failure?.let { failure(attempt, it) }
                    ?: RefreshWorkerDecision.FAILURE(RefreshFailureReason.PANICKED)
            else -> {
                // A blocking refresh contract only returns terminal status.
                val reason =
                    if (trigger == RefreshTrigger.PERIODIC) {
                        RefreshFailureReason.TRANSPORT_FAILED
                    } else {
                        RefreshFailureReason.PANICKED
                    }
                RefreshWorkerDecision.FAILURE(reason)
            }
        }

    private fun failure(attempt: Int, failure: RefreshFailure): RefreshWorkerDecision =
        when {
            failure.reason == RefreshFailureReason.CANCELLED ||
                failure.reason == RefreshFailureReason.SUPERSEDED -> RefreshWorkerDecision.SUCCESS
            failure.retry is RefreshRetry.Backoff && attempt + 1 < MAX_REFRESH_ATTEMPTS ->
                RefreshWorkerDecision.RETRY
            else -> RefreshWorkerDecision.FAILURE(failure.reason)
        }
}

private val RefreshFailureReason.transient: Boolean
    get() =
        this in
            setOf(
                RefreshFailureReason.TLS_INITIALIZATION_FAILED,
                RefreshFailureReason.RATE_LIMITED,
                RefreshFailureReason.TRANSPORT_FAILED,
                RefreshFailureReason.OPERATIONS_EXHAUSTED,
                RefreshFailureReason.PUBLICATION_CONFLICT,
                RefreshFailureReason.ENCODING_FAILED,
            )

internal fun interface ActiveSourceRefreshes {

    fun cancel(source: String)
}

internal fun interface SourceRefreshExecutor {

    fun refresh(request: SourceRefresh): SourceRefreshOutcome
}

internal fun interface SourceRefreshStatusExecutor {

    fun status(source: String): SourceRefreshStatusOutcome
}

internal data class SourceRefreshPaths(val repository: File, val store: File)

internal fun interface SourceRefreshStorage {

    fun resolve(source: RefreshSource): SourceRefreshPaths?
}

/**
 * Process-local execution ownership. It accelerates cancellation; correctness
 * still comes from the source configuration in work data and the Rust
 * coordinator's source-scoped supersession rules.
 */
internal class SourceRefreshRuntime(
    private val executor: SourceRefreshExecutor,
    private val storage: SourceRefreshStorage,
    private val operations: RefreshOperationIds = RefreshOperationIds(),
) : ActiveSourceRefreshes {

    private val active = mutableMapOf<String, MutableSet<Long>>()

    fun execute(
        source: RefreshSource,
        attempt: Int,
        started: (Long) -> Unit = {},
    ): SourceRefreshOutcome {
        val operation = operations.next()
        synchronized(active) {
            active.getOrPut(source.id, ::mutableSetOf).add(operation)
        }
        started(operation)
        return try {
            val paths =
                storage.resolve(source)
                    ?: return SourceRefreshOutcome.Refused(
                        RefreshFailure(
                            RefreshFailureReason.STORAGE_FAILED,
                            retry(attempt, RefreshRetry.FreeStorage),
                        ),
                    )
            executor.refresh(
                SourceRefresh(
                    operation = operation,
                    attempt = attempt,
                    source = source,
                    repository = paths.repository,
                    store = paths.store,
                ),
            )
        } finally {
            synchronized(active) {
                val sourceOperations = active[source.id]
                if (sourceOperations != null) {
                    sourceOperations.remove(operation)
                    if (sourceOperations.isEmpty()) {
                        active.remove(source.id)
                    }
                }
            }
        }
    }

    override fun cancel(source: String) {
        val operations = synchronized(active) { active[source]?.toList().orEmpty() }
        operations.forEach(::cancel)
    }

    fun cancel(operation: Long) {
        (executor as? CancellableSourceRefreshExecutor)?.cancel(operation)
    }

    fun status(source: String): SourceRefreshStatusOutcome =
        (executor as? SourceRefreshStatusExecutor)?.status(source) ?: SourceRefreshStatusOutcome.Idle

    private fun retry(attempt: Int, terminal: RefreshRetry): RefreshRetry =
        if (attempt + 1 < MAX_REFRESH_ATTEMPTS) {
            RefreshRetry.Backoff(30L shl (attempt * 2))
        } else {
            terminal
        }

    companion object {

        @Volatile
        private var instance: SourceRefreshRuntime? = null

        fun packaged(context: Context): SourceRefreshRuntime =
            instance
                ?: synchronized(this) {
                    instance
                        ?: SourceRefreshRuntime(
                                PackagedSourceRefreshExecutor(context.applicationContext),
                                PackagedSourceRefreshStorage(context.applicationContext),
                            )
                            .also { instance = it }
                }
    }
}

internal interface CancellableSourceRefreshExecutor : SourceRefreshExecutor {

    fun cancel(operation: Long): Boolean
}

internal class PackagedSourceRefreshExecutor(context: Context) :
    CancellableSourceRefreshExecutor,
    SourceRefreshStatusExecutor {

    private val application = context.applicationContext

    private val coordinator: SourceRefreshCoordinator by lazy {
        SourceRefreshCoordinator.packaged(application)
    }

    override fun refresh(request: SourceRefresh): SourceRefreshOutcome {
        val owner = credentials(request.source)
        return if (owner == null) {
            coordinator.refresh(request)
        } else {
            owner.use { coordinator.refresh(request, it) }
        }
    }

    override fun cancel(operation: Long): Boolean = coordinator.cancel(operation)

    override fun status(source: String): SourceRefreshStatusOutcome = coordinator.status(source)

    private fun credentials(source: RefreshSource): CredentialRenewalOwner? {
        if (source.provider != RefreshProvider.GITHUB ||
            source.visibility != RefreshVisibility.PRIVATE
        ) {
            return null
        }
        val account = source.account ?: return null
        val credential = source.credential ?: return null
        return CredentialRenewalOwner.packaged(
            application,
            RenewalRequest(
                sourceId = source.id,
                // Only the stable ID participates in renewal scope matching.
                account = VerifiedAccount(account, account),
                credentialRef = credential,
            ),
        )
    }
}

/** Source-owned sibling paths below Android's no-backup directory. */
internal class PackagedSourceRefreshStorage(private val context: Context) : SourceRefreshStorage {

    override fun resolve(source: RefreshSource): SourceRefreshPaths? {
        if (!RefreshWorkWire.validSourceId(source.id)) {
            return null
        }
        val privateRoot =
            when (val root = SlipboxVault.privateRoot(context)) {
                is VaultOutcome.Failed -> return null
                is VaultOutcome.Completed -> root.value
            }
        return prepare(privateRoot, source)
    }

    companion object {

        internal fun paths(privateRoot: File, source: RefreshSource): SourceRefreshPaths? {
            if (!RefreshWorkWire.validSourceId(source.id)) {
                return null
            }
            val owner = sourceDigest(source.id)
            val refreshRoot = safeChild(privateRoot, REFRESH_DIRECTORY) ?: return null
            val sourceRoot = safeChild(refreshRoot, owner) ?: return null
            val configurationRoot =
                safeChild(sourceRoot, "$CONFIGURATION_PREFIX${configurationDigest(source)}")
                    ?: return null
            val repository = safeChild(configurationRoot, REPOSITORY_NAME) ?: return null
            val store = safeChild(configurationRoot, STORE_NAME) ?: return null
            return SourceRefreshPaths(repository, store)
        }

        internal fun prepare(privateRoot: File, source: RefreshSource): SourceRefreshPaths? {
            val paths = paths(privateRoot, source) ?: return null
            if (!paths.store.isDirectory && !paths.store.mkdirs()) {
                return null
            }
            val verified = paths(privateRoot, source) ?: return null
            return verified.takeIf { it.store.isDirectory }
        }

        /** Durable device-owned reading state, outside replaceable configuration generations. */
        internal fun readingTrailFile(privateRoot: File, source: String): File? {
            if (!RefreshWorkWire.validSourceId(source)) return null
            val refreshRoot = safeChild(privateRoot, REFRESH_DIRECTORY) ?: return null
            val sourceRoot = safeChild(refreshRoot, sourceDigest(source)) ?: return null
            val readingRoot = safeChild(sourceRoot, READING_DIRECTORY) ?: return null
            return safeChild(readingRoot, READING_TRAIL_FILE)
        }

        internal fun prepareReadingTrailFile(privateRoot: File, source: String): File? {
            val file = readingTrailFile(privateRoot, source) ?: return null
            val parent = file.parentFile ?: return null
            if (!parent.isDirectory && !parent.mkdirs()) return null
            return readingTrailFile(privateRoot, source)?.takeIf { it.parentFile?.isDirectory == true }
        }

        private fun safeChild(parent: File, name: String): File? =
            try {
                val expected = File(parent.canonicalFile, name).absoluteFile
                expected.takeIf { it.canonicalFile == expected }
            } catch (_: Exception) {
                null
            }

        private fun sourceDigest(source: String): String {
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(DIGEST_LABEL.toByteArray(Charsets.UTF_8))
            digest.update(0)
            val bytes = source.toByteArray(Charsets.UTF_8)
            digest.update(bytes.size.toByte())
            digest.update(bytes)
            return bytesToHex(digest.digest())
        }

        /** Configured inputs own distinct stores, so withdrawn jobs cannot replace new content. */
        private fun configurationDigest(source: RefreshSource): String {
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(CONFIGURATION_DIGEST_LABEL.toByteArray(Charsets.UTF_8))
            digest.update(0)
            listOf(
                    source.id,
                    source.provider.name,
                    source.visibility.name,
                    source.providerRepositoryId,
                    source.account,
                    source.remote,
                    source.branch,
                    source.notesFolder,
                    source.credential,
                )
                .forEach { field ->
                    val bytes = field?.toByteArray(Charsets.UTF_8)
                    digest.update(ByteBuffer.allocate(Int.SIZE_BYTES).putInt(bytes?.size ?: -1).array())
                    bytes?.let(digest::update)
                }
            return bytesToHex(digest.digest())
        }

        private fun bytesToHex(bytes: ByteArray): String =
            buildString(bytes.size * 2) {
                for (byte in bytes) {
                    val value = byte.toInt() and 0xff
                    append(HEX[value ushr 4]).append(HEX[value and 0x0f])
                }
            }

        private const val REFRESH_DIRECTORY = "refresh"

        private const val CONFIGURATION_PREFIX = "configuration-"

        private const val REPOSITORY_NAME = "repository.git"

        private const val STORE_NAME = "store"

        private const val READING_DIRECTORY = "reading"

        private const val READING_TRAIL_FILE = "trail.json"

        private const val DIGEST_LABEL = "slipbox.refresh.source.1"

        private const val CONFIGURATION_DIGEST_LABEL = "slipbox.refresh.configuration.1"

        private const val HEX = "0123456789abcdef"
    }
}

internal class RefreshOperationIds {

    private var last = 0L

    @Synchronized
    fun next(): Long {
        last = if (last == Long.MAX_VALUE) 1L else last + 1L
        return last
    }
}
