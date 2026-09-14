/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.content.Context
import android.os.Looper
import java.io.File

/** Platform Keystore/AES-GCM vault factory rooted in application-private no-backup storage. */
object SlipboxVault {

    /** The thread that draws the screen, which a blocking vault call refuses. */
    val MainThread: ForegroundThread = ForegroundThread {
        Looper.myLooper() === Looper.getMainLooper()
    }

    /** The application's vault for [scope], or why there is none. */
    fun open(context: Context, scope: VaultScope): VaultOutcome<CredentialVault> =
        open(context, VaultNamespace.Application, scope)

    /** Opens off main, refusing a record already held by another vault. */
    fun open(
        context: Context,
        namespace: VaultNamespace,
        scope: VaultScope,
        faults: VaultDeliveryFaults = VaultDeliveryFaults.Ignored,
    ): VaultOutcome<CredentialVault> {
        val policy =
            when (val outcome = policy(context, namespace)) {
                is VaultOutcome.Failed -> return outcome
                is VaultOutcome.Completed -> outcome.value
            }
        val keys = AndroidKeystoreVaultKeys(namespace)
        return CredentialVault.open(
            scope = scope,
            namespace = namespace,
            keys = keys,
            cipher = AesGcmVaultCipher(keys),
            source = PolicyRecordSource(policy, scope),
            foreground = MainThread,
            faults = faults,
        )
    }

    /** Opens and delivers on a background thread; failed/disposed delivery closes the offered vault. */
    fun openAsync(
        context: Context,
        namespace: VaultNamespace,
        scope: VaultScope,
        recipient: VaultRecipient<CredentialVault>,
        faults: VaultDeliveryFaults = VaultDeliveryFaults.Ignored,
    ) {
        val thread =
            Thread({
                val opened = open(context, namespace, scope, faults)
                val delivered =
                    try {
                        recipient.offer(opened)
                    } catch (fault: Throwable) {
                        faults.report(originOf(fault))
                        false
                    }
                if (!delivered && opened is VaultOutcome.Completed) {
                    opened.value.close()
                }
            }, OPEN_THREAD)
        thread.isDaemon = true
        thread.start()
    }

    /** Where [namespace] may keep private data of this installation. */
    fun policy(
        context: Context,
        namespace: VaultNamespace = VaultNamespace.Application,
    ): VaultOutcome<PrivateStoragePolicy> =
        when (val root = privateRoot(context)) {
            is VaultOutcome.Failed -> root
            is VaultOutcome.Completed ->
                VaultOutcome.Completed(PrivateStoragePolicy(root.value, namespace))
        }

    /** The keys of [namespace], for a caller that removes them rather than uses them. */
    fun keys(namespace: VaultNamespace = VaultNamespace.Application): AndroidKeystoreVaultKeys =
        AndroidKeystoreVaultKeys(namespace)

    /** Acquiring noBackupFilesDir can create/stat it, so main-thread calls are refused. */
    fun privateRoot(context: Context): VaultOutcome<File> {
        if (MainThread.isCurrent()) {
            return VaultOutcome.Failed(VaultFailure.ForegroundRefused)
        }
        val root: File? =
            try {
                context.applicationContext.noBackupFilesDir
            } catch (error: RuntimeException) {
                return VaultOutcome.Failed(VaultFailure.ReadFailed(originOf(error)))
            }
        return if (root == null) {
            VaultOutcome.Failed(VaultFailure.ReadFailed(UNAVAILABLE_ROOT))
        } else {
            VaultOutcome.Completed(root)
        }
    }

    /** A private root the platform would not answer for. */
    private const val UNAVAILABLE_ROOT = "unavailable private root"

    private const val OPEN_THREAD = "slipbox-vault-open"
}
