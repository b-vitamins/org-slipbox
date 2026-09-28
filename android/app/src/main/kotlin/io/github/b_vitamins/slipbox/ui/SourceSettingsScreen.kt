/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.AuthorizationVault
import io.github.b_vitamins.slipbox.github.GithubNames
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.sources.ImportDelivery
import io.github.b_vitamins.slipbox.sources.InitialSourceImportEvent
import io.github.b_vitamins.slipbox.sources.InitialSourceImportOwner
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.SourceActivation
import io.github.b_vitamins.slipbox.sources.SourceCatalogGateway
import io.github.b_vitamins.slipbox.sources.SourceCatalogListing
import io.github.b_vitamins.slipbox.sources.SourceManagement
import io.github.b_vitamins.slipbox.sources.SourceManagementResult
import io.github.b_vitamins.slipbox.sync.RefreshRetry
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshState
import io.github.b_vitamins.slipbox.sync.RefreshStatus
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.sync.SourceRefreshRuntime
import io.github.b_vitamins.slipbox.sync.SourceRefreshScheduler
import io.github.b_vitamins.slipbox.sync.SourceRefreshStatusOutcome
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPanel
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPhase
import io.github.b_vitamins.slipbox.ui.auth.rememberGithubAuthorization
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

private enum class DestructiveSourceAction {
    Disconnect,
    Cache,
    Source,
}

private sealed interface SourceSettingsWork {
    data object Idle : SourceSettingsWork
    data class Importing(val event: InitialSourceImportEvent.Progress) : SourceSettingsWork
    data object Removing : SourceSettingsWork
    data class Notice(val message: Int, val problem: Boolean = false) : SourceSettingsWork
}

@Composable
internal fun SourceSettingsScreen(
    sourceId: String,
    catalog: SourceCatalogListing,
    ready: ReadySource?,
    onBack: () -> Unit,
    onOpenSource: (RefreshSource) -> Unit,
    onSelect: (RefreshSource, (Boolean) -> Unit) -> Unit,
    onReady: (ReadySource, Long) -> Unit,
    onCacheRemoved: (RefreshSource) -> Unit,
    onSourceRemoved: (SourceCatalogListing, RefreshSource, Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    var removalProblem by remember(sourceId) { mutableStateOf(false) }
    val source = catalog.sources.firstOrNull { it.id == sourceId }
    if (source == null) {
        ReadingSurface(
            title = stringResource(R.string.sources_title),
            modifier = modifier,
            leading = {
                IconControl(
                    icon = painterResource(R.drawable.ic_back),
                    label = stringResource(R.string.action_back),
                    onClick = onBack,
                )
            },
        ) {
            NoticeText(
                stringResource(
                    if (removalProblem) {
                        R.string.source_removed_cleanup_failed
                    } else {
                        R.string.source_removed
                    },
                ),
                problem = removalProblem,
            )
        }
        return
    }

    val context = LocalContext.current
    val gateway = remember(context) { SourceCatalogGateway(context.applicationContext) }
    val runtime = remember(context) { SourceRefreshRuntime.packaged(context.applicationContext) }
    val scheduler = remember(context) { SourceRefreshScheduler.packaged(context.applicationContext) }
    val management = remember(context) { SourceManagement(context.applicationContext) }
    val activation =
        remember(gateway, catalog.revision, source) {
            SourceActivation { _, replacement, generation ->
                gateway.replace(catalog.revision, source, replacement, generation)
            }
        }
    val importer =
        remember(runtime, activation) {
            InitialSourceImportOwner(runtime, activation, ImportDelivery.MainThread)
        }
    DisposableEffect(importer) { onDispose(importer::close) }
    DisposableEffect(management) { onDispose(management::close) }

    var branch by remember(source) { mutableStateOf(source.branch) }
    var folder by remember(source) { mutableStateOf(source.notesFolder) }
    var invalidConfiguration by remember(source) { mutableStateOf(false) }
    var confirmation by remember(source) { mutableStateOf<DestructiveSourceAction?>(null) }
    var work by remember(source) { mutableStateOf<SourceSettingsWork>(SourceSettingsWork.Idle) }
    var observedReady by remember(source.id) {
        mutableStateOf(ready?.takeIf { it.source.id == source.id })
    }
    LaunchedEffect(ready, source.id) {
        ready?.takeIf { it.source.id == source.id }?.let { observedReady = it }
    }
    val authorization = rememberGithubAuthorization()
    var securingAuthorization by remember(source) { mutableStateOf(false) }

    val authorizationOutcome =
        (authorization.phase as? AuthorizationPhase.Settled)?.outcome
    LaunchedEffect(authorizationOutcome, securingAuthorization, source) {
        if (!securingAuthorization) return@LaunchedEffect
        val outcome = authorizationOutcome ?: return@LaunchedEffect
        val accepted =
            (outcome as? AuthorizationOutcome.Authorized)?.authorization
                ?: run {
                    securingAuthorization = false
                    work = SourceSettingsWork.Notice(R.string.source_reconnect_failed, true)
                    return@LaunchedEffect
                }
        if (accepted.account.id != source.account || source.credential == null) {
            securingAuthorization = false
            work = SourceSettingsWork.Notice(R.string.source_reconnect_account_mismatch, true)
            return@LaunchedEffect
        }
        AuthorizationVault(context.applicationContext).keep(
            sourceId = source.id,
            credentialRef = source.credential,
            authorization = accepted,
            recipient =
                VaultRecipient { outcome ->
                    ImportDelivery.MainThread.post {
                        securingAuthorization = false
                        work =
                            if (outcome is VaultOutcome.Completed) {
                                scheduler.configure(null, source)
                                SourceSettingsWork.Notice(R.string.source_reconnected)
                            } else {
                                SourceSettingsWork.Notice(R.string.source_reconnect_failed, true)
                            }
                    }
                },
        )
    }

    fun acceptImport(event: InitialSourceImportEvent) {
        work =
            when (event) {
                is InitialSourceImportEvent.Progress -> SourceSettingsWork.Importing(event)
                is InitialSourceImportEvent.Ready -> {
                    observedReady = event.source
                    scheduler.imported(source, event.source.source)
                    onReady(event.source, event.catalogRevision)
                    SourceSettingsWork.Notice(R.string.source_configuration_ready)
                }
                is InitialSourceImportEvent.Failed -> {
                    scheduler.configure(null, source)
                    SourceSettingsWork.Notice(R.string.source_configuration_failed, true)
                }
                InitialSourceImportEvent.Cancelled -> {
                    scheduler.configure(null, source)
                    SourceSettingsWork.Idle
                }
            }
    }

    fun actionAvailable(): Boolean =
        work !is SourceSettingsWork.Importing &&
            work !is SourceSettingsWork.Removing &&
            !securingAuthorization

    fun beginImport(replacement: RefreshSource) {
        if (!actionAvailable()) return
        scheduler.remove(source.id)
        if (importer.begin(replacement, catalog.revision, ::acceptImport)) {
            work = SourceSettingsWork.Importing(InitialSourceImportEvent.Progress(null))
        }
    }

    fun beginConfigurationChange() {
        val nextBranch = GithubNames.branch(branch)
        val nextFolder = GithubNames.folder(folder)
        invalidConfiguration = nextBranch == null || nextFolder == null
        if (invalidConfiguration || nextBranch == null || nextFolder == null) return
        val replacement = source.copy(branch = nextBranch, notesFolder = nextFolder)
        if (replacement == source) return
        beginImport(replacement)
    }

    fun completeManagement(result: SourceManagementResult) {
        confirmation = null
        work =
            when (result) {
                SourceManagementResult.Disconnected ->
                    SourceSettingsWork.Notice(R.string.source_disconnected)
                is SourceManagementResult.CacheRemoved -> {
                    onCacheRemoved(source)
                    SourceSettingsWork.Notice(R.string.source_cache_removed)
                }
                is SourceManagementResult.SourceRemoved -> {
                    val cleanupComplete = result.cacheRemoved && result.credentialRemoved
                    removalProblem = !cleanupComplete
                    onSourceRemoved(result.catalog, source, cleanupComplete)
                    if (cleanupComplete) {
                        SourceSettingsWork.Notice(R.string.source_removed)
                    } else {
                        SourceSettingsWork.Notice(R.string.source_removed_cleanup_failed, true)
                    }
                }
                is SourceManagementResult.Failed ->
                    SourceSettingsWork.Notice(R.string.source_action_failed, true)
            }
    }

    ReadingSurface(
        title = stringResource(R.string.sources_title),
        modifier = modifier,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = onBack,
            )
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding)) {
            SectionLabel(stringResource(R.string.sources_configured))
            catalog.sources.forEach { candidate ->
                ChoiceRow(
                    label = candidate.displayName,
                    selected = catalog.activeSource?.id == candidate.id,
                    onSelect = { if (candidate.id != source.id) onOpenSource(candidate) },
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outline)
            SectionLabel(source.displayName)
            if (catalog.activeSource?.id != source.id) {
                TextControl(
                    label = stringResource(R.string.source_make_active),
                    onClick = {
                        onSelect(source) { selected ->
                            if (!selected) {
                                work =
                                    SourceSettingsWork.Notice(
                                        R.string.source_selection_unavailable,
                                        true,
                                    )
                            }
                        }
                    },
                )
            }
            Detail(R.string.source_repository, source.remote)
            OutlinedTextField(
                value = branch,
                onValueChange = { branch = it; invalidConfiguration = false },
                label = { Text(stringResource(R.string.connection_branch)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            OutlinedTextField(
                value = folder,
                onValueChange = { folder = it; invalidConfiguration = false },
                label = { Text(stringResource(R.string.connection_folder)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            if (invalidConfiguration) {
                NoticeText(stringResource(R.string.source_configuration_invalid), problem = true)
            }
            TextControl(
                label = stringResource(R.string.source_apply_configuration),
                onClick = ::beginConfigurationChange,
            )
            TextControl(
                label = stringResource(R.string.source_refresh_now),
                onClick = { beginImport(source) },
            )
            FreshnessDetails(source, observedReady, runtime.status(source.id))
            WorkStatus(work, importer)
            if (source.visibility == RefreshVisibility.PRIVATE) {
                HorizontalDivider(color = MaterialTheme.colorScheme.outline)
                SectionLabel(stringResource(R.string.source_access))
                TextControl(
                    label = stringResource(R.string.source_reconnect),
                    onClick = {
                        if (actionAvailable()) {
                            securingAuthorization = true
                            authorization.begin()
                        }
                    },
                )
                AuthorizationPanel(authorization)
                TextControl(
                    label = stringResource(R.string.source_disconnect),
                    onClick = {
                        if (actionAvailable()) {
                            confirmation = DestructiveSourceAction.Disconnect
                        }
                    },
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outline)
            SectionLabel(stringResource(R.string.source_local_data))
            TextControl(
                label = stringResource(R.string.source_remove_cache),
                onClick = {
                    if (actionAvailable()) {
                        confirmation = DestructiveSourceAction.Cache
                    }
                },
            )
            TextControl(
                label = stringResource(R.string.source_remove),
                onClick = {
                    if (actionAvailable()) {
                        confirmation = DestructiveSourceAction.Source
                    }
                },
            )
            confirmation?.let { action ->
                Confirmation(
                    action = action,
                    source = source,
                    onCancel = { confirmation = null },
                    onConfirm = {
                        if (actionAvailable()) {
                            confirmation = null
                            work = SourceSettingsWork.Removing
                            when (action) {
                                DestructiveSourceAction.Disconnect ->
                                    management.disconnect(source, ::completeManagement)
                                DestructiveSourceAction.Cache ->
                                    management.removeCache(source, ::completeManagement)
                                DestructiveSourceAction.Source ->
                                    management.removeSource(
                                        catalog.revision,
                                        source,
                                        ::completeManagement,
                                    )
                            }
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun FreshnessDetails(
    source: RefreshSource,
    ready: ReadySource?,
    outcome: SourceRefreshStatusOutcome,
) {
    val active = ready?.takeIf { it.source.id == source.id }
    Detail(
        R.string.source_ready_revision,
        active?.revision?.take(12) ?: stringResource(R.string.source_not_ready),
    )
    val status = (outcome as? SourceRefreshStatusOutcome.Known)?.status
    Detail(R.string.source_freshness, freshness(status, active != null))
    status?.failure?.let { failure ->
        Detail(
            R.string.source_recovery,
            stringResource(
                when (failure.retry) {
                    RefreshRetry.Reauthorize -> R.string.source_recovery_reconnect
                    RefreshRetry.FreeStorage -> R.string.source_recovery_storage
                    RefreshRetry.Rebuild -> R.string.source_recovery_rebuild
                    is RefreshRetry.Backoff -> R.string.source_recovery_retry
                    RefreshRetry.Never -> R.string.source_recovery_manual
                },
            ),
        )
    }
}

@Composable
private fun freshness(status: RefreshStatus?, ready: Boolean): String =
    when (status?.state) {
        RefreshState.RECOVERING -> stringResource(R.string.source_freshness_recovering)
        RefreshState.FETCHING,
        RefreshState.COMPARING,
        RefreshState.MATERIALIZING,
        RefreshState.INDEXING,
        RefreshState.PUBLISHING -> stringResource(R.string.source_freshness_refreshing)
        RefreshState.FAILED ->
            if (ready) stringResource(R.string.source_freshness_offline) else stringResource(R.string.source_not_ready)
        RefreshState.CANCELLED, null ->
            if (ready) stringResource(R.string.source_freshness_offline) else stringResource(R.string.source_not_ready)
        RefreshState.READY -> stringResource(R.string.source_freshness_ready)
    }

@Composable
private fun WorkStatus(work: SourceSettingsWork, importer: InitialSourceImportOwner) {
    when (work) {
        SourceSettingsWork.Idle -> Unit
        SourceSettingsWork.Removing -> {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            NoticeText(stringResource(R.string.source_removing))
        }
        is SourceSettingsWork.Importing -> {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            NoticeText(stringResource(R.string.source_configuration_importing))
            TextControl(label = stringResource(R.string.action_cancel), onClick = importer::cancel)
        }
        is SourceSettingsWork.Notice ->
            NoticeText(stringResource(work.message), problem = work.problem)
    }
}

@Composable
private fun Confirmation(
    action: DestructiveSourceAction,
    source: RefreshSource,
    onCancel: () -> Unit,
    onConfirm: () -> Unit,
) {
    val message =
        when (action) {
            DestructiveSourceAction.Disconnect -> R.string.source_confirm_disconnect
            DestructiveSourceAction.Cache -> R.string.source_confirm_cache
            DestructiveSourceAction.Source -> R.string.source_confirm_remove
        }
    NoticeText(stringResource(message, source.displayName), problem = true)
    Row(horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding)) {
        TextControl(label = stringResource(R.string.action_cancel), onClick = onCancel)
        TextControl(label = stringResource(R.string.action_confirm), onClick = onConfirm)
    }
}

@Composable
private fun Detail(label: Int, value: String) {
    Text(
        text = stringResource(label, value),
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SectionLabel(text: String) {
    Text(text = text, style = MaterialTheme.typography.titleMedium)
}

@Composable
private fun NoticeText(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
