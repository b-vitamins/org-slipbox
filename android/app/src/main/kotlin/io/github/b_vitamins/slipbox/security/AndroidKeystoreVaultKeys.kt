/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.Key
import java.security.KeyStore
import java.security.ProviderException
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey

/** The keystore, opened. Every method may throw what the provider raises. */
internal interface KeystoreHandle {

    fun key(alias: String): Key?

    fun contains(alias: String): Boolean

    fun delete(alias: String)

    fun aliases(): List<String>
}

/** Platform provider seam for key operations and invalidation classification. */
internal interface KeystoreProvider {

    fun open(): KeystoreHandle

    fun generate(alias: String, size: Int): SecretKey

    /** What a fault this provider raised means, or null when only its type is known. */
    fun meaning(error: Throwable): VaultFailure?
}

/** Blocking, namespace-scoped AndroidKeyStore operations; hardware backing is platform-dependent. */
class AndroidKeystoreVaultKeys internal constructor(
    private val namespace: VaultNamespace,
    private val provider: KeystoreProvider,
) : VaultKeys {

    constructor(namespace: VaultNamespace) : this(namespace, AndroidKeystoreProvider)

    override fun existing(alias: String): VaultOutcome<SecretKey> {
        refuseAlias(alias)?.let { return VaultOutcome.Failed(it) }
        val store =
            when (val opened = open()) {
                is VaultOutcome.Failed -> return opened
                is VaultOutcome.Completed -> opened.value
            }
        val entry =
            try {
                store.key(alias)
            } catch (error: GeneralSecurityException) {
                return unavailable(KeyStage.Read, error)
            } catch (error: ProviderException) {
                return unavailable(KeyStage.Read, error)
            }
        if (entry == null) {
            return VaultOutcome.Failed(VaultFailure.KeyMissing)
        }
        if (entry !is SecretKey) {
            return VaultOutcome.Failed(
                VaultFailure.KeyUnavailable(KeyStage.Read, entry.javaClass.name),
            )
        }
        return VaultOutcome.Completed(entry)
    }

    override fun provision(alias: String): VaultOutcome<SecretKey> {
        when (val existing = existing(alias)) {
            is VaultOutcome.Completed -> return existing
            is VaultOutcome.Failed ->
                if (existing.failure != VaultFailure.KeyMissing) {
                    return existing
                }
        }
        return try {
            VaultOutcome.Completed(provider.generate(alias, KEY_SIZE))
        } catch (error: GeneralSecurityException) {
            unavailable(KeyStage.Create, error)
        } catch (error: ProviderException) {
            unavailable(KeyStage.Create, error)
        }
    }

    override fun delete(alias: String): VaultOutcome<Boolean> {
        refuseAlias(alias)?.let { return VaultOutcome.Failed(it) }
        val store =
            when (val opened = open()) {
                is VaultOutcome.Failed -> return opened
                is VaultOutcome.Completed -> opened.value
            }
        return try {
            if (!store.contains(alias)) {
                VaultOutcome.Completed(false)
            } else {
                store.delete(alias)
                if (store.contains(alias)) {
                    VaultOutcome.Failed(
                        VaultFailure.RemovalFailed(RemovalStage.Key, UNDELETED),
                    )
                } else {
                    VaultOutcome.Completed(true)
                }
            }
        } catch (error: GeneralSecurityException) {
            removalFailed(error)
        } catch (error: ProviderException) {
            removalFailed(error)
        }
    }

    override fun deleteAll(prefix: String, keep: String?): VaultOutcome<Int> {
        refuseAlias(prefix)?.let { return VaultOutcome.Failed(it) }
        val store =
            when (val opened = open()) {
                is VaultOutcome.Failed -> return opened
                is VaultOutcome.Completed -> opened.value
            }
        val owned =
            try {
                store.aliases().filter { it.startsWith(prefix) && it != keep }
            } catch (error: GeneralSecurityException) {
                return unavailable(KeyStage.Read, error)
            } catch (error: ProviderException) {
                return unavailable(KeyStage.Read, error)
            }

        var removed = 0
        for (alias in owned) {
            when (val deleted = delete(alias)) {
                is VaultOutcome.Failed ->
                    return VaultOutcome.Failed(deleted.failure.progressed(removed))
                is VaultOutcome.Completed -> if (deleted.value) removed++
            }
        }
        return VaultOutcome.Completed(removed)
    }

    override fun classify(error: Throwable): VaultFailure? = provider.meaning(error)

    /** Removes only a test namespace; application-wide cleanup is refused. */
    fun releaseNamespace(): VaultOutcome<Int> {
        if (namespace.isApplication) {
            return VaultOutcome.Failed(
                VaultFailure.RefusedInput("key name space", InputDefect.Foreign),
            )
        }
        return deleteAll(namespace.aliasPrefix)
    }

    private fun open(): VaultOutcome<KeystoreHandle> =
        try {
            VaultOutcome.Completed(provider.open())
        } catch (error: GeneralSecurityException) {
            unavailable(KeyStage.Load, error)
        } catch (error: IOException) {
            unavailable(KeyStage.Load, error)
        } catch (error: ProviderException) {
            unavailable(KeyStage.Load, error)
        }

    private fun unavailable(stage: KeyStage, error: Throwable): VaultOutcome.Failed =
        VaultOutcome.Failed(
            provider.meaning(error) ?: VaultFailure.KeyUnavailable(stage, originOf(error)),
        )

    private fun removalFailed(error: Throwable): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.RemovalFailed(RemovalStage.Key, originOf(error)))

    private fun refuseAlias(alias: String): VaultFailure.RefusedInput? =
        refuseInput(ALIAS_FIELD, alias, VaultNamespace.MAX_ALIAS_LENGTH)
            ?: if (alias.startsWith(namespace.aliasPrefix)) {
                null
            } else {
                VaultFailure.RefusedInput(ALIAS_FIELD, InputDefect.Foreign)
            }

    private companion object {

        const val KEY_SIZE = 256

        const val ALIAS_FIELD = "key alias"

        /** A deletion the provider accepted and did not carry out. */
        const val UNDELETED = "undeleted entry"
    }
}

/** The platform keystore of this device. */
internal object AndroidKeystoreProvider : KeystoreProvider {

    private const val PROVIDER = "AndroidKeyStore"

    override fun open(): KeystoreHandle {
        val store = KeyStore.getInstance(PROVIDER)
        store.load(null)
        return PlatformKeystore(store)
    }

    override fun generate(alias: String, size: Int): SecretKey {
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(
            KeyGenParameterSpec
                .Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(size)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    override fun meaning(error: Throwable): VaultFailure? =
        if (error is KeyPermanentlyInvalidatedException) VaultFailure.KeyInvalidated else null

    private class PlatformKeystore(private val store: KeyStore) : KeystoreHandle {

        override fun key(alias: String): Key? = store.getKey(alias, null)

        override fun contains(alias: String): Boolean = store.containsAlias(alias)

        override fun delete(alias: String) {
            store.deleteEntry(alias)
        }

        override fun aliases(): List<String> = store.aliases().toList()
    }
}
