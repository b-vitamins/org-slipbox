/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github.manual

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Modifier
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.github.GithubInstallation
import io.github.b_vitamins.slipbox.github.GithubOutcome
import io.github.b_vitamins.slipbox.github.GithubProvider
import io.github.b_vitamins.slipbox.github.GithubRepository
import io.github.b_vitamins.slipbox.github.GithubSelectionRequest
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPanel
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPhase
import io.github.b_vitamins.slipbox.ui.auth.GithubAuthorizationState
import io.github.b_vitamins.slipbox.ui.auth.rememberGithubAuthorization
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import java.io.File
import java.util.concurrent.atomic.AtomicReference

/** Explicit manual discovery; approval stays on the device screen. */
class LiveProviderJourney : Instrumentation() {

    private val given = AtomicReference(Bundle())

    override fun onCreate(arguments: Bundle) {
        super.onCreate(arguments)
        given.set(arguments)
        start()
    }

    override fun onStart() {
        val request =
            when (val asked = providerRequestOf(given.get())) {
                is ProviderRequest.Refused -> return refuse(asked.reason)
                is ProviderRequest.Given -> asked
            }
        val report = ProviderJourneyReport()
        try {
            walk(request, report)
        } catch (fault: Throwable) {
            report.refusal = fault.javaClass.name
        } finally {
            publish(request.results, report)
        }
    }

    private fun walk(request: ProviderRequest.Given, report: ProviderJourneyReport) {
        report.configured = GithubApp.packaged() != null
        if (!report.configured) {
            return
        }
        val activity = host()
        val state = mounted(activity)
        try {
            granted(state, request, report)?.let { discover(it, request, report) }
        } finally {
            runOnMainSync {
                state.dispose()
                activity.finish()
            }
            waitForIdleSync()
            // The grant is in-memory only and belongs to the disposed panel.
            report.ownedCleanup =
                state.owner.running() == null && phase(state) is AuthorizationPhase.Idle
        }
    }

    private fun host(): ComponentActivity {
        val intent =
            Intent(targetContext, ComponentActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return startActivitySync(intent) as ComponentActivity
    }

    private fun mounted(activity: ComponentActivity): GithubAuthorizationState {
        val held = AtomicReference<GithubAuthorizationState?>(null)
        runOnMainSync {
            activity.setContent {
                val state = rememberGithubAuthorization()
                SideEffect { held.set(state) }
                SlipboxTheme {
                    Column(
                        modifier =
                            Modifier.fillMaxSize()
                                .background(SlipboxTheme.colors.paper)
                                .padding(SlipboxDimensions.readingPadding),
                    ) {
                        AuthorizationPanel(state = state)
                    }
                }
            }
        }
        return checkNotNull(reached(MOUNT_MILLIS) { held.get() }) {
            "the authorization panel never composed"
        }
    }

    private fun granted(
        state: GithubAuthorizationState,
        request: ProviderRequest.Given,
        report: ProviderJourneyReport,
    ): GithubAuthorization? {
        runOnMainSync { state.begin() }
        val waiting =
            reached(GRANT_MILLIS) {
                when (phase(state)) {
                    is AuthorizationPhase.Verifying -> true
                    is AuthorizationPhase.Settled -> false
                    else -> null
                }
            }
        if (waiting != true) {
            return null
        }
        report.codeShown = true
        status("the code is on the device screen; enter it at GitHub and approve")
        val settled = reached(request.consentMillis) { phase(state) as? AuthorizationPhase.Settled }
        if (settled == null) {
            runOnMainSync { state.cancel() }
            return null
        }
        val authorization =
            (settled.outcome as? AuthorizationOutcome.Authorized)?.authorization ?: return null
        report.authorized = true
        report.identityMatch = authorization.account.id == request.expectedAccountId
        return authorization.takeIf { report.identityMatch }
    }

    private fun discover(
        authorization: GithubAuthorization,
        request: ProviderRequest.Given,
        report: ProviderJourneyReport,
    ) {
        val provider = GithubProvider.packaged(authorization)
        val installations = value(provider.installations()) ?: return
        report.installationsListed = installations.entries.isNotEmpty()
        val found = located(provider, installations.entries, request) ?: return
        report.repositoryDiscoverable = true
        val branches = value(provider.branches(found.repository)) ?: return
        report.branchesListed = branches.entries.isNotEmpty() && branches.isComplete
        val branch = value(provider.branch(found.repository, request.branch)) ?: return
        report.branchResolved = branch.name == request.branch
        val folder = value(provider.folder(found.repository, branch, request.folder)) ?: return
        report.folderListed = folder.isComplete
        confirmed(provider, authorization, found, request, report)
    }

    private fun located(
        provider: GithubProvider,
        installations: List<GithubInstallation>,
        request: ProviderRequest.Given,
    ): Found? {
        for (installation in installations) {
            val repositories = value(provider.repositories(installation)) ?: continue
            val repository =
                repositories.entries.firstOrNull {
                    it.owner == request.owner && it.name == request.repository
                }
            if (repository != null) {
                return Found(installation, repository)
            }
        }
        return null
    }

    private fun confirmed(
        provider: GithubProvider,
        authorization: GithubAuthorization,
        found: Found,
        request: ProviderRequest.Given,
        report: ProviderJourneyReport,
    ) {
        val selection = selectionOf(authorization.account.id, found.repository.id, found, request)
        val confirmed = value(provider.confirm(selection))
        report.selectionConfirmed =
            confirmed != null &&
                confirmed.repositoryId == found.repository.id &&
                confirmed.installationId == found.installation.id &&
                confirmed.remoteUrl == found.repository.remoteUrl &&
                confirmed.branch == request.branch &&
                confirmed.notesFolder == request.folder
        report.foreignAccountRefused =
            provider.confirm(
                selectionOf(ABSENT_IDENTIFIER, found.repository.id, found, request),
            ) is GithubOutcome.Refused
        report.unauthorizedRepositoryRefused =
            provider.confirm(
                selectionOf(authorization.account.id, ABSENT_IDENTIFIER, found, request),
            ) is GithubOutcome.Refused
    }

    private fun selectionOf(
        accountId: String,
        repositoryId: String,
        found: Found,
        request: ProviderRequest.Given,
    ): GithubSelectionRequest =
        GithubSelectionRequest(
            accountId = accountId,
            installationId = found.installation.id,
            repositoryId = repositoryId,
            branch = request.branch,
            folder = request.folder,
        )

    private fun <T> value(outcome: GithubOutcome<T>): T? =
        when (outcome) {
            is GithubOutcome.Read -> outcome.value
            is GithubOutcome.Refused -> null
        }

    private fun phase(state: GithubAuthorizationState): AuthorizationPhase {
        val held = AtomicReference<AuthorizationPhase>(AuthorizationPhase.Idle)
        runOnMainSync { held.set(state.phase) }
        return held.get()
    }

    private fun <T> reached(limitMillis: Long, read: () -> T?): T? {
        val deadline = System.currentTimeMillis() + limitMillis
        while (true) {
            read()?.let { return it }
            if (System.currentTimeMillis() >= deadline) {
                return null
            }
            Thread.sleep(POLL_MILLIS)
        }
    }

    private fun status(note: String) {
        sendStatus(IN_PROGRESS, Bundle().apply { putString(STREAM, "$note\n") })
    }

    private fun refuse(reason: String) {
        val outcome = Bundle().apply { putString(STREAM, "FAIL live-github: $reason\n") }
        finish(Activity.RESULT_CANCELED, outcome)
    }

    private fun publish(results: File, report: ProviderJourneyReport) {
        val json = report.json()
        File(results, RESULT_FILE).writeText("$json\n")
        val outcome = Bundle().apply { putString(STREAM, "${report.summary()}\n$json\n") }
        finish(if (report.passed) Activity.RESULT_OK else Activity.RESULT_CANCELED, outcome)
    }

    private class Found(val installation: GithubInstallation, val repository: GithubRepository)

    private companion object {

        const val STREAM = "stream"

        const val IN_PROGRESS = 0

        const val RESULT_FILE = "live-github.json"

        const val MOUNT_MILLIS = 30_000L

        const val GRANT_MILLIS = 60_000L

        const val POLL_MILLIS = 100L
    }
}
