/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sync

import android.content.Context
import io.github.b_vitamins.slipbox.auth.renewal.CredentialRenewalOwner
import io.github.b_vitamins.slipbox.auth.renewal.RenewalFault
import io.github.b_vitamins.slipbox.auth.renewal.RenewalOutcome
import io.github.b_vitamins.slipbox.git.NativeGitSeam
import io.github.b_vitamins.slipbox.git.SlipboxNativeGit
import io.github.b_vitamins.slipbox.security.ForegroundThread
import io.github.b_vitamins.slipbox.security.SlipboxVault
import java.io.File

class SourceRefresh(
    val operation: Long,
    val attempt: Int,
    val source: RefreshSource,
    val repository: File,
    val store: File,
) {

    override fun toString(): String = "SourceRefresh($operation, ${source.id})"
}

sealed interface SourceRefreshOutcome {

    data class Answered(
        val disposition: RefreshDisposition,
        val status: RefreshStatus,
    ) : SourceRefreshOutcome

    data class Refused(val failure: RefreshFailure) : SourceRefreshOutcome

    data class ContractFailed(val fault: RefreshContractFault) : SourceRefreshOutcome
}

sealed interface SourceRefreshStatusOutcome {

    data class Known(val status: RefreshStatus) : SourceRefreshStatusOutcome

    data object Idle : SourceRefreshStatusOutcome

    data class Refused(val reason: RefreshFailureReason) : SourceRefreshStatusOutcome

    data class ContractFailed(val fault: RefreshContractFault) : SourceRefreshStatusOutcome
}

/**
 * Blocking bridge from platform-owned scheduling to the complete Rust refresh
 * path. This class does no Org parsing, indexing or source mutation.
 */
class SourceRefreshCoordinator internal constructor(
    private val seam: NativeGitSeam,
    private val foreground: ForegroundThread,
    private val initialized: Boolean = true,
) {

    fun refresh(
        request: SourceRefresh,
        credentials: CredentialRenewalOwner? = null,
    ): SourceRefreshOutcome {
        if (foreground.isCurrent()) {
            return refused(
                RefreshFailureReason.FOREGROUND_REFUSED,
                RefreshRetry.Never,
            )
        }
        if (!initialized) {
            return refused(
                RefreshFailureReason.TLS_INITIALIZATION_FAILED,
                retry(request.attempt),
            )
        }
        if (request.operation <= 0 || request.attempt !in 0 until MAX_REFRESH_ATTEMPTS) {
            return refused(RefreshFailureReason.MALFORMED_REQUEST, RefreshRetry.Never)
        }
        if (credentials != null && !credentials.matches(request.source)) {
            return refused(RefreshFailureReason.AUTHORIZATION_FAILED, RefreshRetry.Never)
        }
        val encoded =
            RefreshWire.encode(
                RefreshRequest(
                    operation = request.operation,
                    attempt = request.attempt,
                    source = request.source,
                    repository = request.repository.absolutePath,
                    store = request.store.absolutePath,
                ),
            )
        if (encoded.size > MAX_REFRESH_REQUEST_BYTES) {
            return SourceRefreshOutcome.ContractFailed(RefreshContractFault.REQUEST_OVERSIZED)
        }
        val credential =
            when (val renewed = credentials?.credential()) {
                null -> null
                is RenewalOutcome.Usable -> renewed.credential.accessToken.toByteArray(Charsets.UTF_8)
                is RenewalOutcome.Reauthorize ->
                    return refused(
                        RefreshFailureReason.AUTHORIZATION_REQUIRED,
                        RefreshRetry.Reauthorize,
                    )
                is RenewalOutcome.Uncommitted ->
                    return refused(RefreshFailureReason.STORAGE_FAILED, retry(request.attempt))
                is RenewalOutcome.Unavailable -> return unavailable(renewed.fault, request.attempt)
                RenewalOutcome.Withdrawn ->
                    return refused(
                        RefreshFailureReason.AUTHORIZATION_REQUIRED,
                        RefreshRetry.Reauthorize,
                    )
            }
        return try {
            invoke(request, encoded, credential)
        } finally {
            credential?.fill(0)
        }
    }

    /** Status lookup is memory-only and safe for foreground presentation. */
    fun status(source: String): SourceRefreshStatusOutcome {
        val encoded = RefreshWire.encode(RefreshStatusRequest(source))
        if (encoded.size > MAX_REFRESH_REQUEST_BYTES) {
            return SourceRefreshStatusOutcome.ContractFailed(
                RefreshContractFault.REQUEST_OVERSIZED,
            )
        }
        val answer =
            seam.refreshStatus(encoded)
                ?: return SourceRefreshStatusOutcome.ContractFailed(RefreshContractFault.NO_ANSWER)
        if (answer.size > MAX_REFRESH_RESPONSE_BYTES) {
            return SourceRefreshStatusOutcome.ContractFailed(
                RefreshContractFault.RESPONSE_OVERSIZED,
            )
        }
        val response =
            try {
                RefreshWire.decodeStatus(answer)
            } catch (failure: RefreshContractException) {
                return SourceRefreshStatusOutcome.ContractFailed(failure.fault)
            }
        if (response.version != REFRESH_PROTOCOL_VERSION) {
            return SourceRefreshStatusOutcome.ContractFailed(
                RefreshContractFault.UNSUPPORTED_VERSION,
            )
        }
        return when (val outcome = response.outcome) {
            is RefreshStatusOutcome.Known ->
                if (outcome.status.source.id == source) {
                    SourceRefreshStatusOutcome.Known(outcome.status)
                } else {
                    SourceRefreshStatusOutcome.ContractFailed(RefreshContractFault.FOREIGN_SOURCE)
                }
            is RefreshStatusOutcome.Idle ->
                if (outcome.source == source) {
                    SourceRefreshStatusOutcome.Idle
                } else {
                    SourceRefreshStatusOutcome.ContractFailed(RefreshContractFault.FOREIGN_SOURCE)
                }
            is RefreshStatusOutcome.Refused -> SourceRefreshStatusOutcome.Refused(outcome.reason)
        }
    }

    /** This is deliberately safe to call from the foreground thread. */
    fun cancel(operation: Long): Boolean = operation > 0 && seam.cancel(operation)

    private fun invoke(
        request: SourceRefresh,
        encoded: ByteArray,
        credential: ByteArray?,
    ): SourceRefreshOutcome {
        val answer =
            seam.refresh(encoded, credential)
                ?: return SourceRefreshOutcome.ContractFailed(RefreshContractFault.NO_ANSWER)
        if (answer.size > MAX_REFRESH_RESPONSE_BYTES) {
            return SourceRefreshOutcome.ContractFailed(RefreshContractFault.RESPONSE_OVERSIZED)
        }
        val response =
            try {
                RefreshWire.decodeResponse(answer)
            } catch (failure: RefreshContractException) {
                return SourceRefreshOutcome.ContractFailed(failure.fault)
            }
        if (response.version != REFRESH_PROTOCOL_VERSION) {
            return SourceRefreshOutcome.ContractFailed(RefreshContractFault.UNSUPPORTED_VERSION)
        }
        return when (response) {
            is RefreshResponse.Refused ->
                SourceRefreshOutcome.Refused(RefreshFailure(response.reason, response.retry))
            is RefreshResponse.Answered ->
                when {
                    response.operation != request.operation ->
                        SourceRefreshOutcome.ContractFailed(RefreshContractFault.FOREIGN_OPERATION)
                    response.status.operation != request.operation ->
                        SourceRefreshOutcome.ContractFailed(RefreshContractFault.FOREIGN_OPERATION)
                    response.status.source != request.source ->
                        SourceRefreshOutcome.ContractFailed(RefreshContractFault.FOREIGN_SOURCE)
                    else -> SourceRefreshOutcome.Answered(response.disposition, response.status)
                }
        }
    }

    private fun unavailable(fault: RenewalFault, attempt: Int): SourceRefreshOutcome {
        val rateLimited = fault is RenewalFault.UnexpectedStatus && fault.status == 429
        return refused(
            if (rateLimited) RefreshFailureReason.RATE_LIMITED else RefreshFailureReason.TRANSPORT_FAILED,
            retry(attempt),
        )
    }

    private fun retry(attempt: Int): RefreshRetry =
        if (attempt + 1 >= MAX_REFRESH_ATTEMPTS) {
            RefreshRetry.Never
        } else {
            RefreshRetry.Backoff(30L shl (attempt * 2))
        }

    private fun refused(
        reason: RefreshFailureReason,
        retry: RefreshRetry,
    ): SourceRefreshOutcome = SourceRefreshOutcome.Refused(RefreshFailure(reason, retry))

    private fun CredentialRenewalOwner.matches(source: RefreshSource): Boolean =
        source.provider == RefreshProvider.GITHUB &&
            source.visibility == RefreshVisibility.PRIVATE &&
            request.sourceId == source.id &&
            request.account.id == source.account &&
            request.credentialRef == source.credential

    companion object {

        fun packaged(context: Context): SourceRefreshCoordinator {
            check(SlipboxNativeGit.loadFailure == null) {
                "the packaged native library did not load"
            }
            return SourceRefreshCoordinator(
                SlipboxNativeGit.seam,
                SlipboxVault.MainThread,
                SlipboxNativeGit.initialize(context),
            )
        }
    }
}
