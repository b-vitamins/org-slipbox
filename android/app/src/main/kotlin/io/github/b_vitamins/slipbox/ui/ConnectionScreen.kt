/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Handler
import android.os.Looper
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
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.github.ConfirmedGithubSelection
import io.github.b_vitamins.slipbox.github.GithubBranch
import io.github.b_vitamins.slipbox.github.GithubBrowsing
import io.github.b_vitamins.slipbox.github.GithubInstallation
import io.github.b_vitamins.slipbox.github.GithubListing
import io.github.b_vitamins.slipbox.github.GithubNames
import io.github.b_vitamins.slipbox.github.GithubOutcome
import io.github.b_vitamins.slipbox.github.GithubRepository
import io.github.b_vitamins.slipbox.github.GithubSelectionRequest
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.sources.InitialSourceImportEvent
import io.github.b_vitamins.slipbox.sources.InitialSourceImportOwner
import io.github.b_vitamins.slipbox.sources.ReadySource
import io.github.b_vitamins.slipbox.sources.SourceCatalogGateway
import io.github.b_vitamins.slipbox.sync.RefreshProvider
import io.github.b_vitamins.slipbox.sync.RefreshFailureReason
import io.github.b_vitamins.slipbox.sync.RefreshSource
import io.github.b_vitamins.slipbox.sync.RefreshState
import io.github.b_vitamins.slipbox.sync.RefreshRetry
import io.github.b_vitamins.slipbox.sync.RefreshVisibility
import io.github.b_vitamins.slipbox.sync.SourceRefreshOutcome
import io.github.b_vitamins.slipbox.sync.SourceRefreshRuntime
import io.github.b_vitamins.slipbox.sync.SourceRefreshScheduler
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPanel
import io.github.b_vitamins.slipbox.ui.auth.AuthorizationPhase
import io.github.b_vitamins.slipbox.ui.auth.rememberGithubAuthorization
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import java.net.URI
import java.security.SecureRandom

private enum class ConnectionKind {
    Public,
    Github,
}

private sealed interface GithubStep {
    data object Loading : GithubStep
    data class Installations(val values: List<GithubInstallation>) : GithubStep
    data class Repositories(
        val installation: GithubInstallation,
        val values: List<GithubRepository>,
    ) : GithubStep
    data class Branches(
        val installation: GithubInstallation,
        val repository: GithubRepository,
        val values: List<GithubBranch>,
    ) : GithubStep
    data object Confirming : GithubStep
    data object Failed : GithubStep
}

private sealed interface ImportPresentation {
    data object Idle : ImportPresentation
    data object Securing : ImportPresentation
    data class Running(val event: InitialSourceImportEvent.Progress) : ImportPresentation
    data object Cancelling : ImportPresentation
    data class Failed(val event: InitialSourceImportEvent.Failed) : ImportPresentation
}

@Composable
internal fun ConnectionScreen(
    catalogRevision: Long,
    onBack: () -> Unit,
    onReady: (ReadySource, Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val importer =
        remember(context) {
            InitialSourceImportOwner(
                SourceRefreshRuntime.packaged(context.applicationContext),
                SourceCatalogGateway(context.applicationContext),
            )
        }
    val scheduler = remember(context) { SourceRefreshScheduler.packaged(context.applicationContext) }
    DisposableEffect(importer) { onDispose(importer::close) }

    var kind by remember { mutableStateOf(ConnectionKind.Public) }
    var url by remember { mutableStateOf("") }
    var branch by remember { mutableStateOf("main") }
    var folder by remember { mutableStateOf("") }
    var inputError by remember { mutableStateOf(false) }
    var pending by remember { mutableStateOf<RefreshSource?>(null) }
    var importing by remember { mutableStateOf<ImportPresentation>(ImportPresentation.Idle) }

    fun accept(event: InitialSourceImportEvent) {
        importing =
            when (event) {
                is InitialSourceImportEvent.Progress -> ImportPresentation.Running(event)
                is InitialSourceImportEvent.Ready -> {
                    scheduler.imported(event.source.source)
                    onReady(event.source, event.catalogRevision)
                    ImportPresentation.Idle
                }
                is InitialSourceImportEvent.Failed -> ImportPresentation.Failed(event)
                InitialSourceImportEvent.Cancelled -> ImportPresentation.Idle
            }
    }

    fun begin(source: RefreshSource) {
        pending = source
        if (importer.begin(source, catalogRevision, ::accept)) {
            importing = ImportPresentation.Running(InitialSourceImportEvent.Progress(null))
        }
    }

    ReadingSurface(
        title = stringResource(R.string.connection_title),
        modifier = modifier,
        leading = {
            IconControl(
                icon = painterResource(R.drawable.ic_back),
                label = stringResource(R.string.action_back),
                onClick = onBack,
            )
        },
    ) {
        Column(
            verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding),
        ) {
            if (importing == ImportPresentation.Idle) {
                ChoiceRow(
                    label = stringResource(R.string.connection_public),
                    selected = kind == ConnectionKind.Public,
                    onSelect = { kind = ConnectionKind.Public },
                )
                ChoiceRow(
                    label = stringResource(R.string.connection_github),
                    selected = kind == ConnectionKind.Github,
                    onSelect = { kind = ConnectionKind.Github },
                )
                HorizontalDivider(color = MaterialTheme.colorScheme.outline)
                when (kind) {
                    ConnectionKind.Public ->
                        PublicConnection(
                            url = url,
                            branch = branch,
                            folder = folder,
                            invalid = inputError,
                            onUrl = { url = it; inputError = false },
                            onBranch = { branch = it; inputError = false },
                            onFolder = { folder = it; inputError = false },
                            onConnect = {
                                val source = publicSource(url, branch, folder)
                                inputError = source == null
                                source?.let(::begin)
                            },
                        )
                    ConnectionKind.Github ->
                        GithubConnection(
                            folder = folder,
                            onFolder = { folder = it },
                            onSecuring = { importing = ImportPresentation.Securing },
                            onCredentialFailure = { importing = ImportPresentation.Idle },
                            onSource = ::begin,
                        )
                }
            } else {
                ImportProgress(
                    presentation = importing,
                    onCancel = {
                        importing = ImportPresentation.Cancelling
                        importer.cancel()
                    },
                    onRetry = { pending?.let(::begin) },
                    onReconnect = {
                        pending = null
                        kind = ConnectionKind.Github
                        importing = ImportPresentation.Idle
                    },
                )
            }
        }
    }
}

@Composable
private fun PublicConnection(
    url: String,
    branch: String,
    folder: String,
    invalid: Boolean,
    onUrl: (String) -> Unit,
    onBranch: (String) -> Unit,
    onFolder: (String) -> Unit,
    onConnect: () -> Unit,
) {
    SourceFields(url, branch, folder, onUrl, onBranch, onFolder)
    if (invalid) Problem(stringResource(R.string.connection_invalid_public))
    TextControl(label = stringResource(R.string.action_connect), onClick = onConnect)
}

@Composable
private fun GithubConnection(
    folder: String,
    onFolder: (String) -> Unit,
    onSecuring: () -> Unit,
    onCredentialFailure: () -> Unit,
    onSource: (RefreshSource) -> Unit,
) {
    val context = LocalContext.current
    val authorizationState = rememberGithubAuthorization()
    val authorization =
        ((authorizationState.phase as? AuthorizationPhase.Settled)?.outcome
            as? AuthorizationOutcome.Authorized)?.authorization

    if (authorization == null) {
        if (authorizationState.phase is AuthorizationPhase.Idle) {
            TextControl(
                label = stringResource(R.string.connection_authorize),
                onClick = authorizationState::begin,
            )
        }
        AuthorizationPanel(authorizationState)
        return
    }
    if (!authorization.access.isInstalled) {
        AuthorizationPanel(authorizationState)
        TextControl(
            label = stringResource(R.string.action_retry),
            onClick = authorizationState::begin,
        )
        return
    }

    val browsing = remember(authorization) { GithubBrowsing.packaged(authorization) }
    DisposableEffect(browsing) { onDispose(browsing::close) }
    var step by remember(authorization) { mutableStateOf<GithubStep>(GithubStep.Loading) }

    fun installations() {
        step = GithubStep.Loading
        browsing.browse({ it.installations() }) { outcome ->
            step = outcome.listing()?.let { GithubStep.Installations(it.entries) } ?: GithubStep.Failed
        }
    }

    LaunchedEffect(browsing) { installations() }

    when (val current = step) {
        GithubStep.Loading -> NoticeText(stringResource(R.string.connection_loading_github))
        is GithubStep.Installations -> {
            SectionLabel(stringResource(R.string.connection_installations))
            current.values.forEach { installation ->
                ChoiceRow(
                    label = installation.accountLogin,
                    selected = false,
                    onSelect = {
                        step = GithubStep.Loading
                        browsing.browse({ it.repositories(installation) }) { outcome ->
                            step =
                                outcome.listing()?.let {
                                    GithubStep.Repositories(installation, it.entries)
                                } ?: GithubStep.Failed
                        }
                    },
                )
            }
        }
        is GithubStep.Repositories -> {
            SectionLabel(stringResource(R.string.connection_repositories))
            current.values.forEach { repository ->
                ChoiceRow(
                    label = "${repository.owner}/${repository.name}",
                    selected = false,
                    onSelect = {
                        step = GithubStep.Loading
                        browsing.browse({ it.branches(repository) }) { outcome ->
                            step =
                                outcome.listing()?.let {
                                    GithubStep.Branches(current.installation, repository, it.entries)
                                } ?: GithubStep.Failed
                        }
                    },
                )
            }
        }
        is GithubStep.Branches -> {
            SectionLabel(stringResource(R.string.connection_branches))
            OutlinedTextField(
                value = folder,
                onValueChange = onFolder,
                label = { Text(stringResource(R.string.connection_folder)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            current.values.forEach { selectedBranch ->
                ChoiceRow(
                    label = selectedBranch.name,
                    selected = false,
                    onSelect = {
                        step = GithubStep.Confirming
                        val request =
                            GithubSelectionRequest(
                                accountId = authorization.account.id,
                                installationId = current.installation.id,
                                repositoryId = current.repository.id,
                                branch = selectedBranch.name,
                                folder = folder,
                            )
                        browsing.browse({ it.confirm(request) }) { outcome ->
                            val confirmed = (outcome as? GithubOutcome.Read)?.value
                            if (confirmed == null) {
                                step = GithubStep.Failed
                            } else {
                                secureAndImport(
                                    context = context.applicationContext,
                                    authorization = authorization,
                                    repository = current.repository,
                                    selection = confirmed,
                                    onSecuring = onSecuring,
                                    onFailure = {
                                        onCredentialFailure()
                                        step = GithubStep.Failed
                                    },
                                    onSource = onSource,
                                )
                            }
                        }
                    },
                )
            }
        }
        GithubStep.Confirming -> NoticeText(stringResource(R.string.connection_loading_github))
        GithubStep.Failed -> {
            Problem(stringResource(R.string.connection_github_failed))
            TextControl(label = stringResource(R.string.action_retry), onClick = ::installations)
        }
    }
}

@Composable
private fun SourceFields(
    url: String,
    branch: String,
    folder: String,
    onUrl: (String) -> Unit,
    onBranch: (String) -> Unit,
    onFolder: (String) -> Unit,
) {
    OutlinedTextField(
        value = url,
        onValueChange = onUrl,
        label = { Text(stringResource(R.string.connection_url)) },
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
    )
    OutlinedTextField(
        value = branch,
        onValueChange = onBranch,
        label = { Text(stringResource(R.string.connection_branch)) },
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
    )
    OutlinedTextField(
        value = folder,
        onValueChange = onFolder,
        label = { Text(stringResource(R.string.connection_folder)) },
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
    )
}

@Composable
private fun ImportProgress(
    presentation: ImportPresentation,
    onCancel: () -> Unit,
    onRetry: () -> Unit,
    onReconnect: () -> Unit,
) {
    when (presentation) {
        ImportPresentation.Securing -> {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            NoticeText(stringResource(R.string.connection_saving_credential))
        }
        is ImportPresentation.Running -> {
            val status = presentation.event.status
            val progress = status?.progress
            if (progress != null && progress.total > 0) {
                LinearProgressIndicator(
                    progress = { (progress.completed.toFloat() / progress.total).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth(),
                )
            } else {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }
            NoticeText(
                stringResource(
                    when (status?.state) {
                        RefreshState.FETCHING -> R.string.connection_fetching
                        RefreshState.INDEXING -> R.string.connection_indexing
                        RefreshState.PUBLISHING -> R.string.connection_publishing
                        else -> R.string.connection_preparing
                    },
                ),
            )
            TextControl(label = stringResource(R.string.action_cancel), onClick = onCancel)
        }
        ImportPresentation.Cancelling -> NoticeText(stringResource(R.string.connection_cancelling))
        is ImportPresentation.Failed -> {
            Problem(stringResource(presentation.event.message()))
            Row(horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding)) {
                if (presentation.event.requiresAuthorization()) {
                    TextControl(
                        label = stringResource(R.string.connection_reconnect),
                        onClick = onReconnect,
                    )
                } else {
                    TextControl(label = stringResource(R.string.action_retry), onClick = onRetry)
                }
            }
        }
        ImportPresentation.Idle -> Unit
    }
}

private fun InitialSourceImportEvent.Failed.requiresAuthorization(): Boolean =
    when (val outcome = refresh) {
        is SourceRefreshOutcome.Answered -> outcome.status.failure?.retry is RefreshRetry.Reauthorize
        is SourceRefreshOutcome.Refused -> outcome.failure.retry is RefreshRetry.Reauthorize
        is SourceRefreshOutcome.ContractFailed, null -> false
    }

private fun InitialSourceImportEvent.Failed.message(): Int {
    if (catalog != null) return R.string.connection_catalog_failed
    val failure =
        when (val outcome = refresh) {
            is SourceRefreshOutcome.Answered -> outcome.status.failure
            is SourceRefreshOutcome.Refused -> outcome.failure
            is SourceRefreshOutcome.ContractFailed, null -> null
        }
    return when (failure?.reason) {
        RefreshFailureReason.AUTHORIZATION_REQUIRED,
        RefreshFailureReason.AUTHORIZATION_FAILED -> R.string.connection_authorization_failed
        RefreshFailureReason.TRANSPORT_FAILED,
        RefreshFailureReason.TLS_INITIALIZATION_FAILED,
        RefreshFailureReason.RATE_LIMITED -> R.string.connection_reach_failed
        RefreshFailureReason.REPOSITORY_INVALID,
        RefreshFailureReason.BRANCH_UNAVAILABLE,
        RefreshFailureReason.NOTES_FOLDER_UNAVAILABLE -> R.string.connection_selection_failed
        RefreshFailureReason.STORAGE_FAILED -> R.string.connection_storage_failed
        else -> R.string.connection_import_failed
    }
}

private fun secureAndImport(
    context: android.content.Context,
    authorization: GithubAuthorization,
    repository: GithubRepository,
    selection: ConfirmedGithubSelection,
    onSecuring: () -> Unit,
    onFailure: () -> Unit,
    onSource: (RefreshSource) -> Unit,
) {
    val id = sourceId()
    val credential = if (repository.isPrivate) "slipbox.source.$id" else null
    val source =
        RefreshSource(
            id = id,
            displayName = "${selection.owner}/${selection.name}",
            provider = RefreshProvider.GITHUB,
            visibility =
                if (repository.isPrivate) RefreshVisibility.PRIVATE else RefreshVisibility.PUBLIC,
            providerRepositoryId = selection.repositoryId,
            account = if (repository.isPrivate) selection.accountId else null,
            remote = selection.remoteUrl,
            branch = selection.branch,
            notesFolder = selection.notesFolder,
            credential = credential,
        )
    if (credential == null) {
        onSource(source)
        return
    }
    onSecuring()
    val main = Handler(Looper.getMainLooper())
    AuthorizationVault(context).keep(
        sourceId = id,
        credentialRef = credential,
        authorization = authorization,
        recipient =
            VaultRecipient { outcome ->
                main.post {
                    when (outcome) {
                        is VaultOutcome.Completed -> onSource(source)
                        is VaultOutcome.Failed -> onFailure()
                    }
                }
            },
    )
}

internal fun publicSource(url: String, branch: String, folder: String): RefreshSource? {
    val remote = url.trim()
    val parsed = runCatching { URI(remote) }.getOrNull() ?: return null
    val host = parsed.host ?: return null
    if (!parsed.scheme.equals("https", ignoreCase = true) ||
        host.isBlank() ||
        parsed.userInfo != null ||
        parsed.query != null || parsed.fragment != null || parsed.path.isNullOrBlank()
    ) {
        return null
    }
    val path = parsed.path.removeSuffix("/")
    if (remote.length > 512 || parsed.port == 0 || host.length > 253 ||
        host.split('.').any {
            it.isEmpty() || it.length > 63 || it.startsWith('-') || it.endsWith('-') ||
                it.any { character -> !character.isAsciiAlphaNumeric() && character != '-' }
        } ||
        path.isEmpty() ||
        path.removePrefix("/").split('/').any {
            it.isEmpty() || it == "." || it == ".." ||
                it.any { character -> !character.isAsciiAlphaNumeric() && character !in "-_.~" }
        }
    ) {
        return null
    }
    val canonicalBranch = GithubNames.branch(branch) ?: return null
    val canonicalFolder = GithubNames.folder(folder) ?: return null
    val authority =
        host.lowercase() + if (parsed.port == -1 || parsed.port == 443) "" else ":${parsed.port}"
    val canonicalRemote = "https://$authority$path"
    val name =
        path.substringAfterLast('/').removeSuffix(".git").ifBlank { host }
    return RefreshSource(
        id = sourceId(),
        displayName = name,
        provider = RefreshProvider.GENERIC_HTTPS,
        visibility = RefreshVisibility.PUBLIC,
        remote = canonicalRemote,
        branch = canonicalBranch,
        notesFolder = canonicalFolder,
    )
}

private fun sourceId(): String {
    val bytes = ByteArray(16).also(SecureRandom()::nextBytes)
    val digits = "0123456789abcdef"
    return buildString(32) {
        for (byte in bytes) {
            val value = byte.toInt() and 0xff
            append(digits[value ushr 4]).append(digits[value and 0x0f])
        }
    }
}

private fun Char.isAsciiAlphaNumeric(): Boolean =
    this in 'A'..'Z' || this in 'a'..'z' || this in '0'..'9'

private fun <T> GithubOutcome<GithubListing<T>>.listing(): GithubListing<T>? =
    (this as? GithubOutcome.Read)?.value

@Composable
private fun SectionLabel(text: String) {
    Text(text = text, style = MaterialTheme.typography.titleMedium)
}

@Composable
private fun NoticeText(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun Problem(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.error,
    )
}
