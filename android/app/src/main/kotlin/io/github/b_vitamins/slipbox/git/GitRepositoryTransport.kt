/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git

import android.content.Context
import io.github.b_vitamins.slipbox.auth.renewal.CredentialRenewalOwner
import io.github.b_vitamins.slipbox.auth.renewal.RenewalOutcome
import io.github.b_vitamins.slipbox.security.ForegroundThread
import io.github.b_vitamins.slipbox.security.SlipboxVault
import java.io.File

/** A credential-free description of one clone or fetch. */
class GitSynchronization(
    val operation: Long,
    val remote: String,
    val branch: String,
    val repository: File,
) {

    override fun toString(): String = "GitSynchronization($operation)"
}

sealed interface GitSynchronizationOutcome {

    data class Fetched(
        val disposition: GitDisposition,
        val revision: String,
        val receivedObjects: Long,
    ) : GitSynchronizationOutcome

    data class Refused(val reason: GitRefusalReason) : GitSynchronizationOutcome

    data class AccessRefused(val reason: GitAccessRefusal) : GitSynchronizationOutcome

    data class ContractFailed(val fault: GitContractFault) : GitSynchronizationOutcome
}

/** One exact fetched revision and its immutable candidate destination. */
class GitMaterialization(
    val operation: Long,
    val source: String,
    val repository: File,
    val revision: String,
    val notesFolder: String,
    val snapshot: File,
) {

    override fun toString(): String = "GitMaterialization($operation)"
}

sealed interface GitMaterializationOutcome {

    data class Materialized(
        val disposition: GitSnapshotDisposition,
        val revision: String,
        val entries: Long,
        val files: Long,
        val orgFiles: Long,
        val assets: Long,
        val bytes: Long,
        val diagnostics: List<GitSnapshotDiagnostic>,
    ) : GitMaterializationOutcome

    data class Refused(val reason: GitRefusalReason) : GitMaterializationOutcome

    data class ContractFailed(val fault: GitContractFault) : GitMaterializationOutcome
}

enum class GitAccessRefusal {
    REAUTHORIZE,
    UNCOMMITTED,
    UNAVAILABLE,
    WITHDRAWN,
}

/** A contract failure selected without native, path, URL or credential prose. */
class GitContractException internal constructor(val fault: GitContractFault) :
    RuntimeException("the native Git contract failed: ${fault.name.lowercase()}")

/**
 * Blocking native HTTPS Git. Callers own scheduling and may signal the
 * operation identity from another thread through [cancel].
 */
class GitRepositoryTransport internal constructor(
    private val seam: NativeGitSeam,
    private val foreground: ForegroundThread,
    private val initialized: Boolean = true,
) {

    fun synchronize(
        request: GitSynchronization,
        credentials: CredentialRenewalOwner? = null,
    ): GitSynchronizationOutcome {
        if (foreground.isCurrent()) {
            return GitSynchronizationOutcome.ContractFailed(GitContractFault.FOREGROUND_REFUSED)
        }
        if (!initialized) {
            return GitSynchronizationOutcome.ContractFailed(
                GitContractFault.TLS_INITIALIZATION_FAILED,
            )
        }
        val credential =
            when (val renewed = credentials?.credential()) {
                null -> null
                is RenewalOutcome.Usable -> renewed.credential.accessToken.toByteArray(Charsets.UTF_8)
                is RenewalOutcome.Reauthorize ->
                    return GitSynchronizationOutcome.AccessRefused(GitAccessRefusal.REAUTHORIZE)
                is RenewalOutcome.Uncommitted ->
                    return GitSynchronizationOutcome.AccessRefused(GitAccessRefusal.UNCOMMITTED)
                is RenewalOutcome.Unavailable ->
                    return GitSynchronizationOutcome.AccessRefused(GitAccessRefusal.UNAVAILABLE)
                RenewalOutcome.Withdrawn ->
                    return GitSynchronizationOutcome.AccessRefused(GitAccessRefusal.WITHDRAWN)
            }
        return try {
            synchronize(request, credential)
        } finally {
            credential?.fill(0)
        }
    }

    /** This is deliberately safe to call from the foreground thread. */
    fun cancel(operation: Long): Boolean = operation > 0 && seam.cancel(operation)

    /** Blocking object-to-filesystem materialization, with no network or credential access. */
    fun materialize(request: GitMaterialization): GitMaterializationOutcome {
        if (foreground.isCurrent()) {
            return GitMaterializationOutcome.ContractFailed(GitContractFault.FOREGROUND_REFUSED)
        }
        val encoded =
            GitWire.encode(
                GitMaterializeRequest(
                    operation = request.operation,
                    source = request.source,
                    repository = request.repository.absolutePath,
                    revision = request.revision,
                    notesFolder = request.notesFolder,
                    snapshot = request.snapshot.absolutePath,
                ),
            )
        if (encoded.size > MAX_GIT_REQUEST_BYTES) {
            return GitMaterializationOutcome.ContractFailed(GitContractFault.REQUEST_OVERSIZED)
        }
        val answer =
            seam.materialize(encoded)
                ?: return GitMaterializationOutcome.ContractFailed(GitContractFault.NO_ANSWER)
        if (answer.size > MAX_GIT_RESPONSE_BYTES) {
            return GitMaterializationOutcome.ContractFailed(GitContractFault.RESPONSE_OVERSIZED)
        }
        val response =
            try {
                GitWire.decode(answer)
            } catch (failure: GitContractException) {
                return GitMaterializationOutcome.ContractFailed(failure.fault)
            }
        if (response.version != GIT_PROTOCOL_VERSION) {
            return GitMaterializationOutcome.ContractFailed(GitContractFault.UNSUPPORTED_VERSION)
        }
        return when (response) {
            is GitResponse.Refused -> GitMaterializationOutcome.Refused(response.reason)
            is GitResponse.Fetched ->
                GitMaterializationOutcome.ContractFailed(GitContractFault.UNEXPECTED_OUTCOME)
            is GitResponse.Materialized -> {
                when {
                    response.operation != request.operation ->
                        GitMaterializationOutcome.ContractFailed(GitContractFault.FOREIGN_OPERATION)
                    response.revision != request.revision ->
                        GitMaterializationOutcome.ContractFailed(GitContractFault.FOREIGN_REVISION)
                    else ->
                        GitMaterializationOutcome.Materialized(
                            response.disposition,
                            response.revision,
                            response.entries,
                            response.files,
                            response.orgFiles,
                            response.assets,
                            response.bytes,
                            response.diagnostics,
                        )
                }
            }
        }
    }

    private fun synchronize(
        request: GitSynchronization,
        credential: ByteArray?,
    ): GitSynchronizationOutcome {
        val encoded =
            GitWire.encode(
                GitRequest(
                    operation = request.operation,
                    remote = request.remote,
                    branch = request.branch,
                    repository = request.repository.absolutePath,
                ),
            )
        if (encoded.size > MAX_GIT_REQUEST_BYTES) {
            return GitSynchronizationOutcome.ContractFailed(GitContractFault.REQUEST_OVERSIZED)
        }
        val answer =
            seam.synchronize(encoded, credential)
                ?: return GitSynchronizationOutcome.ContractFailed(GitContractFault.NO_ANSWER)
        if (answer.size > MAX_GIT_RESPONSE_BYTES) {
            return GitSynchronizationOutcome.ContractFailed(GitContractFault.RESPONSE_OVERSIZED)
        }
        val response =
            try {
                GitWire.decode(answer)
            } catch (failure: GitContractException) {
                return GitSynchronizationOutcome.ContractFailed(failure.fault)
            }
        if (response.version != GIT_PROTOCOL_VERSION) {
            return GitSynchronizationOutcome.ContractFailed(GitContractFault.UNSUPPORTED_VERSION)
        }
        return when (response) {
            is GitResponse.Refused -> GitSynchronizationOutcome.Refused(response.reason)
            is GitResponse.Materialized ->
                GitSynchronizationOutcome.ContractFailed(GitContractFault.UNEXPECTED_OUTCOME)
            is GitResponse.Fetched -> {
                if (response.operation != request.operation) {
                    GitSynchronizationOutcome.ContractFailed(GitContractFault.FOREIGN_OPERATION)
                } else {
                    GitSynchronizationOutcome.Fetched(
                        response.disposition,
                        response.revision,
                        response.receivedObjects,
                    )
                }
            }
        }
    }

    companion object {

        fun packaged(context: Context): GitRepositoryTransport {
            check(SlipboxNativeGit.loadFailure == null) {
                "the packaged native library did not load"
            }
            return GitRepositoryTransport(
                SlipboxNativeGit.seam,
                SlipboxVault.MainThread,
                SlipboxNativeGit.initialize(context),
            )
        }
    }
}
