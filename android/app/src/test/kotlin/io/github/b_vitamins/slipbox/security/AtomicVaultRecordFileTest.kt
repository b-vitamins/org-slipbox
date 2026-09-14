/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException

/**
 * The record file over real files, with single faults planted in the file system.
 *
 * Each case reaches the replacement protocol itself: what it observes of a write,
 * a sync, a close, a rename and an abandonment decides the outcome.
 */
class AtomicVaultRecordFileTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val io = FaultyRecordIo()

    private lateinit var directory: File

    private lateinit var target: File

    @Test
    fun aRecordThatWasNeverWrittenReadsAsAbsent() {
        val file = recordFile()

        assertNull(file.read(LIMIT).completed())
        assertFalse(directory.exists())
    }

    @Test
    fun anAbsentRecordInAnExistingDirectoryReadsAsAbsent() {
        val file = recordFile()
        assertTrue(directory.mkdirs())

        assertNull(file.read(LIMIT).completed())
    }

    @Test
    fun aRecordThatIsThereAndWillNotOpenIsNotAbsence() {
        val file = recordFile()
        write(FIRST)
        io.readFault = { FileNotFoundException("$RECORD (Permission denied)") }

        val failure = file.read(LIMIT).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.ReadFailed)
    }

    @Test
    fun aDirectoryWhereTheRecordBelongsIsRefusedRatherThanReadAsAbsent() {
        val file = recordFile()
        assertTrue(target.mkdirs())

        val failure = file.read(LIMIT).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.ReadFailed)
        assertTrue(target.isDirectory)
    }

    @Test
    fun anUnlistableDirectoryIsUnknownRatherThanAbsence() {
        val file = recordFile()
        assertTrue(directory.mkdirs())
        io.unlistable.add(directory.path)

        val failure = file.read(LIMIT).refusal()

        assertTrue(failure.toString(), failure is VaultFailure.ReadFailed)
    }

    @Test
    fun aRecordUnderADirectoryThisProcessCannotEnterIsNotAbsence() {
        val file = recordFile()
        write(FIRST)

        val outcome = withoutAccess(requireNotNull(directory.parentFile)) { file.read(LIMIT) }

        val failure = outcome.refusal()
        assertTrue(failure.toString(), failure is VaultFailure.ReadFailed)
    }

    @Test
    fun aReplacedRecordIsTheOneThatComesBack() {
        val file = recordFile()

        file.replace(SECOND).completed()

        assertArrayEquals(SECOND, file.read(LIMIT).completed())
        assertFalse(staging().exists())
    }

    @Test
    fun aRefusedOpenLeavesThePreviousRecordAndStartsNothing() {
        val file = recordFile()
        write(FIRST)
        io.openFault = { FileNotFoundException("$STAGING (Permission denied)") }

        val failure = file.replace(SECOND).refusal()

        assertEquals(CommitPhase.NotStarted, phaseOf(failure))
        assertArrayEquals(FIRST, target.readBytes())
    }

    @Test
    fun aStagingRecordThatIsALinkIsRefusedRatherThanWrittenThrough() {
        val file = recordFile()
        write(FIRST)
        val elsewhere = File(temporary.newFolder("outside"), "sentinel.bin")
        elsewhere.writeBytes(SENTINEL)
        link(staging(), elsewhere)

        val failure = file.replace(SECOND).refusal()

        assertEquals(ESCAPING, failure)
        assertArrayEquals("the previous record was replaced", FIRST, target.readBytes())
        assertArrayEquals("bytes no record owns were written", SENTINEL, elsewhere.readBytes())
        assertFalse("the record became the link", target.canonicalPath == elsewhere.path)
    }

    @Test
    fun aStagingRecordThatIsALinkToNowhereIsRefusedRatherThanCreatingWhereItPoints() {
        val file = recordFile()
        write(FIRST)
        val never = File(temporary.newFolder("outside"), "never-there")
        link(staging(), never)

        val failure = file.replace(SECOND).refusal()

        assertEquals(ESCAPING, failure)
        assertFalse("a record was created outside the directory", never.exists())
        assertArrayEquals("the previous record was replaced", FIRST, target.readBytes())
    }

    @Test
    fun aRefusedRenameKeepsThePreviousRecordAndReportsTheFailedCommit() {
        val file = recordFile()
        write(FIRST)
        io.renameRefused = true

        val failure = file.replace(SECOND).refusal()

        assertEquals(CommitPhase.BeforeCommit, phaseOf(failure))
        assertNull("a successful abandonment was reported as a cleanup", failure.cleanup)
        assertArrayEquals(FIRST, target.readBytes())
        assertFalse("the staging record was left behind", staging().exists())
    }

    @Test
    fun aRenameThatKeepsItsSourceIsReportedAfterTheCommit() {
        val file = recordFile()
        write(FIRST)
        io.renameKeepsSource = true

        val failure = file.replace(SECOND).refusal()

        assertEquals(CommitPhase.Committed, phaseOf(failure))
        assertNull(failure.cleanup)
        assertArrayEquals(SECOND, target.readBytes())
        assertFalse("the staging record was left behind", staging().exists())
    }

    @Test
    fun aRenameThatFaultsPromisesNothingAboutTheRecordItWasCommitting() {
        val file = recordFile()
        write(FIRST)
        io.renameFault = { SecurityException(QUOTED_INPUT) }

        val failure = answered { file.replace(SECOND) }.refusal()

        assertEquals(originOf(SecurityException()), originIn(failure))
        assertEquals(
            "a fault of unknown effect promised the previous record",
            CommitPhase.Unknown,
            phaseOf(failure),
        )
        assertFalse(failure.toString(), failure.toString().contains(QUOTED_INPUT))
        assertFalse("the staging record was left behind", staging().exists())
    }

    @Test
    fun aFaultWhileCheckingACommitIsStillReportedAsACommit() {
        val file = recordFile()
        write(FIRST)
        io.afterRename = { io.entriesFault = { SecurityException(QUOTED_INPUT) } }

        val failure = answered { file.replace(SECOND) }.refusal()

        assertEquals(CommitPhase.Committed, phaseOf(failure))
        assertEquals(originOf(SecurityException()), originIn(failure))
        assertFalse(failure.toString(), failure.toString().contains(QUOTED_INPUT))
        assertArrayEquals("the committed record is not the one written", SECOND, target.readBytes())
    }

    @Test
    fun aFaultThatIsNotAnIoFaultStillAbandonsTheStagingRecord() {
        val file = recordFile()
        write(FIRST)
        io.writeFault = { IllegalStateException("synthetic writer fault") }

        val failure = file.replace(SECOND).refusal()

        assertEquals(CommitPhase.BeforeCommit, phaseOf(failure))
        assertNull(failure.cleanup)
        assertArrayEquals(FIRST, target.readBytes())
        assertFalse("the staging record was left behind", staging().exists())
    }

    @Test
    fun aFailedAbandonmentIsReportedAlongsideTheFailureItFollowed() {
        val file = recordFile()
        write(FIRST)
        io.syncFault = { IOException("synthetic sync fault") }
        io.kept.add(staging().path)

        val failure = file.replace(SECOND).refusal()

        val carried = failure as VaultFailure.CleanupAfter
        assertEquals(
            VaultFailure.WriteFailed(CommitPhase.BeforeCommit, originOf(IOException())),
            carried.primary,
        )
        assertTrue(carried.toString(), carried.cleanup is VaultFailure.CleanupFailed)
        assertEquals(CommitPhase.BeforeCommit, phaseOf(carried))
        assertArrayEquals(FIRST, target.readBytes())
        assertTrue("the staging record it could not take is gone", staging().exists())
    }

    @Test
    fun aCloseThatFailsAgainWhileAbandoningIsReportedAsTheCleanupItIs() {
        val file = recordFile()
        write(FIRST)
        io.closeFault = { IOException("synthetic close fault") }

        val failure = file.replace(SECOND).refusal()

        val carried = failure as VaultFailure.CleanupAfter
        assertEquals(
            VaultFailure.WriteFailed(CommitPhase.BeforeCommit, originOf(IOException())),
            carried.primary,
        )
        assertEquals(VaultFailure.CleanupFailed(originOf(IOException())), carried.cleanup)
        assertArrayEquals(FIRST, target.readBytes())
        assertFalse("the staging record was left behind", staging().exists())
    }

    @Test
    fun anAbandonmentThatFaultsIsReportedAsTheCleanupItIsRatherThanThrown() {
        val file = recordFile()
        write(FIRST)
        io.syncFault = { IOException("synthetic sync fault") }
        io.removeFault = { SecurityException(QUOTED_INPUT) }

        val failure = answered { file.replace(SECOND) }.refusal()

        val carried = failure as VaultFailure.CleanupAfter
        assertEquals(
            VaultFailure.WriteFailed(CommitPhase.BeforeCommit, originOf(IOException())),
            carried.primary,
        )
        assertEquals(VaultFailure.CleanupFailed(originOf(SecurityException())), carried.cleanup)
        assertFalse(carried.toString(), carried.toString().contains(QUOTED_INPUT))
        assertArrayEquals(FIRST, target.readBytes())
    }

    @Test
    fun deletionTakesTheEntriesTheProtocolOwnsAndReportsWhatWasThere() {
        val file = recordFile()
        write(FIRST)
        staging().writeBytes(SECOND)
        File(target.path + LEGACY_SUFFIX).writeBytes(FIRST)

        assertTrue(file.delete().completed())

        assertFalse(target.exists())
        assertFalse("the staging record was left behind", staging().exists())
        assertFalse(File(target.path + LEGACY_SUFFIX).exists())
        assertFalse(file.delete().completed())
    }

    @Test
    fun aDeletionTheFileSystemDidNotCarryOutIsReported() {
        val file = recordFile()
        write(FIRST)
        io.kept.add(target.path)

        val failure = file.delete().refusal()

        assertTrue(failure.toString(), failure is VaultFailure.RemovalFailed)
        assertEquals(RemovalStage.Record, (failure as VaultFailure.RemovalFailed).stage)
    }

    @Test
    fun aDeletionThatFaultsPartWayThroughKeepsWhatItHadRemoved() {
        val file = recordFile()
        val legacy = File(target.path + LEGACY_SUFFIX)
        write(FIRST)
        staging().writeBytes(SECOND)
        legacy.writeBytes(FIRST)
        io.faulting.add(legacy.path)
        io.removeFault = { SecurityException(QUOTED_INPUT) }

        val failure = answered { file.delete() }.refusal()

        val refused = failure as VaultFailure.RemovalFailed
        assertEquals(originOf(SecurityException()), refused.origin)
        assertEquals(REMOVED_BEFORE_THE_FAULT.toLong(), refused.removed.toLong())
        assertFalse(refused.toString(), refused.toString().contains(QUOTED_INPUT))
        assertFalse(target.exists())
        assertFalse(staging().exists())
    }

    @Test
    fun aDeletionUnlinksTheRecoverySidecarAndLeavesWhatItPointsAt() {
        val file = recordFile()
        write(FIRST)
        val elsewhere = File(temporary.newFolder("outside"), "sentinel.bin")
        elsewhere.writeBytes(SENTINEL)
        link(File(target.path + LEGACY_SUFFIX), elsewhere)

        assertTrue(file.delete().completed())

        assertFalse("the sidecar link was left behind", listed(target.name + LEGACY_SUFFIX))
        assertTrue("what the sidecar pointed at went", elsewhere.isFile)
        assertArrayEquals(SENTINEL, elsewhere.readBytes())
    }

    @Test
    fun aRecordLargerThanTheLimitIsRefusedRatherThanReturned() {
        val file = recordFile()
        write(ByteArray(LIMIT + 1) { 7 })

        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.Oversized),
            file.read(LIMIT).refusal(),
        )
    }

    private fun recordFile(): AtomicVaultRecordFile {
        directory = File(temporary.newFolder("private"), "vault")
        target = File(directory, RECORD)
        return AtomicVaultRecordFile(target, io)
    }

    private fun staging(): File = File(target.path + STAGING_SUFFIX)

    private fun write(record: ByteArray) {
        assertTrue(directory.isDirectory || directory.mkdirs())
        target.writeBytes(record)
    }

    /** Whether the record's own directory holds [name], link or not. */
    private fun listed(name: String): Boolean = directory.list()?.contains(name) == true

    /** How far the replacement that [failure] reports got, cleanup aside. */
    private fun phaseOf(failure: VaultFailure): CommitPhase = written(failure).phase

    /** What the replacement that [failure] reports failed on, cleanup aside. */
    private fun originIn(failure: VaultFailure): String = written(failure).origin

    private fun written(failure: VaultFailure): VaultFailure.WriteFailed {
        val primary = if (failure is VaultFailure.CleanupAfter) failure.primary else failure
        assertTrue(failure.toString(), primary is VaultFailure.WriteFailed)
        return primary as VaultFailure.WriteFailed
    }

    private companion object {

        const val RECORD = "credential.bin"

        const val STAGING = "credential.bin.new"

        const val STAGING_SUFFIX = ".new"

        /** The sidecar the platform's own atomic file used to keep. */
        const val LEGACY_SUFFIX = ".bak"

        const val LIMIT = 64

        val FIRST = ByteArray(8) { 1 }

        val SECOND = ByteArray(12) { 2 }

        /** Bytes of a file no record owns, in a directory no record owns. */
        val SENTINEL = ByteArray(16) { 9 }

        /** What a path leaving its own directory is refused as. */
        val ESCAPING = VaultFailure.RefusedInput(PRIVATE_PATH_FIELD, InputDefect.Escaping)

        /** The record and the staging record, taken before the sidecar faulted. */
        const val REMOVED_BEFORE_THE_FAULT = 2
    }
}
