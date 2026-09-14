/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.manual

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
import io.github.b_vitamins.slipbox.auth.AuthorizationVault
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultProbeRun
import io.github.b_vitamins.slipbox.security.VaultReauthorization
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.security.VaultScope
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPanel
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPhase
import io.github.b_vitamins.slipbox.ui.auth.GithubAuthorizationState
import io.github.b_vitamins.slipbox.ui.auth.rememberGithubAuthorization
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/** Explicitly invoked consent and scoped-vault smoke. See doc/android-auth.org. */
class LiveAuthorizationJourney : Instrumentation() {

    private val given = AtomicReference(Bundle())

    private val kept = mutableListOf<VaultScope>()

    override fun onCreate(arguments: Bundle) {
        super.onCreate(arguments)
        given.set(arguments)
        start()
    }

    override fun onStart() {
        val request =
            when (val read = liveRequestOf(given.get())) {
                is LiveRequest.Refused -> return refuse(read.reason)
                is LiveRequest.Given -> read
            }
        val report = LiveJourneyReport(request.stage)
        try {
            walk(request, report)
        } catch (fault: Throwable) {
            report.refusal = fault.javaClass.name
        } finally {
            publish(request.results, report)
        }
    }

    private fun walk(request: LiveRequest.Given, report: LiveJourneyReport) {
        val vaultRun = VaultProbeRun(targetContext)
        try {
            report.configured = GithubApp.packaged() != null
            if (report.configured) {
                panel(request, report, vaultRun)
            }
        } finally {
            report.ownedCleanup = released(vaultRun)
        }
    }

    private fun panel(
        request: LiveRequest.Given,
        report: LiveJourneyReport,
        vaultRun: VaultProbeRun,
    ) {
        val activity = host()
        val state = mounted(activity)
        try {
            if (request.stage == LiveStage.Grant) {
                consent(state, request, report, vaultRun)
            }
            report.cancellation = cancelled(state, report)
        } finally {
            runOnMainSync {
                state.dispose()
                activity.finish()
            }
            waitForIdleSync()
        }
    }

    /** Uses the ComponentActivity already declared by the instrumentation dependencies. */
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

    private fun consent(
        state: GithubAuthorizationState,
        request: LiveRequest.Given,
        report: LiveJourneyReport,
        vaultRun: VaultProbeRun,
    ) {
        if (!requested(state)) {
            return
        }
        report.codeShown = true
        status("the code is on the device screen; enter it at GitHub and approve")
        val settled =
            reached(request.consentMillis) { phase(state) as? AuthorizationPhase.Settled }
        if (settled == null) {
            runOnMainSync { state.cancel() }
            return
        }
        val authorization =
            (settled.outcome as? AuthorizationOutcome.Authorized)?.authorization ?: return
        report.authorized = true
        val gate = LiveGate(authorization, checkNotNull(request.expectedAccountId))
        report.identityMatch = gate.identityMatch
        report.selectedInstallation = gate.selectedInstallation
        keep(authorization, vaultRun, report)
    }

    private fun requested(state: GithubAuthorizationState): Boolean {
        runOnMainSync { state.begin() }
        return reached(GRANT_MILLIS) {
            when (phase(state)) {
                is AuthorizationPhase.Verifying -> true
                is AuthorizationPhase.Settled -> false
                else -> null
            }
        } ?: false
    }

    private fun keep(
        authorization: GithubAuthorization,
        vaultRun: VaultProbeRun,
        report: LiveJourneyReport,
    ) {
        val vault = AuthorizationVault(targetContext, vaultRun.namespace)
        val scope =
            when (val named = vault.scopeFor(SOURCE_ID, CREDENTIAL_REF, authorization.account)) {
                is VaultOutcome.Failed -> return
                is VaultOutcome.Completed -> named.value
            }
        kept.add(scope)
        val answered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<VaultReauthorization>?>(null)
        vault.keep(
            sourceId = SOURCE_ID,
            credentialRef = CREDENTIAL_REF,
            authorization = authorization,
            recipient =
                VaultRecipient { outcome ->
                    answer.set(outcome)
                    answered.countDown()
                },
        )
        if (!answered.await(VAULT_MILLIS, TimeUnit.MILLISECONDS)) {
            return
        }
        report.vaultHandoff = answer.get() is VaultOutcome.Completed
        if (report.vaultHandoff) {
            report.vaultReadback = readsBack(vaultRun, scope, authorization)
        }
    }

    /** Delivery precedes scoped-record release, so reopening may need to wait. */
    private fun readsBack(
        vaultRun: VaultProbeRun,
        scope: VaultScope,
        authorization: GithubAuthorization,
    ): Boolean {
        val reopened =
            reached(VAULT_MILLIS) {
                when (val opened = SlipboxVault.open(targetContext, vaultRun.namespace, scope)) {
                    is VaultOutcome.Failed -> null
                    is VaultOutcome.Completed -> opened.value
                }
            } ?: return false
        return try {
            when (val stored = reopened.read()) {
                is VaultOutcome.Failed -> false
                is VaultOutcome.Completed -> stored.value == authorization.credential
            }
        } finally {
            reopened.close()
            reopened.awaitClosed(VAULT_MILLIS)
        }
    }

    private fun cancelled(state: GithubAuthorizationState, report: LiveJourneyReport): Boolean {
        if (!requested(state)) {
            return false
        }
        report.codeShown = true
        runOnMainSync { state.cancel() }
        if (phase(state) !is AuthorizationPhase.Idle || state.owner.running() != null) {
            return false
        }
        Thread.sleep(QUIET_MILLIS)
        return phase(state) is AuthorizationPhase.Idle
    }

    private fun released(vaultRun: VaultProbeRun): Boolean {
        var removed = true
        for (scope in kept) {
            removed =
                vaultRun.policy.removeCredential(scope) is VaultOutcome.Completed &&
                    vaultRun.policy.removeAccount(scope) is VaultOutcome.Completed &&
                    removed
        }
        val remains = vaultRun.release()
        return removed && remains.aliases.isEmpty() && !remains.rootExists
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
        val outcome = Bundle().apply { putString(STREAM, "FAIL live-auth: $reason\n") }
        finish(Activity.RESULT_CANCELED, outcome)
    }

    private fun publish(results: File, report: LiveJourneyReport) {
        val json = report.json()
        File(results, RESULT_FILE).writeText("$json\n")
        val outcome = Bundle().apply { putString(STREAM, "${report.summary()}\n$json\n") }
        finish(if (report.passed) Activity.RESULT_OK else Activity.RESULT_CANCELED, outcome)
    }

    private companion object {

        const val STREAM = "stream"

        const val IN_PROGRESS = 0

        const val RESULT_FILE = "live-auth.json"

        const val SOURCE_ID = "live-auth-source-1"

        const val CREDENTIAL_REF = "live-auth-credential-1"

        const val MOUNT_MILLIS = 30_000L

        const val GRANT_MILLIS = 60_000L

        const val VAULT_MILLIS = 30_000L

        const val QUIET_MILLIS = 10_000L

        const val POLL_MILLIS = 100L
    }
}
