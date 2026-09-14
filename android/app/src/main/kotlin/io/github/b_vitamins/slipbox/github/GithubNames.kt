/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

/** Branch and folder validation mirrors slipbox-core's source identity grammar. */
internal object GithubNames {

    const val MAX_NAME_CHARS = 100

    const val MAX_BRANCH_CHARS = 255

    const val MAX_FOLDER_CHARS = 512

    const val MAX_FOLDER_DEPTH = 32

    fun name(value: String): String? =
        value.takeIf {
            it.isNotEmpty() &&
                it.length <= MAX_NAME_CHARS &&
                it != "." &&
                it != ".." &&
                it.all { character -> isNameCharacter(character) }
        }

    fun branch(value: String): String? {
        if (value.isEmpty() || value.length > MAX_BRANCH_CHARS) {
            return null
        }
        if (value == "HEAD" || value == "@") {
            return null
        }
        if (value.contains("..") || value.contains("@{") || value.contains("//")) {
            return null
        }
        if (value.startsWith('-') || value.startsWith('/')) {
            return null
        }
        if (value.endsWith('/') || value.endsWith('.')) {
            return null
        }
        if (value.any { it.code <= 0x20 || it.code == 0x7f || it in FORBIDDEN_IN_REF }) {
            return null
        }
        val components = value.split('/')
        if (components.any { it.startsWith('.') || it.endsWith(".lock") }) {
            return null
        }
        return value
    }

    fun folder(value: String): String? {
        if (value.isEmpty() || value == ".") {
            return ""
        }
        if (value.length > MAX_FOLDER_CHARS) {
            return null
        }
        if (value.startsWith('/') || value.endsWith('/') || value.contains("//")) {
            return null
        }
        if (value.contains("..")) {
            return null
        }
        if (value.any { it.code < 0x20 || it.code == 0x7f || it == '\\' || it == ':' }) {
            return null
        }
        val segments = value.split('/')
        if (segments.size > MAX_FOLDER_DEPTH) {
            return null
        }
        if (segments.any { it == "." || it == ".git" }) {
            return null
        }
        return value
    }

    fun commit(value: String): String? =
        value.takeIf {
            it.length in MIN_SHA_CHARS..MAX_SHA_CHARS &&
                it.all { character -> character.isDigit() || character in 'a'..'f' }
        }

    private fun isNameCharacter(character: Char): Boolean =
        character in 'A'..'Z' ||
            character in 'a'..'z' ||
            character in '0'..'9' ||
            character == '-' ||
            character == '_' ||
            character == '.'

    private const val FORBIDDEN_IN_REF = "~^:?*[\\"

    private const val MIN_SHA_CHARS = 40

    private const val MAX_SHA_CHARS = 64
}
