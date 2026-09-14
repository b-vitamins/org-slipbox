/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.io.File

/**
 * The private name space one family of vaults keeps its keys and files in.
 *
 * [Application] is the shipped app's. A test run takes one of its own from
 * [forRun], so its keys and directories are named apart from the app's and it
 * can delete exactly what it created: nothing enumerates or removes entries of a
 * name space it does not own.
 */
class VaultNamespace private constructor(val label: String, private val rootName: String?) {

    /** Every key alias of this name space begins with this. */
    val aliasPrefix: String = "$ALIAS_ROOT$label."

    /** Whether this is the shipped app's name space rather than a test run's. */
    val isApplication: Boolean = rootName == null

    /**
     * Every key alias of [scope] here begins with this, whatever its generation.
     *
     * One prefix per scope is what lets a vault delete the keys it superseded
     * without naming them and without enumerating anything else.
     */
    fun aliasPrefixFor(scope: VaultScope): String = "$aliasPrefix${scope.digestHex}."

    /**
     * The key alias of [scope]'s [generation] here, at most [MAX_ALIAS_LENGTH]
     * characters of printable ASCII derived only from digests, this label and a
     * generation between [FIRST_GENERATION] and [LAST_GENERATION].
     */
    fun aliasFor(scope: VaultScope, generation: Int): String =
        aliasPrefixFor(scope) + generation

    /** This name space's root beneath the application-private [privateRoot]. */
    fun rootIn(privateRoot: File): File =
        if (rootName == null) privateRoot else File(privateRoot, rootName)

    override fun equals(other: Any?): Boolean = other is VaultNamespace && other.label == label

    override fun hashCode(): Int = label.hashCode()

    override fun toString(): String = "VaultNamespace($label)"

    companion object {

        /** The prefix every alias of every name space shares. */
        const val ALIAS_ROOT: String = "slipbox.vault.1."

        /** The bound an alias of any name space stays within. */
        const val MAX_ALIAS_LENGTH: Int = 128

        /** The longest run identifier [forRun] accepts. */
        const val MAX_RUN_LENGTH: Int = 24

        /** The generation of a scope's first key. */
        const val FIRST_GENERATION: Int = 1

        /** The last generation before one wraps round, being one stored byte. */
        const val LAST_GENERATION: Int = 255

        /** How many generations a scope's keys cycle through. */
        const val GENERATIONS: Int = LAST_GENERATION - FIRST_GENERATION + 1

        /**
         * The generation that supersedes [generation], or the first one when a
         * scope has none yet.
         *
         * Generations wrap, so an alias stays bounded; a wrapped one may name a key
         * another record still needs, which is why a recovery searches from here
         * rather than taking this one.
         */
        fun nextGeneration(generation: Int?): Int =
            if (generation == null || generation >= LAST_GENERATION) {
                FIRST_GENERATION
            } else {
                generation + 1
            }

        /** The name space the shipped application uses. */
        val Application: VaultNamespace = VaultNamespace("app", null)

        /**
         * A name space of the test run [runId], or its refusal.
         *
         * [runId] is lowercase letters, digits and dashes so that it can be both
         * a key alias fragment and a directory name unchanged.
         */
        fun forRun(runId: String): VaultOutcome<VaultNamespace> {
            val field = "run id"
            val refusal =
                refuseInput(field, runId, MAX_RUN_LENGTH)
                    ?: if (runId.any { it !in '0'..'9' && it !in 'a'..'z' && it != '-' }) {
                        VaultFailure.RefusedInput(field, InputDefect.NotPlainText)
                    } else {
                        null
                    }
            if (refusal != null) {
                return VaultOutcome.Failed(refusal)
            }
            return VaultOutcome.Completed(VaultNamespace("test-$runId", "test-$runId"))
        }
    }
}
