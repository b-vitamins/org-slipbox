/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import java.util.concurrent.atomic.AtomicBoolean

/** A contract failure without parser or native diagnostic text. */
class EngineContractException internal constructor(val fault: EngineFault) :
    RuntimeException(fault.summary)

/** An adapter refusal described only by closed tokens. */
class EngineRefusedException internal constructor(val refusal: AdapterResponse.Refused) :
    RuntimeException("the engine adapter refused: ${refusal.summary()}")

/** The session, or the owner that held it, has been disposed. */
class EngineClosedException internal constructor(message: String) : IllegalStateException(message)

/** Callback and cleanup failure categories. */
enum class EngineSecondaryFault {
    ANSWER_CALLBACK,
    FAILURE_CALLBACK,
    RETIREMENT,
    ;

    val summary: String
        get() =
            when (this) {
                ANSWER_CALLBACK -> "a callback failed on the answer it was given"
                FAILURE_CALLBACK -> "a callback failed on the failure it was told about"
                RETIREMENT -> "retiring a session failed outside this contract"
            }
}

/** A secondary failure without the original throwable's diagnostic graph. */
class EngineSecondaryException internal constructor(val fault: EngineSecondaryFault) :
    RuntimeException(fault.summary)

/** The observed result of a session retirement. */
data class EngineRetirement(
    val handle: Long,
    val retired: Boolean,
    val failure: Throwable? = null,
)

/** Retirement outcomes in the owner's session order, including failed attempts. */
data class EngineDisposal(val retirements: List<EngineRetirement>) {

    val isComplete: Boolean
        get() = retirements.all { it.retired && it.failure == null }

    val failures: List<Throwable>
        get() = retirements.mapNotNull { it.failure }
}

/**
 * Runs session work on a serial background lane. Disposal refuses new work and
 * orders session retirements after admitted work.
 */
class SlipboxEngineHost internal constructor(
    private val seam: NativeSeam,
    val contract: AdapterResponse.Contract,
) : AutoCloseable, DefaultLifecycleObserver {

    private val lane =
        EngineLane(
            LANE_NAME,
            contract.limits.maxQueuedRequests,
            contract.limits.laneCapacity(),
        )

    private val live = AtomicBoolean(true)

    private val sessions = mutableListOf<EngineSession>()

    /** Retained so repeated close calls share the outcome. */
    @Volatile
    private var disposal: EngineTask<EngineDisposal>? = null

    val isOpen: Boolean
        get() = live.get()

    fun openRead(
        binding: GenerationBinding,
        context: SessionContext,
    ): EngineTask<EngineReadSession> =
        open(AdapterCapability.READ, binding, context, seam::openRead) { handle ->
            EngineReadSession(this, handle, binding)
        }

    fun openMaintenance(
        binding: GenerationBinding,
        context: SessionContext,
    ): EngineTask<EngineMaintenanceSession> =
        open(AdapterCapability.MAINTENANCE, binding, context, seam::openMaintenance) { handle ->
            EngineMaintenanceSession(this, handle, binding)
        }

    /** Refuses further work at once and orders the retirements behind it. */
    override fun close() {
        synchronized(sessions) {
            if (!live.compareAndSet(true, false)) {
                return
            }
            val held = sessions.toList()
            sessions.clear()
            held.forEach { it.markRetired() }
            disposal = lane.retire { disposed(held) }
        }
    }

    override fun onDestroy(owner: LifecycleOwner) = close()

    /** The disposal result, or null while it is still under way. */
    fun awaitDisposal(timeoutMillis: Long): EngineDisposal? = disposal?.await(timeoutMillis)

    internal fun answer(
        session: EngineSession,
        operation: ReadOperation,
        callback: EngineCallback<ReadAnswer>?,
    ): EngineTask<ReadAnswer> {
        session.ensureOpen()
        val request = ReadRequest(session.handle, session.binding, operation)
        return lane.submit {
            delivered(callback, session::isOpen) {
                val response = call(EngineWire.encode(request), seam::read)
                operation.answerIn(answered(response, session))
                    ?: throw EngineContractException(EngineFault.FOREIGN_ANSWER)
            }
        }
    }

    internal fun carryOut(
        session: EngineSession,
        operation: MaintenanceOperation,
        callback: EngineCallback<MaintenanceAnswer>?,
    ): EngineTask<MaintenanceAnswer> {
        session.ensureOpen()
        val request = MaintenanceRequest(session.handle, session.binding, operation)
        return lane.submit {
            delivered(callback, session::isOpen) {
                val response = call(EngineWire.encode(request), seam::maintain)
                operation.answerIn(answered(response, session))
                    ?: throw EngineContractException(EngineFault.FOREIGN_ANSWER)
            }
        }
    }

    /** Shares one retirement task and retains unaccounted sessions for disposal. */
    internal fun retire(session: EngineSession): EngineTask<Boolean> =
        synchronized(sessions) {
            session.retiring {
                if (live.get()) {
                    lane.submitClosure { accounted(session) }
                } else {
                    // Disposal ordered its retirement, and reports what that did.
                    EngineTask.settled(false)
                }
            }
        }

    private fun <T : EngineSession> open(
        capability: AdapterCapability,
        binding: GenerationBinding,
        context: SessionContext,
        invoke: (ByteArray) -> ByteArray?,
        session: (Long) -> T,
    ): EngineTask<T> {
        ensureOpen()
        val request = OpenRequest(capability, binding, context)
        return lane.submit {
            val response = call(EngineWire.encode(request), invoke)
            val opened =
                response as? AdapterResponse.Opened
                    ?: throw EngineContractException(EngineFault.UNEXPECTED_OUTCOME)
            // The handle is this owner's from here on: whatever fails next retires it.
            val owned = session(opened.handle)
            if (opened.capability != capability || opened.binding != binding) {
                throw discarded(owned, EngineContractException(EngineFault.FOREIGN_SESSION))
            }
            register(owned)
            owned
        }
    }

    /** Discard a newly opened session if disposal won the registration race. */
    private fun register(session: EngineSession) {
        val held = synchronized(sessions) { live.get() && sessions.add(session) }
        if (!held) {
            throw discarded(session, EngineClosedException(DISPOSED_WHILE_OPENING))
        }
    }

    /** Retire an undelivered session, preserving the primary and cleanup failures. */
    private fun discarded(session: EngineSession, primary: Throwable): Throwable {
        session.markRetired()
        try {
            retireNatively(session)
        } catch (cleanup: Throwable) {
            primary.addSuppressed(secondary(cleanup))
        }
        return primary
    }

    /** Keep failed or unknown retirements in the owner's held sessions. */
    private fun accounted(session: EngineSession): Boolean {
        val retired =
            try {
                retireNatively(session)
            } catch (failure: Throwable) {
                session.outcome = EngineRetirement(session.handle, false, secondary(failure))
                throw failure
            }
        session.outcome = EngineRetirement(session.handle, retired)
        synchronized(sessions) { sessions.remove(session) }
        return retired
    }

    private fun disposed(held: List<EngineSession>): EngineDisposal =
        EngineDisposal(held.map { session -> session.outcome ?: attempted(session) })

    private fun attempted(session: EngineSession): EngineRetirement =
        try {
            EngineRetirement(session.handle, retireNatively(session))
        } catch (failure: Throwable) {
            EngineRetirement(session.handle, false, secondary(failure))
        }

    /** Validate the handle and binding reported by native retirement. */
    private fun retireNatively(session: EngineSession): Boolean {
        val request = CloseRequest(session.handle, session.binding)
        val response = call(EngineWire.encode(request), seam::closeSession)
        val closed =
            response as? AdapterResponse.Closed
                ?: throw EngineContractException(EngineFault.UNEXPECTED_OUTCOME)
        val retired = if (closed.retired) session.binding else null
        if (closed.handle != session.handle || closed.binding != retired) {
            throw EngineContractException(EngineFault.FOREIGN_RETIREMENT)
        }
        return closed.retired
    }

    private fun call(request: ByteArray, invoke: (ByteArray) -> ByteArray?): AdapterResponse {
        if (request.size > contract.limits.maxRequestBytes) {
            throw EngineRefusedException(bounded(AdapterBound.REQUEST_BYTES))
        }
        val answer =
            invoke(request) ?: throw EngineContractException(EngineFault.NO_ANSWER)
        if (answer.size > contract.limits.maxResponseBytes) {
            throw EngineRefusedException(bounded(AdapterBound.RESPONSE_BYTES))
        }
        val response = EngineWire.decode(answer)
        if (response.version != ADAPTER_PROTOCOL_VERSION) {
            throw EngineContractException(EngineFault.UNSUPPORTED_VERSION)
        }
        if (response is AdapterResponse.Refused) {
            throw EngineRefusedException(response)
        }
        return response
    }

    private fun answered(response: AdapterResponse, session: EngineSession): EngineAnswer {
        val answered =
            response as? AdapterResponse.Answered
                ?: throw EngineContractException(EngineFault.UNEXPECTED_OUTCOME)
        if (answered.handle != session.handle || answered.binding != session.binding) {
            throw EngineContractException(EngineFault.FOREIGN_ANSWER)
        }
        return answered.answer
    }

    private fun ensureOpen() {
        if (!live.get()) {
            throw EngineClosedException("the owner has been disposed")
        }
    }

    companion object {

        private const val LANE_NAME = "slipbox-engine"

        private const val DISPOSED_WHILE_OPENING =
            "the owner was disposed before the session opened"

        fun packaged(): SlipboxEngineHost {
            val failure = SlipboxNativeEngine.loadFailure
            check(failure == null) { "the packaged engine did not load: $failure" }
            return over(SlipboxNativeEngine.seam)
        }

        /** Validate the native declaration before allocating the lane. */
        internal fun over(seam: NativeSeam): SlipboxEngineHost {
            val answer =
                seam.contract() ?: throw EngineContractException(EngineFault.NO_CONTRACT)
            if (answer.size > MAX_CONTRACT_BYTES) {
                throw EngineContractException(EngineFault.OVERSIZED_CONTRACT)
            }
            val contract =
                EngineWire.decode(answer) as? AdapterResponse.Contract
                    ?: throw EngineContractException(EngineFault.UNEXPECTED_OUTCOME)
            if (contract.version != ADAPTER_PROTOCOL_VERSION) {
                throw EngineContractException(EngineFault.UNSUPPORTED_VERSION)
            }
            if (answer.size > contract.limits.maxResponseBytes) {
                throw EngineContractException(EngineFault.OVERSIZED_CONTRACT)
            }
            if (!contract.limits.withinPolicy()) {
                throw EngineContractException(EngineFault.LIMIT_MISMATCH)
            }
            if (!contract.readOperations.admits(EngineWire.readOperations) ||
                !contract.maintenanceOperations.admits(EngineWire.maintenanceOperations)
            ) {
                throw EngineContractException(EngineFault.VOCABULARY_MISMATCH)
            }
            return SlipboxEngineHost(seam, contract)
        }
    }
}

/** One session: one capability and one binding, from opening to retirement. */
sealed class EngineSession protected constructor(
    internal val host: SlipboxEngineHost,
    val handle: Long,
    val binding: GenerationBinding,
) {

    private val live = AtomicBoolean(true)

    /** Accessed under the owner's session lock. */
    private var ordered: EngineTask<Boolean>? = null

    /** Failed retirement outcome retained for host disposal. */
    @Volatile
    internal var outcome: EngineRetirement? = null

    val isOpen: Boolean
        get() = live.get() && host.isOpen

    /**
     * Orders closure after admitted work and retires pending callbacks.
     * Repeated calls share the retirement task; running callbacks are not interrupted.
     */
    fun retire(): EngineTask<Boolean> = host.retire(this)

    internal fun retiring(order: () -> EngineTask<Boolean>): EngineTask<Boolean> {
        ordered?.let { return it }
        markRetired()
        return order().also { ordered = it }
    }

    internal fun markRetired(): Boolean = live.compareAndSet(true, false)

    internal fun ensureOpen() {
        if (!isOpen) {
            throw EngineClosedException("the session has been retired")
        }
    }
}

/** Reading one bound generation: no operation here writes anything. */
class EngineReadSession internal constructor(
    host: SlipboxEngineHost,
    handle: Long,
    binding: GenerationBinding,
) : EngineSession(host, handle, binding) {

    fun answer(
        operation: ReadOperation,
        callback: EngineCallback<ReadAnswer>? = null,
    ): EngineTask<ReadAnswer> = host.answer(this, operation, callback)
}

/** Internal index maintenance, which no reading session can reach. */
class EngineMaintenanceSession internal constructor(
    host: SlipboxEngineHost,
    handle: Long,
    binding: GenerationBinding,
) : EngineSession(host, handle, binding) {

    fun carryOut(
        operation: MaintenanceOperation,
        callback: EngineCallback<MaintenanceAnswer>? = null,
    ): EngineTask<MaintenanceAnswer> = host.carryOut(this, operation, callback)
}

/**
 * Admit callbacks only while live and report callback failures as safe categories.
 */
private fun <T> delivered(callback: EngineCallback<T>?, live: () -> Boolean, work: () -> T): T {
    val answer =
        try {
            work()
        } catch (failure: Throwable) {
            if (live()) {
                try {
                    callback?.onFailed(failure)
                } catch (reporting: Throwable) {
                    failure.addSuppressed(
                        EngineSecondaryException(EngineSecondaryFault.FAILURE_CALLBACK),
                    )
                }
            }
            throw failure
        }
    if (live()) {
        try {
            callback?.onAnswered(answer)
        } catch (reporting: Throwable) {
            throw EngineSecondaryException(EngineSecondaryFault.ANSWER_CALLBACK)
        }
    }
    return answer
}

/** Keep adapter categories; discard external cleanup diagnostic text. */
private fun secondary(failure: Throwable): Throwable =
    when (failure) {
        is EngineContractException, is EngineRefusedException, is EngineClosedException -> failure
        else -> EngineSecondaryException(EngineSecondaryFault.RETIREMENT)
    }

private fun List<String>.admits(vocabulary: Set<String>): Boolean =
    size == vocabulary.size && toSet() == vocabulary

private fun bounded(bound: AdapterBound): AdapterResponse.Refused =
    AdapterResponse.Refused(
        reason = RefusalReason.OUT_OF_BOUNDS,
        bound = bound,
        version = ADAPTER_PROTOCOL_VERSION,
    )
