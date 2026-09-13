/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.version

object SlipboxVersion {

    const val RELEASE_STAGE: String = "release"

    private const val COMPONENT_LIMIT = 100
    private const val CANDIDATE_LIMIT = 1000

    private const val MAJOR_WINDOW = 10_000_000
    private const val MINOR_WINDOW = 100_000
    private const val PATCH_WINDOW = 1_000

    fun versionName(
        major: Int,
        minor: Int,
        patch: Int,
        candidate: Int,
        stage: String,
    ): String {
        requireComponents(major, minor, patch, candidate)
        requireStage(stage)
        val release = "$major.$minor.$patch"
        return if (stage == RELEASE_STAGE) release else "$release-$stage.$candidate"
    }

    /** Fixed decimal windows; stage is excluded and callers maintain monotonic candidates. */
    fun versionCode(
        major: Int,
        minor: Int,
        patch: Int,
        candidate: Int,
    ): Int {
        requireComponents(major, minor, patch, candidate)
        return major * MAJOR_WINDOW + minor * MINOR_WINDOW + patch * PATCH_WINDOW + candidate
    }

    fun isDevelopmentCandidate(stage: String): Boolean {
        requireStage(stage)
        return stage != RELEASE_STAGE
    }

    private fun requireComponents(major: Int, minor: Int, patch: Int, candidate: Int) {
        requireWindow("major", major, COMPONENT_LIMIT)
        requireWindow("minor", minor, COMPONENT_LIMIT)
        requireWindow("patch", patch, COMPONENT_LIMIT)
        requireWindow("candidate", candidate, CANDIDATE_LIMIT)
    }

    private fun requireWindow(name: String, value: Int, limit: Int) {
        require(value in 0 until limit) {
            "$name must be between 0 and ${limit - 1}, was $value"
        }
    }

    private fun requireStage(stage: String) {
        require(stage.matches(Regex("[a-z]+"))) {
            "stage must be lower-case letters, was $stage"
        }
    }
}
