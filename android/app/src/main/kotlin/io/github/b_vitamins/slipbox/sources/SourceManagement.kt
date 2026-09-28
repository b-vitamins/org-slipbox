/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import android.content.Context
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultScope
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshScheduleOutcome
import io.github.b_vitamins.slipbox.sync.SourceRefreshScheduler
import java.util.concurrent.atomic.AtomicBoolean

internal sealed interface SourceManagementResult {

    data object Disconnected : SourceManagementResult

    data class CacheRemoved(val files: Long, val bytes: Long) : SourceManagementResult

    data class SourceRemoved(
        val catalog: SourceCatalogListing,
        val cacheRemoved: Boolean,
        val credentialRemoved: Boolean,
    ) : SourceManagementResult

    data class Failed(val failure: SourceCatalogResult.Failed? = null) : SourceManagementResult
}

/** Coordinates work withdrawal with intentionally distinct credential, cache, and catalog removal. */
internal class SourceManagement(
    context: Context,
    private val gateway: SourceCatalogGateway = SourceCatalogGateway(context.applicationContext),
    private val scheduler: SourceRefreshScheduler =
        SourceRefreshScheduler.packaged(context.applicationContext),
    private val delivery: ImportDelivery = ImportDelivery.MainThread,
) : AutoCloseable {

    private val application = context.applicationContext
    private val live = AtomicBoolean(true)
    private val operating = AtomicBoolean()

    fun disconnect(source: RefreshSource, recipient: (SourceManagementResult) -> Unit) {
        run(recipient) {
            if (scheduler.removeAndAwait(source.id) !is RefreshScheduleOutcome.Accepted) {
                return@run SourceManagementResult.Failed()
            }
            if (SourceCredentials.remove(application, source)) {
                SourceManagementResult.Disconnected
            } else {
                SourceManagementResult.Failed()
            }
        }
    }

    fun removeCache(source: RefreshSource, recipient: (SourceManagementResult) -> Unit) {
        run(recipient) {
            if (scheduler.removeAndAwait(source.id) !is RefreshScheduleOutcome.Accepted) {
                return@run SourceManagementResult.Failed()
            }
            when (val removed = gateway.purge(source)) {
                is SourceCacheRemovalResult.Removed ->
                    SourceManagementResult.CacheRemoved(removed.files, removed.bytes)
                is SourceCacheRemovalResult.Failed -> SourceManagementResult.Failed(removed.failure)
            }
        }
    }

    fun removeSource(
        expectedRevision: Long,
        source: RefreshSource,
        recipient: (SourceManagementResult) -> Unit,
    ) {
        run(recipient) {
            if (scheduler.removeAndAwait(source.id) !is RefreshScheduleOutcome.Accepted) {
                return@run SourceManagementResult.Failed()
            }
            when (val removed = gateway.remove(expectedRevision, source)) {
                is SourceCatalogListingResult.Failed -> SourceManagementResult.Failed(removed.failure)
                is SourceCatalogListingResult.Loaded -> {
                    val cacheRemoved = gateway.purge(source) is SourceCacheRemovalResult.Removed
                    val credentialRemoved = SourceCredentials.remove(application, source)
                    SourceManagementResult.SourceRemoved(
                        catalog = removed.listing,
                        cacheRemoved = cacheRemoved,
                        credentialRemoved = credentialRemoved,
                    )
                }
            }
        }
    }

    override fun close() {
        live.set(false)
    }

    private fun run(
        recipient: (SourceManagementResult) -> Unit,
        operation: () -> SourceManagementResult,
    ) {
        if (!live.get() || !operating.compareAndSet(false, true)) return
        Thread(
                {
                    val result =
                        try {
                            operation()
                        } catch (_: Exception) {
                            SourceManagementResult.Failed()
                        }
                    operating.set(false)
                    delivery.post { if (live.get()) recipient(result) }
                },
                "slipbox-source-management",
            )
            .apply {
                isDaemon = true
                start()
            }
    }
}

/** Credential presence is the durable connection marker for private-source scheduling. */
internal object SourceCredentials {

    fun connected(context: Context, source: RefreshSource): Boolean {
        if (source.credential == null) return true
        val scope = scope(source) ?: return false
        val vault =
            when (val opened = SlipboxVault.open(context.applicationContext, scope)) {
                is VaultOutcome.Completed -> opened.value
                is VaultOutcome.Failed -> return false
            }
        return vault.use { opened ->
            when (val credential = opened.read()) {
                is VaultOutcome.Completed -> credential.value != null
                is VaultOutcome.Failed -> false
            }
        }
    }

    fun remove(context: Context, source: RefreshSource): Boolean {
        if (source.credential == null) return true
        val scope = scope(source) ?: return false
        val vault =
            when (val opened = SlipboxVault.open(context.applicationContext, scope)) {
                is VaultOutcome.Completed -> opened.value
                is VaultOutcome.Failed -> return false
            }
        return vault.use { it.remove() is VaultOutcome.Completed }
    }

    private fun scope(source: RefreshSource): VaultScope? {
        val account = source.account ?: return null
        val credential = source.credential ?: return null
        return when (
            val composed =
                VaultScope.of(
                    sourceId = source.id,
                    providerAuthority = GithubApp.PROVIDER_AUTHORITY,
                    accountId = account,
                    credentialRef = credential,
                )
        ) {
            is VaultOutcome.Completed -> composed.value
            is VaultOutcome.Failed -> null
        }
    }
}
