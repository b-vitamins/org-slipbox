/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/**
 * Why a vault operation did not complete.
 *
 * A failure names what went wrong and what it left behind. None of them carries
 * a credential, a file's contents or a platform error message, so [toString] is
 * the one string form of a failure and it is safe to log: `origin` below is the
 * type of the platform error, never its text.
 */
sealed class VaultFailure {

    /** A one-line account of this failure. */
    abstract val summary: String

    /** A second failure raised while cleaning up after this one. */
    open val cleanup: VaultFailure? = null

    /**
     * Whether authorizing again and replacing the record recovers from this.
     *
     * A reauthorizing failure has deleted nothing: an unreadable record, its key
     * and the offline corpus are all still on disk until a caller replaces or
     * removes them.
     */
    open val reauthorize: Boolean = false

    final override fun toString(): String {
        val second = cleanup
        return if (second == null) summary else "$summary; while cleaning up, $second"
    }

    /** An input never reached storage: the [input] is [defect]. */
    data class RefusedInput(val input: String, val defect: InputDefect) : VaultFailure() {
        override val summary: String
            get() = "the $input is ${defect.description}"
    }

    /** A stored record cannot be interpreted: it is [defect]. */
    data class CorruptRecord(val defect: RecordDefect) : VaultFailure() {
        override val summary: String
            get() = "the stored record is ${defect.description}"
        override val reauthorize: Boolean = true
    }

    /**
     * The record did not authenticate under its key and scope.
     *
     * Its ciphertext, nonce, version or scope was altered, or it was written for
     * another scope; either way its plaintext is not recoverable.
     */
    object AuthenticationFailed : VaultFailure() {
        override val summary: String = "the stored record does not authenticate"
        override val reauthorize: Boolean = true
    }

    /** No key exists for this scope, so an existing record cannot be read. */
    object KeyMissing : VaultFailure() {
        override val summary: String = "this scope has no key"
        override val reauthorize: Boolean = true
    }

    /** The key exists but the platform has permanently invalidated it. */
    object KeyInvalidated : VaultFailure() {
        override val summary: String = "this scope's key is permanently invalidated"
        override val reauthorize: Boolean = true
    }

    /** The key provider refused or failed while [stage]; no key was returned. */
    data class KeyUnavailable(val stage: KeyStage, val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the key provider failed while ${stage.description} ($origin)"
    }

    /** Encryption itself was unavailable while [stage]. */
    data class CipherUnavailable(val stage: CipherStage, val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the cipher failed while ${stage.description} ($origin)"
    }

    /** The stored record could not be read; nothing was changed. */
    data class ReadFailed(val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the stored record could not be read ($origin)"
    }

    /**
     * A replacement did not commit, having reached [phase].
     *
     * Up to and including [CommitPhase.BeforeCommit] the previous record is the
     * one a read still returns.
     */
    data class WriteFailed(val phase: CommitPhase, val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the replacement failed ${phase.description} ($origin)"
    }

    /**
     * A removal did not finish, having failed at [stage] with [removed] entries
     * already gone.
     *
     * What a removal deleted before it failed stays deleted; repeating the removal
     * continues from there.
     */
    data class RemovalFailed(
        val stage: RemovalStage,
        val origin: String,
        val removed: Int = 0,
    ) : VaultFailure() {
        override val summary: String
            get() =
                if (removed == 0) {
                    "the removal failed at this scope's ${stage.description} ($origin)"
                } else {
                    "the removal failed at this scope's ${stage.description} " +
                        "after removing $removed ($origin)"
                }
    }

    /** Cleaning up after another failure failed as well. */
    data class CleanupFailed(val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the cleanup itself failed ($origin)"
    }

    /**
     * [primary], together with the failure of cleaning up after it.
     *
     * Both are kept: what went wrong decides what a caller may do next, and what
     * the cleanup left decides what is still on disk.
     */
    data class CleanupAfter(
        val primary: VaultFailure,
        override val cleanup: VaultFailure,
    ) : VaultFailure() {
        override val summary: String
            get() = primary.summary
        override val reauthorize: Boolean
            get() = primary.reauthorize
    }

    /** The vault is closed and accepts no further work. */
    object VaultClosed : VaultFailure() {
        override val summary: String = "this vault is closed"
    }

    /** Another vault of this process already holds this record. */
    object VaultHeld : VaultFailure() {
        override val summary: String = "another vault of this process holds this record"
    }

    /** A bound this vault keeps was already reached; nothing was accepted. */
    data class VaultAtCapacity(val bound: VaultBound) : VaultFailure() {
        override val summary: String
            get() = "this vault's ${bound.description} is full"
    }

    /** A blocking call was made on the thread that draws the screen. */
    object ForegroundRefused : VaultFailure() {
        override val summary: String = "keystore and file work must not block the main thread"
    }

    /** The vault's own work threw where it should have answered. */
    data class WorkFailed(val origin: String) : VaultFailure() {
        override val summary: String
            get() = "the vault's own work failed unexpectedly ($origin)"
    }
}

/** Why an input was refused before it reached a key, a file or a path. */
enum class InputDefect(internal val description: String) {
    Empty("empty"),
    TooLong("longer than this vault accepts"),
    NotPlainText("not printable ASCII text"),
    Foreign("outside this vault's namespace"),
    Absolute("an absolute path"),
    Traversing("a path leaving its own directory"),
    Escaping("a link out of its own directory"),
}

/** Why a stored record cannot be interpreted. */
enum class RecordDefect(internal val description: String) {
    Truncated("shorter than one record"),
    Oversized("larger than this vault stores"),
    UnsupportedVersion("of an unsupported version"),
    WrongNonceLength("carrying a nonce of the wrong length"),
    UnknownGeneration("sealed under a key generation this vault does not name"),
    ScopeMismatch("bound to another scope"),
    MalformedBody("malformed inside its authenticated body"),
    NotPlainTextBody("carrying a field that is not printable ASCII text"),
    TrailingBytes("longer than the body it declares"),
}

/** What the key provider was doing when it failed. */
enum class KeyStage(internal val description: String) {
    Load("opening the keystore"),
    Create("creating a key"),
    Read("reading a key"),
    Delete("deleting a key"),
}

/** What the cipher was doing when it failed. */
enum class CipherStage(internal val description: String) {
    Seal("sealing a record"),
    Open("opening a record"),
}

/** How far a replacement got before it failed. */
enum class CommitPhase(internal val description: String) {
    NotStarted("before it began"),
    BeforeCommit("before it committed"),

    /** The commit itself neither reported success nor reported refusal. */
    Unknown("with the commit unreported"),
    Committed("after it committed"),
    ;

    /** Whether the record a replacement was writing may be the stored one now. */
    internal val mayHaveCommitted: Boolean
        get() = this == Unknown || this == Committed
}

/** What a removal was deleting when it failed. */
enum class RemovalStage(internal val description: String) {
    Key("key"),
    Record("record"),
    Directory("directory"),
}

/** A bound a vault keeps, reached rather than exceeded. */
enum class VaultBound(internal val description: String) {
    /** How many records this process holds open at once. */
    OpenRecords("table of open records"),

    /** How many requests may wait for one vault's thread. */
    PendingRequests("queue of pending requests"),

    /** How many key generations one scope's aliases cycle through. */
    KeyGenerations("generations of this scope's keys"),
}

/** This failure, with [cleanup] as the failure of cleaning up after it. */
internal fun VaultFailure.after(cleanup: VaultFailure?): VaultFailure =
    if (cleanup == null) this else VaultFailure.CleanupAfter(this, cleanup)

/** This failure, counting the [removed] entries an earlier step had deleted. */
internal fun VaultFailure.progressed(removed: Int): VaultFailure =
    if (this is VaultFailure.RemovalFailed) copy(removed = this.removed + removed) else this

/**
 * The type of a platform error, for a diagnostic that cannot quote its text.
 *
 * A provider or file system message may repeat an input; a type name cannot.
 */
internal fun originOf(error: Throwable): String = error.javaClass.name

/**
 * The refusal of [value] as the input named [input], or null when it is
 * acceptable.
 *
 * Every text a caller supplies is bounded printable ASCII. Canonical source
 * identities, account identities and credential references are that already,
 * and an OAuth token is a printable ASCII string by its own specification, so
 * nothing longer or stranger reaches a key alias, a path or a record.
 */
internal fun refuseInput(input: String, value: String, limit: Int): VaultFailure.RefusedInput? =
    when {
        value.isEmpty() -> VaultFailure.RefusedInput(input, InputDefect.Empty)
        value.length > limit -> VaultFailure.RefusedInput(input, InputDefect.TooLong)
        value.any { it < '!' || it > '~' } ->
            VaultFailure.RefusedInput(input, InputDefect.NotPlainText)
        else -> null
    }
