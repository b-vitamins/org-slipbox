/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequest
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import java.util.concurrent.TimeUnit
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** The application event asking for a source refresh. */
@Serializable
enum class RefreshTrigger {
    @SerialName("startup")
    STARTUP,

    @SerialName("manual")
    MANUAL,

    @SerialName("periodic")
    PERIODIC,
}

/** Why a refresh could not be handed to WorkManager. */
enum class RefreshScheduleRefusal {
    INVALID_SOURCE,
    INPUT_OVERSIZED,
}

sealed interface RefreshScheduleOutcome {

    data object Accepted : RefreshScheduleOutcome

    data class Refused(val reason: RefreshScheduleRefusal) : RefreshScheduleOutcome
}

/** Bounded result of checking known sources when the application starts. */
data class StartupRefreshOutcome(
    val accepted: Int,
    val refused: Int,
    val deferred: Int,
)

@Serializable
internal data class RefreshWorkInput(
    val trigger: RefreshTrigger,
    val source: RefreshSource,
    val version: Int = REFRESH_WORK_VERSION,
)

/** Stable names let WorkManager and the native coordinator coalesce every entry point. */
internal object RefreshWorkNames {

    private const val WORK_PREFIX = "slipbox.refresh"

    fun oneShot(source: String): String = "$WORK_PREFIX.once.$source"

    fun periodic(source: String): String = "$WORK_PREFIX.periodic.$source"

    fun sourceTag(source: String): String = "$WORK_PREFIX.source.$source"

    const val ALL: String = "$WORK_PREFIX.all"
}

internal interface RefreshWorkQueue {

    fun oneShot(input: RefreshWorkInput, replace: Boolean)

    fun periodic(input: RefreshWorkInput, replace: Boolean)

    fun cancel(source: String)
}

/**
 * Application boundary for startup, manual and periodic refresh work.
 *
 * Scheduling only persists validated source configuration. Credentials stay in
 * the vault and are acquired by the worker for the duration of one attempt.
 */
class SourceRefreshScheduler internal constructor(
    private val queue: RefreshWorkQueue,
    private val active: ActiveSourceRefreshes,
) {

    /** Continue a completed foreground import without immediately fetching it again. */
    fun imported(source: RefreshSource): RefreshScheduleOutcome {
        val input = input(RefreshTrigger.PERIODIC, source) ?: return refusal(source)
        queue.periodic(input, replace = false)
        return RefreshScheduleOutcome.Accepted
    }

    /** Installs recurring work and requests a first refresh for [source]. */
    fun configure(
        previous: RefreshSource?,
        source: RefreshSource,
    ): RefreshScheduleOutcome {
        val input = input(RefreshTrigger.STARTUP, source) ?: return refusal(source)
        val replaced = previous != null && !previous.hasSameImportConfiguration(source)
        if (replaced) {
            active.cancel(previous.id)
            if (previous.id != source.id) {
                queue.cancel(previous.id)
            }
        }
        queue.periodic(input.copy(trigger = RefreshTrigger.PERIODIC), replace = replaced)
        queue.oneShot(input, replace = replaced)
        return RefreshScheduleOutcome.Accepted
    }

    /** Explicit user refresh; an already queued attempt for the source wins. */
    fun manual(source: RefreshSource): RefreshScheduleOutcome {
        val input = input(RefreshTrigger.MANUAL, source) ?: return refusal(source)
        queue.oneShot(input, replace = false)
        return RefreshScheduleOutcome.Accepted
    }

    /**
     * Reconciles only a bounded set at startup. Recurring work remains
     * registered for every valid source, while later sources wait for their
     * normal cadence or an explicit action.
     */
    fun startup(sources: Iterable<RefreshSource>): StartupRefreshOutcome {
        var accepted = 0
        var refused = 0
        var deferred = 0
        for (source in sources) {
            val input = input(RefreshTrigger.STARTUP, source)
            if (input == null) {
                refused += 1
                continue
            }
            queue.periodic(input.copy(trigger = RefreshTrigger.PERIODIC), replace = false)
            if (accepted >= MAX_STARTUP_REFRESHES) {
                deferred += 1
                continue
            }
            queue.oneShot(input, replace = false)
            accepted += 1
        }
        return StartupRefreshOutcome(accepted, refused, deferred)
    }

    /** Cancels queued and currently executing work without deleting readable data. */
    fun remove(source: String): RefreshScheduleOutcome {
        if (!RefreshWorkWire.validSourceId(source)) {
            return RefreshScheduleOutcome.Refused(RefreshScheduleRefusal.INVALID_SOURCE)
        }
        active.cancel(source)
        queue.cancel(source)
        return RefreshScheduleOutcome.Accepted
    }

    private fun input(trigger: RefreshTrigger, source: RefreshSource): RefreshWorkInput? {
        if (!RefreshWorkWire.validSourceId(source.id)) {
            return null
        }
        val input = RefreshWorkInput(trigger, source)
        return input.takeIf { RefreshWorkWire.encode(it) != null }
    }

    private fun refusal(source: RefreshSource): RefreshScheduleOutcome.Refused =
        if (!RefreshWorkWire.validSourceId(source.id)) {
            RefreshScheduleOutcome.Refused(RefreshScheduleRefusal.INVALID_SOURCE)
        } else {
            RefreshScheduleOutcome.Refused(RefreshScheduleRefusal.INPUT_OVERSIZED)
        }

    companion object {

        /** Maximum network checks admitted synchronously from one startup snapshot. */
        const val MAX_STARTUP_REFRESHES: Int = 8

        fun packaged(context: Context): SourceRefreshScheduler {
            val runtime = SourceRefreshRuntime.packaged(context.applicationContext)
            return SourceRefreshScheduler(
                WorkManagerRefreshQueue(WorkManager.getInstance(context.applicationContext)),
                runtime,
            )
        }
    }
}

internal class WorkManagerRefreshQueue(private val workManager: WorkManager) : RefreshWorkQueue {

    override fun oneShot(input: RefreshWorkInput, replace: Boolean) {
        workManager.enqueueUniqueWork(
            RefreshWorkNames.oneShot(input.source.id),
            if (replace) ExistingWorkPolicy.REPLACE else ExistingWorkPolicy.KEEP,
            RefreshWorkRequests.oneShot(input),
        )
    }

    override fun periodic(input: RefreshWorkInput, replace: Boolean) {
        workManager.enqueueUniquePeriodicWork(
            RefreshWorkNames.periodic(input.source.id),
            if (replace) {
                ExistingPeriodicWorkPolicy.CANCEL_AND_REENQUEUE
            } else {
                ExistingPeriodicWorkPolicy.UPDATE
            },
            RefreshWorkRequests.periodic(input),
        )
    }

    override fun cancel(source: String) {
        workManager.cancelAllWorkByTag(RefreshWorkNames.sourceTag(source))
    }
}

internal object RefreshWorkRequests {

    fun oneShot(input: RefreshWorkInput): OneTimeWorkRequest =
        OneTimeWorkRequest.Builder(SourceRefreshWorker::class.java)
            .setInputData(checkNotNull(RefreshWorkWire.data(input)))
            .setConstraints(constraints(input.trigger))
            .setBackoffCriteria(
                BackoffPolicy.EXPONENTIAL,
                MIN_BACKOFF_SECONDS,
                TimeUnit.SECONDS,
            )
            .addTag(RefreshWorkNames.ALL)
            .addTag(RefreshWorkNames.sourceTag(input.source.id))
            .build()

    fun periodic(input: RefreshWorkInput): PeriodicWorkRequest =
        PeriodicWorkRequest.Builder(
            SourceRefreshWorker::class.java,
            PERIODIC_HOURS,
            TimeUnit.HOURS,
            PERIODIC_FLEX_HOURS,
            TimeUnit.HOURS,
        )
            .setInputData(checkNotNull(RefreshWorkWire.data(input)))
            .setConstraints(constraints(input.trigger))
            .setBackoffCriteria(
                BackoffPolicy.EXPONENTIAL,
                MIN_BACKOFF_SECONDS,
                TimeUnit.SECONDS,
            )
            .addTag(RefreshWorkNames.ALL)
            .addTag(RefreshWorkNames.sourceTag(input.source.id))
            .build()

    private fun constraints(trigger: RefreshTrigger): Constraints =
        Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .setRequiresStorageNotLow(true)
            .setRequiresBatteryNotLow(trigger != RefreshTrigger.MANUAL)
            .build()

    const val PERIODIC_HOURS = 12L

    const val PERIODIC_FLEX_HOURS = 2L

    const val MIN_BACKOFF_SECONDS = 30L
}

internal object RefreshWorkWire {

    private val json = Json {
        encodeDefaults = true
        explicitNulls = false
        ignoreUnknownKeys = false
    }

    fun validSourceId(source: String): Boolean =
        source.length == SOURCE_ID_CHARACTERS &&
            source.all { it in '0'..'9' || it in 'a'..'f' }

    fun encode(input: RefreshWorkInput): String? =
        try {
            json.encodeToString(RefreshWorkInput.serializer(), input)
                .takeIf { it.toByteArray(Charsets.UTF_8).size <= MAX_WORK_INPUT_BYTES }
        } catch (_: SerializationException) {
            null
        }

    fun data(input: RefreshWorkInput): Data? =
        encode(input)?.let { Data.Builder().putString(INPUT, it).build() }

    fun decode(data: Data): RefreshWorkInput? {
        val encoded = data.getString(INPUT) ?: return null
        if (encoded.toByteArray(Charsets.UTF_8).size > MAX_WORK_INPUT_BYTES) {
            return null
        }
        return try {
            json.decodeFromString(RefreshWorkInput.serializer(), encoded)
                .takeIf {
                    it.version == REFRESH_WORK_VERSION && validSourceId(it.source.id)
                }
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    private const val INPUT = "refresh"

    private const val SOURCE_ID_CHARACTERS = 32

    private const val MAX_WORK_INPUT_BYTES = 8 * 1024
}

internal fun RefreshSource.hasSameImportConfiguration(other: RefreshSource): Boolean =
    id == other.id &&
        provider == other.provider &&
        visibility == other.visibility &&
        providerRepositoryId == other.providerRepositoryId &&
        account == other.account &&
        remote == other.remote &&
        branch == other.branch &&
        notesFolder == other.notesFolder &&
        credential == other.credential

private const val REFRESH_WORK_VERSION = 1
