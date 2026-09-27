/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git.manual

import android.app.Activity
import android.app.Instrumentation
import android.os.Bundle
import io.github.b_vitamins.slipbox.auth.AuthorizationClock
import io.github.b_vitamins.slipbox.auth.AuthorizationReply
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.auth.renewal.CredentialRenewalOwner
import io.github.b_vitamins.slipbox.auth.renewal.RenewalRequest
import io.github.b_vitamins.slipbox.auth.renewal.SlipboxRenewalStorage
import io.github.b_vitamins.slipbox.git.GitDisposition
import io.github.b_vitamins.slipbox.git.GitMaterialization
import io.github.b_vitamins.slipbox.git.GitMaterializationOutcome
import io.github.b_vitamins.slipbox.git.GitRepositoryTransport
import io.github.b_vitamins.slipbox.git.GitSnapshotDisposition
import io.github.b_vitamins.slipbox.git.GitSynchronization
import io.github.b_vitamins.slipbox.git.GitSynchronizationOutcome
import io.github.b_vitamins.slipbox.security.ForegroundThread
import io.github.b_vitamins.slipbox.security.PrivateStore
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.VaultProbeRun
import io.github.b_vitamins.slipbox.security.completed
import java.io.File
import java.util.concurrent.atomic.AtomicLong
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

/** Explicit private Git smoke; secret inputs enter through application-private files. */
class LiveGitJourney : Instrumentation() {

    private var results: File? = null

    override fun onCreate(arguments: Bundle) {
        super.onCreate(arguments)
        results = resultDirectory(arguments)
        start()
    }

    override fun onStart() {
        val report = GitJourneyReport()
        try {
            walk(report)
        } catch (fault: Throwable) {
            report.refusal = fault.javaClass.name
        } finally {
            publish(checkNotNull(results), report)
        }
    }

    private fun walk(report: GitJourneyReport) {
        val input = readInput() ?: return
        report.inputAccepted = true
        val run = VaultProbeRun(targetContext)
        val scope =
            run.scope(
                sourceId = SOURCE_ID,
                providerAuthority = GithubApp.PROVIDER_AUTHORITY,
                accountId = ACCOUNT_ID,
                credentialRef = CREDENTIAL_REF,
            )
        val request = RenewalRequest(SOURCE_ID, VerifiedAccount(ACCOUNT_ID, LOGIN), CREDENTIAL_REF)
        val refresh = PatRefresh(input.token)
        var owner: CredentialRenewalOwner? = null
        var repositoryRoot: File? = null
        try {
            run.using(scope) { vault ->
                vault.reauthorize(StoredCredential(SPENT_ACCESS, REFRESH_MARKER, 0)).completed()
            }
            owner =
                CredentialRenewalOwner(
                    request = request,
                    app = requireNotNull(GithubApp.of(CLIENT_ID, INSTALLATION_URL)),
                    storage = SlipboxRenewalStorage(targetContext, run.namespace),
                    transport = refresh,
                    clock = FixedClock,
                    foreground = ForegroundThread.None,
            )
            val checkout = run.policy.directoryFor(PrivateStore.Checkout, scope).completed()
            repositoryRoot = File(checkout, REPOSITORY_ROOT_NAME)
            check(!repositoryRoot.exists() || repositoryRoot.deleteRecursively()) {
                "the previous Git fixture could not be removed"
            }
            val transport = GitRepositoryTransport.packaged(targetContext)
            val public =
                transport.synchronize(
                    GitSynchronization(
                        operation(),
                        PUBLIC_REMOTE,
                        PUBLIC_BRANCH,
                        File(repositoryRoot, PUBLIC_REPOSITORY_NAME),
                    ),
                )
            report.publicClone =
                public is GitSynchronizationOutcome.Fetched &&
                    public.disposition == GitDisposition.CLONED
            if (!report.publicClone) {
                report.refusal = classify("public", public)
                return
            }
            val publicRevision = (public as GitSynchronizationOutcome.Fetched).revision
            val materialized =
                transport.materialize(
                    GitMaterialization(
                        operation = operation(),
                        source = SNAPSHOT_SOURCE_ID,
                        repository = File(repositoryRoot, PUBLIC_REPOSITORY_NAME),
                        revision = publicRevision,
                        notesFolder = "",
                        snapshot = File(repositoryRoot, PUBLIC_SNAPSHOT_NAME),
                    ),
                )
            report.publicMaterialized =
                materialized is GitMaterializationOutcome.Materialized &&
                    materialized.disposition == GitSnapshotDisposition.CREATED &&
                    materialized.revision == publicRevision &&
                    materialized.orgFiles > 0 &&
                    materialized.files >= materialized.orgFiles
            if (!report.publicMaterialized) {
                report.refusal = classify("materialize", materialized)
                return
            }
            val repository = File(repositoryRoot, PRIVATE_REPOSITORY_NAME)
            val first =
                transport.synchronize(
                    GitSynchronization(operation(), input.remote, input.branch, repository),
                    owner,
                )
            report.tokenRenewed = refresh.exchanges == 1
            report.privateClone =
                first is GitSynchronizationOutcome.Fetched &&
                    first.disposition == GitDisposition.CLONED
            if (!report.privateClone) {
                report.refusal = classify("private", first)
                return
            }
            report.renewalCommitted =
                run.using(scope) { vault -> vault.read().completed()?.accessToken == input.token }
            val second =
                transport.synchronize(
                    GitSynchronization(operation(), input.remote, input.branch, repository),
                    owner,
                )
            report.authenticatedFetch =
                second is GitSynchronizationOutcome.Fetched &&
                    second.disposition in setOf(GitDisposition.UPDATED, GitDisposition.UNCHANGED) &&
                    refresh.exchanges == 1
            if (!report.authenticatedFetch) {
                report.refusal = classify("fetch", second)
            }
        } finally {
            owner?.close()
            input.erase()
            val checkoutClean =
                repositoryRoot?.let { root ->
                    (!root.exists() || root.deleteRecursively()) && !root.exists()
                } ?: true
            val remains = run.release()
            report.ownedCleanup =
                checkoutClean && remains.aliases.isEmpty() && !remains.rootExists
        }
    }

    private fun classify(stage: String, outcome: GitSynchronizationOutcome): String =
        when (outcome) {
            is GitSynchronizationOutcome.Fetched ->
                "$stage-${outcome.disposition.name.lowercase()}"
            is GitSynchronizationOutcome.Refused ->
                "$stage-${outcome.reason.name.lowercase()}"
            is GitSynchronizationOutcome.AccessRefused ->
                "$stage-${outcome.reason.name.lowercase()}"
            is GitSynchronizationOutcome.ContractFailed ->
                "$stage-${outcome.fault.name.lowercase()}"
        }

    private fun classify(stage: String, outcome: GitMaterializationOutcome): String =
        when (outcome) {
            is GitMaterializationOutcome.Materialized ->
                "$stage-${outcome.disposition.name.lowercase()}"
            is GitMaterializationOutcome.Refused ->
                "$stage-${outcome.reason.name.lowercase()}"
            is GitMaterializationOutcome.ContractFailed ->
                "$stage-${outcome.fault.name.lowercase()}"
        }

    private fun readInput(): GitInput? {
        val directory = File(targetContext.filesDir, INPUT_DIRECTORY)
        val tokenFile = File(directory, TOKEN_FILE)
        val remoteFile = File(directory, REMOTE_FILE)
        val branchFile = File(directory, BRANCH_FILE)
        val token = bounded(tokenFile, MAX_TOKEN_BYTES)
        val remoteBytes = bounded(remoteFile, MAX_REMOTE_BYTES)
        val branchBytes = bounded(branchFile, MAX_BRANCH_BYTES)
        tokenFile.delete()
        remoteFile.delete()
        branchFile.delete()
        directory.delete()
        if (token == null || remoteBytes == null || branchBytes == null) {
            token?.fill(0)
            remoteBytes?.fill(0)
            branchBytes?.fill(0)
            return null
        }
        val remote = remoteBytes.toString(Charsets.UTF_8)
        val branch = branchBytes.toString(Charsets.UTF_8)
        remoteBytes.fill(0)
        branchBytes.fill(0)
        if (!remote.startsWith("https://github.com/") || branch.isBlank()) {
            token.fill(0)
            return null
        }
        return GitInput(token.toString(Charsets.UTF_8), remote, branch, token)
    }

    private fun bounded(file: File, limit: Int): ByteArray? {
        if (!file.isFile || file.length() !in 1..limit.toLong()) {
            return null
        }
        return file.readBytes().takeIf { bytes -> bytes.all { it.toInt() in 0x21..0x7e } }
    }

    private fun resultDirectory(arguments: Bundle): File {
        val path = arguments.getString(RESULT_ARGUMENT)?.takeIf { it.startsWith('/') }
            ?: throw IllegalArgumentException("the result directory is absent")
        val directory = File(path)
        require(directory.isDirectory || directory.mkdirs()) {
            "the result directory is unavailable"
        }
        return directory
    }

    private fun publish(directory: File, report: GitJourneyReport) {
        val json = report.json()
        File(directory, RESULT_FILE).writeText("$json\n")
        val outcome = Bundle().apply { putString(STREAM, "${report.summary()}\n$json\n") }
        finish(if (report.passed) Activity.RESULT_OK else Activity.RESULT_CANCELED, outcome)
    }

    private class GitInput(
        val token: String,
        val remote: String,
        val branch: String,
        private val tokenBytes: ByteArray,
    ) {

        fun erase() {
            tokenBytes.fill(0)
        }
    }

    private class PatRefresh(private val token: String) : AuthorizationTransport {

        var exchanges: Int = 0
            private set

        override fun exchange(request: io.github.b_vitamins.slipbox.auth.AuthorizationRequest): AuthorizationReply {
            exchanges += 1
            if (request.bearer != null || request.form?.get("refresh_token") != REFRESH_MARKER) {
                return AuthorizationReply.Failed("refresh contract refused")
            }
            return AuthorizationReply.Answered(
                status = 200,
                body =
                    buildJsonObject {
                        put("access_token", JsonPrimitive(token))
                        put("token_type", JsonPrimitive("bearer"))
                    }.toString(),
            )
        }
    }

    private object FixedClock : AuthorizationClock {

        override fun elapsedMillis(): Long = 0

        override fun epochSeconds(): Long = 1_000_000

        override fun waitFor(millis: Long): Boolean = true
    }

    private companion object {

        const val STREAM = "stream"
        const val RESULT_ARGUMENT = "resultDir"
        const val RESULT_FILE = "live-git.json"
        const val INPUT_DIRECTORY = "live-git-input"
        const val TOKEN_FILE = "token"
        const val REMOTE_FILE = "remote"
        const val BRANCH_FILE = "branch"
        const val REPOSITORY_ROOT_NAME = "live-git"
        const val PUBLIC_REPOSITORY_NAME = "public.git"
        const val PUBLIC_SNAPSHOT_NAME = "public-snapshot"
        const val PRIVATE_REPOSITORY_NAME = "private.git"
        const val PUBLIC_REMOTE = "https://github.com/b-vitamins/org-slipbox.git"
        const val PUBLIC_BRANCH = "master"
        const val SOURCE_ID = "live-git-source"
        const val SNAPSHOT_SOURCE_ID = "0123456789abcdef0123456789abcdef"
        const val ACCOUNT_ID = "1"
        const val LOGIN = "live-git"
        const val CREDENTIAL_REF = "live-git-credential"
        const val CLIENT_ID = "live-git-client"
        const val INSTALLATION_URL = "https://github.com/settings/installations"
        const val SPENT_ACCESS = "spent-access"
        const val REFRESH_MARKER = "refresh-marker"
        const val MAX_TOKEN_BYTES = 4_096
        const val MAX_REMOTE_BYTES = 4_096
        const val MAX_BRANCH_BYTES = 1_024

        val nextOperation = AtomicLong(1)

        fun operation(): Long = nextOperation.getAndIncrement()
    }
}
