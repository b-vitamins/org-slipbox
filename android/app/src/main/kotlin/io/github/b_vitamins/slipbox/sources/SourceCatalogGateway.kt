/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.sources

import android.content.Context
import io.github.b_vitamins.slipbox.git.NativeGitSeam
import io.github.b_vitamins.slipbox.git.SlipboxNativeGit
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.sync.PackagedSourceRefreshStorage
import io.github.b_vitamins.slipbox.sync.RefreshSource
import java.io.File

internal sealed interface SourceCatalogResult {

    data class Empty(val revision: Long) : SourceCatalogResult

    data class Active(val revision: Long, val ready: ReadySource) : SourceCatalogResult

    data class Failed(
        val failure: SourceCatalogFailure? = null,
        val fault: SourceCatalogFault? = null,
    ) : SourceCatalogResult
}

private sealed interface SourceCatalogCall {

    data class Answer(val response: SourceCatalogResponse) : SourceCatalogCall

    data class Failed(val result: SourceCatalogResult.Failed) : SourceCatalogCall
}

/** Blocking bridge to Rust-owned catalog and generation verification. */
internal fun interface SourceActivation {

    fun commit(expectedRevision: Long, source: RefreshSource, generation: String): SourceCatalogResult
}

internal class SourceCatalogGateway(
    context: Context,
    private val seam: NativeGitSeam = SlipboxNativeGit.seam,
) : SourceActivation {

    private val application = context.applicationContext

    fun load(): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val loaded =
            when (val call = call(SourceCatalogRequest.Load(File(root, CATALOG_FILE).absolutePath))) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return call.result
            }
        return when (loaded) {
            is SourceCatalogResponse.Loaded -> {
                val source = loaded.activeSource ?: return SourceCatalogResult.Empty(loaded.revision)
                verify(root, loaded.revision, source)
            }
            is SourceCatalogResponse.Refused -> SourceCatalogResult.Failed(failure = loaded.reason)
            is SourceCatalogResponse.Ready -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    override fun commit(
        expectedRevision: Long,
        source: RefreshSource,
        generation: String,
    ): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val paths =
            PackagedSourceRefreshStorage.paths(root, source.id)
                ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val request =
            SourceCatalogRequest.Commit(
                catalog = File(root, CATALOG_FILE).absolutePath,
                expectedRevision = expectedRevision,
                source = source,
                store = paths.store.absolutePath,
                generation = generation,
            )
        val response =
            when (val call = call(request)) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return call.result
            }
        return when (response) {
            is SourceCatalogResponse.Ready -> admitted(response, source, generation)
            is SourceCatalogResponse.Refused -> {
                if (response.reason == SourceCatalogFailure.CATALOG_CONFLICT) {
                    val recovered = load()
                    if (recovered is SourceCatalogResult.Active &&
                        recovered.ready.source == source &&
                        recovered.ready.binding.generation == generation
                    ) {
                        recovered
                    } else {
                        SourceCatalogResult.Failed(failure = response.reason)
                    }
                } else {
                    SourceCatalogResult.Failed(failure = response.reason)
                }
            }
            is SourceCatalogResponse.Loaded -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    private fun verify(
        root: File,
        catalogRevision: Long,
        source: RefreshSource,
    ): SourceCatalogResult {
        val paths =
            PackagedSourceRefreshStorage.paths(root, source.id)
                ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val response =
            when (val call = call(
                SourceCatalogRequest.Verify(
                    catalog = File(root, CATALOG_FILE).absolutePath,
                    expectedRevision = catalogRevision,
                    source = source,
                    store = paths.store.absolutePath,
                ),
            )) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return call.result
            }
        return when (response) {
            is SourceCatalogResponse.Ready -> admitted(response, source, null)
            is SourceCatalogResponse.Refused -> SourceCatalogResult.Failed(failure = response.reason)
            is SourceCatalogResponse.Loaded -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    private fun admitted(
        response: SourceCatalogResponse.Ready,
        source: RefreshSource,
        generation: String?,
    ): SourceCatalogResult =
        when {
            response.ready.source != source || response.ready.binding.source != source.id ->
                SourceCatalogResult.Failed(fault = SourceCatalogFault.FOREIGN_SOURCE)
            generation != null && response.ready.binding.generation != generation ->
                SourceCatalogResult.Failed(fault = SourceCatalogFault.FOREIGN_GENERATION)
            else -> SourceCatalogResult.Active(response.revision, response.ready)
        }

    private fun call(request: SourceCatalogRequest): SourceCatalogCall {
        val encoded = SourceCatalogWire.encode(request)
        if (encoded.size > MAX_SOURCE_CATALOG_REQUEST_BYTES) {
            return failed(SourceCatalogFault.REQUEST_OVERSIZED)
        }
        val answer = seam.sourceCatalog(encoded)
        if (answer == null) {
            return failed(SourceCatalogFault.NO_ANSWER)
        }
        if (answer.size > MAX_SOURCE_CATALOG_RESPONSE_BYTES) {
            return failed(SourceCatalogFault.RESPONSE_OVERSIZED)
        }
        val response =
            try {
                SourceCatalogWire.decode(answer)
            } catch (failure: SourceCatalogContractException) {
                return failed(failure.fault)
            }
        if (response.version != SOURCE_CATALOG_PROTOCOL_VERSION) {
            return failed(SourceCatalogFault.UNSUPPORTED_VERSION)
        }
        return SourceCatalogCall.Answer(response)
    }

    private fun failed(fault: SourceCatalogFault): SourceCatalogCall.Failed =
        SourceCatalogCall.Failed(SourceCatalogResult.Failed(fault = fault))

    private fun privateRoot(): File? =
        when (val root = SlipboxVault.privateRoot(application)) {
            is VaultOutcome.Completed -> root.value
            is VaultOutcome.Failed -> null
        }

    private companion object {

        const val CATALOG_FILE = "source-catalog.json"
    }
}
