/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.auth.InstallationAccess
import io.github.b_vitamins.slipbox.auth.RepositorySelection
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.security.StoredCredential
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

internal class RecordedApiTransport : GithubApiTransport {

    private val queued = mutableMapOf<String, ArrayDeque<GithubApiReply>>()

    private val fetched = CopyOnWriteArrayList<GithubApiRequest>()

    var beforeFetch: (GithubApiRequest) -> Unit = {}

    val requests: List<GithubApiRequest>
        get() = fetched.toList()

    val urls: List<String>
        get() = requests.map { it.address.url }

    fun answers(url: String, vararg replies: GithubApiReply): RecordedApiTransport {
        synchronized(queued) { queued.getOrPut(url) { ArrayDeque() }.addAll(replies) }
        return this
    }

    override fun fetch(request: GithubApiRequest): GithubApiReply {
        fetched.add(request)
        beforeFetch(request)
        val url = request.address.url
        return synchronized(queued) {
            val replies = queued[url] ?: throw AssertionError("no reply prepared for $url")
            if (replies.size > 1) replies.removeFirst() else replies.first()
        }
    }
}

internal val DirectDelivery: GithubDelivery = GithubDelivery { it() }

internal class QueuedDelivery : GithubDelivery {

    private val lock = ReentrantLock()

    private val arrived = lock.newCondition()

    private val pending = ArrayDeque<() -> Unit>()

    private var posts = 0

    override fun post(action: () -> Unit) {
        lock.withLock {
            pending.addLast(action)
            posts++
            arrived.signalAll()
        }
    }

    fun awaitPosts(count: Int, millis: Long = WAIT_MILLIS): Boolean {
        val deadline = System.currentTimeMillis() + millis
        lock.withLock {
            while (posts < count) {
                val left = deadline - System.currentTimeMillis()
                if (left <= 0) {
                    return false
                }
                arrived.await(left, TimeUnit.MILLISECONDS)
            }
            return true
        }
    }

    fun drain(): Int {
        var ran = 0
        while (true) {
            val action = lock.withLock { pending.removeFirstOrNull() } ?: return ran
            action()
            ran++
        }
    }
}

internal fun answered(
    body: String,
    status: Int = 200,
    link: String? = null,
    retryAfter: String? = null,
    rateLimitRemaining: String? = null,
    rateLimitReset: String? = null,
    singleSignOn: String? = null,
    bodyExceeded: Boolean = false,
): GithubApiReply.Answered =
    GithubApiReply.Answered(
        status,
        body,
        GithubApiHeaders.of(link, retryAfter, rateLimitRemaining, rateLimitReset, singleSignOn),
        bodyExceeded,
    )

internal fun failed(origin: String = "java.net.SocketTimeoutException"): GithubApiReply =
    GithubApiReply.Failed(origin)

internal fun testApp(installationUrl: String = INSTALLATION_URL): GithubApp? =
    GithubApp.of(CLIENT_ID, installationUrl)

internal fun testAuthorization(
    accountId: String = ACCOUNT_ID,
    login: String = ACCOUNT_LOGIN,
    installations: Int = 1,
    selection: RepositorySelection = RepositorySelection.Selected,
    accessToken: String = ACCESS_TOKEN,
): GithubAuthorization =
    GithubAuthorization(
        VerifiedAccount(accountId, login),
        InstallationAccess(installations, selection),
        StoredCredential(accessToken = accessToken),
    )

internal fun testProvider(
    transport: GithubApiTransport,
    authorization: GithubAuthorization = testAuthorization(),
    app: GithubApp? = testApp(),
    live: () -> Boolean = { true },
): GithubProvider = GithubProvider(authorization, app, transport, live)

internal fun testInstallation(
    id: String = INSTALLATION_ID,
    accountId: String = ACCOUNT_ID,
    login: String = ACCOUNT_LOGIN,
    selection: RepositorySelection = RepositorySelection.Selected,
): GithubInstallation = GithubInstallation(id, accountId, login, selection)

internal fun testRepository(
    id: String = REPOSITORY_ID,
    installationId: String = INSTALLATION_ID,
    ownerAccountId: String = ACCOUNT_ID,
    owner: String = OWNER,
    name: String = REPOSITORY,
    isPrivate: Boolean = true,
    defaultBranch: String? = BRANCH,
): GithubRepository =
    GithubRepository(id, installationId, ownerAccountId, owner, name, isPrivate, defaultBranch)

internal fun testBranch(name: String = BRANCH, commitSha: String = COMMIT_SHA): GithubBranch =
    GithubBranch(name, commitSha, false)

internal fun installationsBody(vararg entries: String): String =
    """{"total_count":${entries.size},"installations":[${entries.joinToString(",")}]}"""

internal fun installationEntry(
    id: String = INSTALLATION_ID,
    accountId: String = ACCOUNT_ID,
    login: String = ACCOUNT_LOGIN,
    selection: String = "selected",
): String =
    """{"id":$id,"account":{"id":$accountId,"login":"$login"},""" +
        """"repository_selection":"$selection"}"""

internal fun repositoriesBody(vararg entries: String): String =
    """{"total_count":${entries.size},"repositories":[${entries.joinToString(",")}]}"""

internal fun repositoryEntry(
    id: String = REPOSITORY_ID,
    name: String = REPOSITORY,
    owner: String = OWNER,
    ownerAccountId: String = ACCOUNT_ID,
    isPrivate: Boolean = true,
    defaultBranch: String = BRANCH,
): String =
    """{"id":$id,"name":"$name","owner":{"id":$ownerAccountId,"login":"$owner"},""" +
        """"private":$isPrivate,"default_branch":"$defaultBranch"}"""

internal fun branchesBody(vararg entries: String): String = "[${entries.joinToString(",")}]"

internal fun branchEntry(
    name: String = BRANCH,
    commitSha: String = COMMIT_SHA,
    isProtected: Boolean = false,
): String = """{"name":"$name","commit":{"sha":"$commitSha"},"protected":$isProtected}"""

internal fun treeBody(vararg entries: String, truncated: Boolean = false): String =
    """{"truncated":$truncated,"tree":[${entries.joinToString(",")}]}"""

internal fun treeEntry(
    path: String,
    type: String = "tree",
    sha: String = FOLDER_TREE_SHA,
    mode: String = "040000",
): String = """{"path":"$path","mode":"$mode","type":"$type","sha":"$sha"}"""

internal fun installationsUrl(): String = "$API/user/installations?per_page=100"

internal fun repositoriesUrl(installationId: String = INSTALLATION_ID): String =
    "$API/user/installations/$installationId/repositories?per_page=100"

internal fun branchesUrl(owner: String = OWNER, repository: String = REPOSITORY): String =
    "$API/repos/$owner/$repository/branches?per_page=100"

internal fun branchUrl(
    branch: String = BRANCH,
    owner: String = OWNER,
    repository: String = REPOSITORY,
): String = "$API/repos/$owner/$repository/branches/$branch"

internal fun treeUrl(
    sha: String = COMMIT_SHA,
    owner: String = OWNER,
    repository: String = REPOSITORY,
): String = "$API/repos/$owner/$repository/git/trees/$sha"

internal fun nextLink(url: String): String = """<$url>; rel="next", <$url>; rel="last""""

internal fun <T> GithubOutcome<T>.read(): T =
    when (this) {
        is GithubOutcome.Read -> value
        is GithubOutcome.Refused -> throw AssertionError("expected a value, got $refusal")
    }

internal fun <T> GithubOutcome<T>.refusal(): GithubRefusal =
    when (this) {
        is GithubOutcome.Read -> throw AssertionError("expected a refusal, got $value")
        is GithubOutcome.Refused -> refusal
    }

internal const val API = "https://api.github.com"

internal const val CLIENT_ID = "Iv23liSlipboxTestClient"

internal const val INSTALLATION_URL =
    "https://github.com/apps/slipbox-test/installations/new"

internal const val ACCESS_TOKEN = "synthetic-access-token-0123456789abcdef"

internal const val ACCOUNT_ID = "52000001"

internal const val ACCOUNT_LOGIN = "synthetic-account"

internal const val INSTALLATION_ID = "77000002"

internal const val REPOSITORY_ID = "88000003"

internal const val OWNER = "synthetic-owner"

internal const val REPOSITORY = "synthetic-notes"

internal const val BRANCH = "master"

internal const val COMMIT_SHA = "1f0a2b3c4d5e6f708192a3b4c5d6e7f809a1b2c3"

internal const val FOLDER_TREE_SHA = "2e1b3c4d5e6f708192a3b4c5d6e7f809a1b2c3d4"

internal const val NESTED_TREE_SHA = "3d2c4b5a6978e0f1a2b3c4d5e6f70819a2b3c4d5"

internal const val WAIT_MILLIS = 10_000L
