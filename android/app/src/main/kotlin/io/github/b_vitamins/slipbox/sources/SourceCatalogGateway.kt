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

internal data class SourceCatalogListing(
    val revision: Long,
    val sources: List<RefreshSource>,
    val activeSource: RefreshSource?,
)

internal sealed interface SourceCatalogListingResult {

    data class Loaded(val listing: SourceCatalogListing) : SourceCatalogListingResult

    data class Failed(val failure: SourceCatalogResult.Failed) : SourceCatalogListingResult
}

internal sealed interface SourceCacheRemovalResult {

    data class Removed(val files: Long, val bytes: Long) : SourceCacheRemovalResult

    data class Failed(val failure: SourceCatalogResult.Failed) : SourceCacheRemovalResult
}

private sealed interface SourceCatalogCall {

    data class Answer(val response: SourceCatalogResponse) : SourceCatalogCall

    data class Failed(val result: SourceCatalogResult.Failed) : SourceCatalogCall
}

/** Blocking bridge to Rust-owned catalog and generation verification. */
internal fun interface SourceActivation {

    fun commit(expectedRevision: Long, source: RefreshSource, generation: String): SourceCatalogResult
}

internal interface SourceLibraryCatalog {

    fun list(): SourceCatalogListingResult

    fun load(listing: SourceCatalogListing): SourceCatalogResult

    fun activate(expectedRevision: Long, source: RefreshSource): SourceCatalogResult
}

internal class SourceCatalogGateway(
    context: Context,
    private val seam: NativeGitSeam = SlipboxNativeGit.seam,
) : SourceActivation, SourceLibraryCatalog {

    private val application = context.applicationContext

    fun load(): SourceCatalogResult {
        val listing =
            when (val listed = list()) {
                is SourceCatalogListingResult.Loaded -> listed.listing
                is SourceCatalogListingResult.Failed -> return listed.failure
            }
        return load(listing)
    }

    override fun load(listing: SourceCatalogListing): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val source = listing.activeSource ?: return SourceCatalogResult.Empty(listing.revision)
        return verify(root, listing.revision, source)
    }

    override fun list(): SourceCatalogListingResult {
        val root = privateRoot()
            ?: return SourceCatalogListingResult.Failed(
                SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE),
            )
        return list(root)
    }

    override fun activate(expectedRevision: Long, source: RefreshSource): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val paths =
            PackagedSourceRefreshStorage.paths(root, source)
                ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val response =
            when (val call = call(
                SourceCatalogRequest.Activate(
                    catalog = File(root, CATALOG_FILE).absolutePath,
                    expectedRevision = expectedRevision,
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
            else -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    override fun commit(
        expectedRevision: Long,
        source: RefreshSource,
        generation: String,
    ): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val paths =
            PackagedSourceRefreshStorage.paths(root, source)
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
            is SourceCatalogResponse.Removed, is SourceCatalogResponse.Purged ->
                SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    fun replace(
        expectedRevision: Long,
        previous: RefreshSource,
        source: RefreshSource,
        generation: String,
    ): SourceCatalogResult {
        val root = privateRoot() ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val paths =
            PackagedSourceRefreshStorage.paths(root, source)
                ?: return SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE)
        val response =
            when (val call = call(
                SourceCatalogRequest.Replace(
                    catalog = File(root, CATALOG_FILE).absolutePath,
                    expectedRevision = expectedRevision,
                    previous = previous,
                    source = source,
                    store = paths.store.absolutePath,
                    generation = generation,
                ),
            )) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return call.result
            }
        return when (response) {
            is SourceCatalogResponse.Ready -> admitted(response, source, generation)
            is SourceCatalogResponse.Refused -> SourceCatalogResult.Failed(failure = response.reason)
            else -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
        }
    }

    fun remove(expectedRevision: Long, source: RefreshSource): SourceCatalogListingResult {
        val root = privateRoot()
            ?: return SourceCatalogListingResult.Failed(
                SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE),
            )
        val response =
            when (val call = call(
                SourceCatalogRequest.Remove(
                    catalog = File(root, CATALOG_FILE).absolutePath,
                    expectedRevision = expectedRevision,
                    source = source,
                ),
            )) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed ->
                    return SourceCatalogListingResult.Failed(call.result)
            }
        return when (response) {
            is SourceCatalogResponse.Removed -> response.listingResult()
            is SourceCatalogResponse.Refused ->
                SourceCatalogListingResult.Failed(SourceCatalogResult.Failed(failure = response.reason))
            else ->
                SourceCatalogListingResult.Failed(
                    SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED),
                )
        }
    }

    fun purge(source: RefreshSource): SourceCacheRemovalResult {
        val root = privateRoot()
            ?: return SourceCacheRemovalResult.Failed(
                SourceCatalogResult.Failed(fault = SourceCatalogFault.STORAGE),
            )
        val response =
            when (val call = call(SourceCatalogRequest.Purge(root.absolutePath, source.id))) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return SourceCacheRemovalResult.Failed(call.result)
            }
        return when (response) {
            is SourceCatalogResponse.Purged ->
                SourceCacheRemovalResult.Removed(response.removedFiles, response.removedBytes)
            is SourceCatalogResponse.Refused ->
                SourceCacheRemovalResult.Failed(SourceCatalogResult.Failed(failure = response.reason))
            else ->
                SourceCacheRemovalResult.Failed(
                    SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED),
                )
        }
    }

    private fun verify(
        root: File,
        catalogRevision: Long,
        source: RefreshSource,
    ): SourceCatalogResult {
        val paths =
            PackagedSourceRefreshStorage.paths(root, source)
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
            else -> SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED)
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

    private fun list(root: File): SourceCatalogListingResult {
        val response =
            when (
                val call = call(SourceCatalogRequest.Load(File(root, CATALOG_FILE).absolutePath))
            ) {
                is SourceCatalogCall.Answer -> call.response
                is SourceCatalogCall.Failed -> return SourceCatalogListingResult.Failed(call.result)
            }
        return when (response) {
            is SourceCatalogResponse.Loaded -> response.listingResult()
            is SourceCatalogResponse.Refused ->
                SourceCatalogListingResult.Failed(SourceCatalogResult.Failed(failure = response.reason))
            else ->
                SourceCatalogListingResult.Failed(
                    SourceCatalogResult.Failed(fault = SourceCatalogFault.MALFORMED),
                )
        }
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

private fun SourceCatalogResponse.Loaded.listingResult(): SourceCatalogListingResult =
    listingResult(revision, sources, activeSource)

private fun SourceCatalogResponse.Removed.listingResult(): SourceCatalogListingResult =
    listingResult(revision, sources, activeSource)

private fun listingResult(
    revision: Long,
    sources: List<RefreshSource>,
    activeSource: RefreshSource?,
): SourceCatalogListingResult {
    val distinct = sources.map(RefreshSource::id).distinct().size == sources.size
    val activeIsListed = activeSource == null || sources.any { it == activeSource }
    return if (sources.size <= MAX_CATALOG_SOURCES && distinct && activeIsListed) {
        SourceCatalogListingResult.Loaded(SourceCatalogListing(revision, sources, activeSource))
    } else {
        SourceCatalogListingResult.Failed(
            SourceCatalogResult.Failed(fault = SourceCatalogFault.FOREIGN_SOURCE),
        )
    }
}

private const val MAX_CATALOG_SOURCES = 64
