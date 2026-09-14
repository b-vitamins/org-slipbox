/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import javax.crypto.SecretKey

/**
 * Where a vault's data keys come from.
 *
 * Reading and writing are deliberately asymmetric: [provision] may create a key
 * because a caller that writes a record is authorizing one, while [existing]
 * never does, so a missing or invalidated key over stored ciphertext is reported
 * as such instead of being replaced by a new key that could not read it.
 */
interface VaultKeys {

    /**
     * The key named [alias], or [VaultFailure.KeyMissing] when there is none.
     * Creates nothing.
     */
    fun existing(alias: String): VaultOutcome<SecretKey>

    /** The key named [alias], created with this provider's parameters if absent. */
    fun provision(alias: String): VaultOutcome<SecretKey>

    /** Deletes the key named [alias] and reports whether one was there. */
    fun delete(alias: String): VaultOutcome<Boolean>

    /**
     * Deletes every key whose alias begins with [prefix] except [keep], and
     * reports how many it removed.
     *
     * [prefix] belongs to one scope of this provider's name space, which is what
     * keeps a superseded key of that scope from outliving the record it sealed
     * while leaving every other scope and name space alone.
     */
    fun deleteAll(prefix: String, keep: String? = null): VaultOutcome<Int>

    /**
     * What a platform error raised while using one of these keys means, or null
     * when this provider knows nothing more precise than its type.
     *
     * Only the provider can tell an invalidated key from an unusable one, so a
     * cipher asks rather than matching on exception types it does not own.
     */
    fun classify(error: Throwable): VaultFailure? = null
}
