/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.File

const val PRIVATE_PATH_FIELD: String = "private path"

/** Vault entries are credential-scoped; other stores are account-scoped. */
enum class PrivateStore(internal val directoryName: String, internal val perCredential: Boolean) {
    Vault("vault", true),
    Checkout("checkout", false),
    Index("index", false),
    Assets("assets", false),
    Reading("reading", false),
}

/**
 * Digested private paths below an explicitly trusted root. Links below that root are refused.
 * Validation is a precheck, not race-free filesystem authority; cleanup never follows links.
 */
class PrivateStoragePolicy(
    private val privateRoot: File,
    namespace: VaultNamespace,
    private val io: RecordIo = SystemRecordIo,
) {

    val root: File = namespace.rootIn(privateRoot)

    /** Applies the trusted-root check to record and staging entries at use. */
    val entries: RecordEntries = RecordEntries { contained(it) }

    /** The directory of [store] for [scope], or why it is not a private path. */
    fun directoryFor(store: PrivateStore, scope: VaultScope): VaultOutcome<File> =
        contained(plainDirectoryFor(store, scope))

    /** The sealed credential record of [scope], or why it is not a private path. */
    fun recordFor(scope: VaultScope): VaultOutcome<File> =
        fileFor(PrivateStore.Vault, scope, RECORD_NAME)

    /**
     * The file [name] within [store] for [scope], or the refusal of that name or
     * its path. Creates nothing.
     */
    fun fileFor(store: PrivateStore, scope: VaultScope, name: String): VaultOutcome<File> {
        refuseName(name)?.let { return VaultOutcome.Failed(it) }
        return contained(File(plainDirectoryFor(store, scope), name))
    }

    /**
     * Deletes the per-credential stores of [scope] and reports how many entries
     * went. The account's own stores are left to [removeAccount].
     */
    fun removeCredential(scope: VaultScope): VaultOutcome<Int> =
        remove(scope) { it.perCredential }

    /** Deletes account stores; per-credential stores require [removeCredential]. */
    fun removeAccount(scope: VaultScope): VaultOutcome<Int> =
        remove(scope) { !it.perCredential }

    private fun remove(scope: VaultScope, wanted: (PrivateStore) -> Boolean): VaultOutcome<Int> {
        var removed = 0
        for (store in PrivateStore.values()) {
            if (!wanted(store)) {
                continue
            }
            val directory = plainDirectoryFor(store, scope)
            // The removal root may be an owned link; its ancestors may not.
            when (val ancestors = contained(requireNotNull(directory.parentFile))) {
                is VaultOutcome.Failed -> return progressed(ancestors.failure, removed)
                is VaultOutcome.Completed -> Unit
            }
            when (val deleted = removeEntry(directory, MAX_DEPTH)) {
                is VaultOutcome.Failed -> return progressed(deleted.failure, removed)
                is VaultOutcome.Completed -> removed += deleted.value
            }
        }
        return VaultOutcome.Completed(removed)
    }

    private fun removeEntry(entry: File, depth: Int): VaultOutcome<Int> {
        val parent = requireNotNull(entry.parentFile)
        when (val there = io.presenceOf(parent, entry.name)) {
            Presence.Absent -> return VaultOutcome.Completed(0)
            is Presence.Unknown -> return failed(there.origin, 0)
            Presence.Present -> Unit
        }
        val itself =
            when (val resolved = observing { io.canonicalPath(entry) }) {
                is Observed.Faulted -> return failed(resolved.origin, 0)
                is Observed.Answered -> resolved.value
            }
        val named =
            when (val resolved = observing { io.canonicalPath(parent) }) {
                is Observed.Faulted -> return failed(resolved.origin, 0)
                is Observed.Answered -> resolved.value + File.separator + entry.name
            }
        if (itself != named) {
            return unlink(entry, 0)
        }
        when (val kind = observing { io.isDirectory(entry) }) {
            is Observed.Faulted -> return failed(kind.origin, 0)
            is Observed.Answered ->
                if (!kind.value) {
                    return unlink(entry, 0)
                }
        }
        if (depth <= 0) {
            return failed(TOO_DEEP, 0)
        }

        var removed = 0
        val names =
            when (val listed = observing { io.entries(entry) }) {
                is Observed.Faulted -> return failed(listed.origin, 0)
                is Observed.Answered -> listed.value ?: return failed(UNLISTABLE_DIRECTORY, 0)
            }
        for (name in names) {
            when (val deleted = removeEntry(File(entry, name), depth - 1)) {
                is VaultOutcome.Failed -> return progressed(deleted.failure, removed)
                is VaultOutcome.Completed -> removed += deleted.value
            }
        }
        return unlink(entry, removed)
    }

    private fun unlink(entry: File, removed: Int): VaultOutcome<Int> =
        when (val gone = observing { io.remove(entry) }) {
            is Observed.Faulted -> failed(gone.origin, removed)
            is Observed.Answered ->
                if (gone.value) {
                    VaultOutcome.Completed(removed + 1)
                } else {
                    failed(UNDELETED_ENTRY, removed)
                }
        }

    private fun plainDirectoryFor(store: PrivateStore, scope: VaultScope): File {
        val owner = if (store.perCredential) scope.digestHex else scope.accountDigestHex
        return File(File(root, store.directoryName), owner)
    }

    private fun contained(candidate: File): VaultOutcome<File> {
        val steps = mutableListOf<File>()
        var walk: File? = candidate
        while (walk != null && walk != privateRoot) {
            steps.add(walk)
            walk = walk.parentFile
        }
        if (walk == null) {
            return VaultOutcome.Failed(
                VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Foreign),
            )
        }
        return try {
            var resolved = io.canonicalPath(privateRoot)
            for (step in steps.asReversed()) {
                val expected = resolved + File.separator + step.name
                if (io.canonicalPath(step) != expected) {
                    return VaultOutcome.Failed(
                        VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping),
                    )
                }
                io.refuseLinkToNowhere(step)?.let { return VaultOutcome.Failed(it) }
                resolved = expected
            }
            VaultOutcome.Completed(candidate)
        } catch (error: Exception) {
            VaultOutcome.Failed(VaultFailure.ReadFailed(originOf(error)))
        }
    }

    private fun refuseName(name: String): VaultFailure.RefusedInput? {
        refuseInput(FILE_FIELD, name, MAX_NAME_LENGTH)?.let { return it }
        if (name.startsWith('/')) {
            return VaultFailure.RefusedInput(FILE_FIELD, InputDefect.Absolute)
        }
        if (name.contains('/') || name.contains('\\') || name.contains("..") || name == ".") {
            return VaultFailure.RefusedInput(FILE_FIELD, InputDefect.Traversing)
        }
        val leads = name[0] in '0'..'9' || name[0] in 'a'..'z'
        if (!leads || name.any { it !in '0'..'9' && it !in 'a'..'z' && it !in TRAILING }) {
            return VaultFailure.RefusedInput(FILE_FIELD, InputDefect.NotPlainText)
        }
        return null
    }

    private fun failed(origin: String, removed: Int): VaultOutcome.Failed =
        VaultOutcome.Failed(
            VaultFailure.RemovalFailed(RemovalStage.Directory, origin, removed),
        )

    private fun progressed(failure: VaultFailure, removed: Int): VaultOutcome.Failed =
        VaultOutcome.Failed(failure.progressed(removed))

    companion object {

        /** The longest private file name this policy accepts. */
        const val MAX_NAME_LENGTH: Int = 64

        /** The name every vault record file has within its own directory. */
        const val RECORD_NAME: String = "credential.bin"

        private const val FILE_FIELD = "file name"

        private const val TRAILING = "._-"

        /** Deeper than any store here nests; a deeper tree is refused, not walked. */
        private const val MAX_DEPTH = 64

        private const val UNDELETED_ENTRY = "undeleted entry"

        private const val TOO_DEEP = "tree deeper than this policy removes"
    }
}
