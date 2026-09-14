/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.security.GeneralSecurityException
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec

/** AES-GCM with provider-generated nonces and authenticated scope/version metadata. */
class AesGcmVaultCipher(private val keys: VaultKeys) : VaultCipher {

    override fun seal(
        alias: String,
        aad: ByteArray,
        plaintext: ByteArray,
        key: KeySource,
    ): VaultOutcome<SealedRecord> {
        val obtained =
            when (key) {
                KeySource.Existing -> keys.existing(alias)
                KeySource.Provisioned -> keys.provision(alias)
            }
        val secret =
            when (obtained) {
                is VaultOutcome.Failed -> return obtained
                is VaultOutcome.Completed -> obtained.value
            }
        return try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, secret)
            cipher.updateAAD(aad)
            val ciphertext = cipher.doFinal(plaintext)
            VaultOutcome.Completed(SealedRecord(cipher.iv, ciphertext))
        } catch (error: GeneralSecurityException) {
            failure(CipherStage.Seal, error)
        }
    }

    override fun open(
        alias: String,
        aad: ByteArray,
        nonce: ByteArray,
        ciphertext: ByteArray,
    ): VaultOutcome<ByteArray> {
        val key =
            when (val existing = keys.existing(alias)) {
                is VaultOutcome.Failed -> return existing
                is VaultOutcome.Completed -> existing.value
            }
        return try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                key,
                GCMParameterSpec(VaultCipher.TAG_LENGTH_BITS, nonce),
            )
            cipher.updateAAD(aad)
            VaultOutcome.Completed(cipher.doFinal(ciphertext))
        } catch (untrusted: AEADBadTagException) {
            VaultOutcome.Failed(VaultFailure.AuthenticationFailed)
        } catch (error: GeneralSecurityException) {
            failure(CipherStage.Open, error)
        }
    }

    private fun failure(stage: CipherStage, error: GeneralSecurityException): VaultOutcome.Failed =
        VaultOutcome.Failed(
            keys.classify(error) ?: VaultFailure.CipherUnavailable(stage, originOf(error)),
        )

    private companion object {

        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
