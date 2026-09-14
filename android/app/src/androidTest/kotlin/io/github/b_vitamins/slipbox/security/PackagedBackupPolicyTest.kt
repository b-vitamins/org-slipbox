/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.xmlpull.v1.XmlPullParser

/**
 * The backup policy of the installed package, read back from the installation
 * rather than from the sources it was built from.
 *
 * This is what the platform parsed at install time and what it would consult; it
 * is not an observation of a backup transport, a cloud account or a device
 * transfer, none of which a test can drive.
 */
@RunWith(AndroidJUnit4::class)
class PackagedBackupPolicyTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun theInstalledPackageRefusesBackupAndNamesNoAgentToPerformOne() {
        val info = context.applicationInfo
        assertEquals(context.packageName, info.packageName)

        assertEquals(0L, (info.flags and ApplicationInfo.FLAG_ALLOW_BACKUP).toLong())
        assertEquals(0L, (info.flags and ApplicationInfo.FLAG_FULL_BACKUP_ONLY).toLong())
        assertEquals(0L, (info.flags and ApplicationInfo.FLAG_RESTORE_ANY_VERSION).toLong())
        assertNull("the installation names a backup agent", info.backupAgentName)
        Log.i(TAG, "installed package ${info.packageName} refuses backup and names no agent")
    }

    @Test
    fun thePackagedRulesExcludeEverySensitiveDomainInEveryMode() {
        for (resource in listOf(R.xml.backup_rules, R.xml.data_extraction_rules)) {
            assertEquals("xml", context.resources.getResourceTypeName(resource))
            assertEquals(context.packageName, context.resources.getResourcePackageName(resource))
        }
        assertEquals("backup_rules", context.resources.getResourceEntryName(R.xml.backup_rules))
        assertEquals(
            "data_extraction_rules",
            context.resources.getResourceEntryName(R.xml.data_extraction_rules),
        )

        val legacy = rulesOf(R.xml.backup_rules, LEGACY_MODE)
        assertEquals(DOMAINS.map { "$LEGACY_MODE|exclude|$it|." }, legacy)

        val modern = rulesOf(R.xml.data_extraction_rules, SECTIONLESS)
        assertEquals(MODERN_MODES.flatMap { mode -> DOMAINS.map { "$mode|exclude|$it|." } }, modern)
        Log.i(
            TAG,
            "packaged rules: ${legacy.size} legacy and ${modern.size} modern exclusions over" +
                " ${DOMAINS.size} domains, no inclusion and none outside a mode",
        )
    }

    @Test
    fun theInstalledPackageHoldsNoStorageOrBackupPermission() {
        for (permission in REFUSED_PERMISSIONS) {
            assertEquals(
                permission,
                PackageManager.PERMISSION_DENIED.toLong(),
                context.checkSelfPermission(permission).toLong(),
            )
        }
        Log.i(TAG, "${REFUSED_PERMISSIONS.size} storage or backup permission(s) all denied")
    }

    /**
     * The rules the packaged resource holds, as "mode|kind|domain|path" lines.
     *
     * A rule outside any section keeps [mode], so the modern resource reports one
     * as sectionless rather than silently attributing it to a mode. Rule
     * attributes carry no name space, which is why each is read with a null one.
     */
    private fun rulesOf(resource: Int, mode: String): List<String> {
        val listing = mutableListOf<String>()
        var section = mode
        val parser = context.resources.getXml(resource)
        try {
            while (parser.next() != XmlPullParser.END_DOCUMENT) {
                if (parser.eventType != XmlPullParser.START_TAG) {
                    continue
                }
                val name = parser.name
                when {
                    name in ROOTS -> assertTrue(name, section == mode)
                    name in MODERN_MODES -> section = name
                    else ->
                        listing.add(
                            "$section|$name|${parser.getAttributeValue(null, "domain")}" +
                                "|${parser.getAttributeValue(null, "path")}",
                        )
                }
            }
        } finally {
            parser.close()
        }
        return listing
    }

    private companion object {

        const val TAG = "SlipboxVaultProbe"

        val ROOTS = listOf("full-backup-content", "data-extraction-rules")

        const val LEGACY_MODE = "legacy"

        val MODERN_MODES = listOf("cloud-backup", "device-transfer")

        /** What a rule of the modern resource carries until a mode section opens. */
        const val SECTIONLESS = ""

        /** Every domain a credential, an index, a checkout or a note can reach. */
        val DOMAINS =
            listOf(
                "root",
                "file",
                "database",
                "sharedpref",
                "external",
                "device_root",
                "device_file",
                "device_database",
                "device_sharedpref",
            )

        /**
         * Named as strings: this suite must ask about a permission the compiled
         * platform constant of which arrived after the API this app supports.
         */
        val REFUSED_PERMISSIONS =
            listOf(
                "android.permission.READ_EXTERNAL_STORAGE",
                "android.permission.WRITE_EXTERNAL_STORAGE",
                "android.permission.MANAGE_EXTERNAL_STORAGE",
                "android.permission.ACCESS_MEDIA_LOCATION",
                "android.permission.READ_MEDIA_IMAGES",
                "android.permission.READ_MEDIA_VIDEO",
                "android.permission.READ_MEDIA_AUDIO",
                "android.permission.READ_MEDIA_VISUAL_USER_SELECTED",
                "android.permission.BACKUP",
            )
    }
}
