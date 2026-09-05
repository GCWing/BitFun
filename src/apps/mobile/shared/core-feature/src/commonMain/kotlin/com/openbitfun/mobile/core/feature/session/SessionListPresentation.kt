package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.domain.RecentWorkspace
import com.openbitfun.mobile.core.domain.RemoteSession
import com.openbitfun.mobile.core.domain.SessionAgentTypes
import com.openbitfun.mobile.core.domain.SessionWorkspacePaths
import kotlinx.datetime.TimeZone
import kotlinx.datetime.toLocalDateTime
import kotlin.time.Instant

/** How the session list is carved up, from `viewSettings.grouping`. */
public enum class SessionGroupMode {
    /** Projects first, then the chat sessions that belong to no project. */
    PROJECT,

    /** One flat list, newest first, split into today / yesterday / earlier. */
    TIME,

    /** Like [PROJECT] with the chat group lifted to the top. */
    CHAT,
}

/** Which of the three kinds of session a row is, from `agentGroup`. */
public enum class SessionAgentGroup { CHAT, CODE, COWORK }

/**
 * The statuses worth a word of our own.
 *
 * Everything else is [RAW]: the desktop invents status strings faster than
 * either client can translate them, and showing one verbatim is more honest
 * than mapping it onto the nearest word we happen to have.
 */
public enum class SessionStatusLabel { RUNNING, READY, ARCHIVED, RAW }

/** A workspace the list can be filtered down to. */
public data class SessionWorkspaceOption public constructor(
    public val path: String,
    public val name: String,
)

/**
 * Where the list is being shown from.
 *
 * The selected workspace is not just another entry: it is the fallback for
 * sessions the desktop sent no `workspacePath` for, so a session created before
 * the desktop started reporting paths still lands in the project the user is
 * looking at.
 */
public data class SessionWorkspaceContext public constructor(
    public val selectedPath: String,
    public val selectedName: String,
    public val selectedKind: String,
    public val recent: List<RecentWorkspace>,
)

/**
 * The view settings, as one value.
 *
 * A record rather than five parameters because these travel together — the
 * sheet edits them as a unit and every query below needs all of them. An empty
 * string means "no filter" for the two free-text filters, matching the source.
 */
public data class SessionListOptions public constructor(
    public val groupMode: SessionGroupMode,
    public val query: String,
    public val workspaceFilter: String,
    public val agentFilter: SessionAgentGroup?,
    public val statusFilter: String,
)

/** One headed group of rows. */
public sealed interface SessionListSection {
    public val sessions: List<RemoteSession>

    public data class Chat public constructor(
        override val sessions: List<RemoteSession>,
    ) : SessionListSection

    public data class Project public constructor(
        public val path: String,
        public val name: String,
        override val sessions: List<RemoteSession>,
    ) : SessionListSection

    public data class Today public constructor(
        override val sessions: List<RemoteSession>,
    ) : SessionListSection

    public data class Yesterday public constructor(
        override val sessions: List<RemoteSession>,
    ) : SessionListSection

    public data class Earlier public constructor(
        override val sessions: List<RemoteSession>,
    ) : SessionListSection
}

/**
 * The list as it should be drawn.
 *
 * [filtered] separates the two empty states: "this desktop has no sessions" and
 * "your filters hid them all" need different words and different buttons, and
 * only the caller of [SessionListPresentation.view] knows which it is.
 */
public data class SessionListView public constructor(
    public val sections: List<SessionListSection>,
    public val filtered: Boolean,
)

/** One section after the incremental three-row disclosure is applied. */
public data class SessionListBatch public constructor(
    public val visible: List<RemoteSession>,
    public val remaining: Int,
    public val nextCount: Int,
)

/**
 * Port of the grouping and filtering in `pages/components/RemoteSessionList.ets`
 * and the option lists in `pages/components/ConversationViewSettings.ets`,
 * together with `pages/policy/ConversationSessionFilterPolicy.ets`.
 *
 * It sits in `core-feature` rather than `core-domain` for the usual reason: the
 * app has to `when` over [SessionListSection] and [SessionAgentGroup] to draw a
 * header, and the architecture guardrail keeps `core-domain` off the app's
 * import list. The rules that are only about strings — path normalisation, which
 * `agent_type` values mean "chat" — stay in `core-domain`.
 *
 * Two deliberate deviations from the source:
 *
 * 1. Workspace identity is compared with [SessionWorkspacePaths.equal]
 *    everywhere, including when deciding whether a workspace is an assistant
 *    one. The ArkTS version compares those two raw, which means a desktop that
 *    reports the same assistant workspace with a trailing slash in one command
 *    and without it in another groups its sessions under the wrong heading.
 * 2. Sections are returned whole and [batch] applies the incremental disclosure
 *    separately. Group identity and filtering stay stable while each platform
 *    owns the transient count of how many batches the user has opened.
 */
public object SessionListPresentation {
    /** Groups [sessions] the way [options] asks for; [nowMs] dates the buckets. */
    public fun view(
        sessions: List<RemoteSession>,
        workspace: SessionWorkspaceContext,
        options: SessionListOptions,
        nowMs: Long,
    ): SessionListView {
        val filtered = sessions.filter { matches(it, workspace, options) }
        val active = options.hasActiveFilter()
        val (chat, project) = filtered.partition { isAssistant(it, workspace) }
        val sections = when (options.groupMode) {
            SessionGroupMode.TIME -> timeSections(filtered, nowMs)
            SessionGroupMode.CHAT ->
                chatSections(chat) + projectSections(project, workspace, active)
            SessionGroupMode.PROJECT ->
                projectSections(project, workspace, active) + chatSections(chat)
        }
        return SessionListView(sections = sections, filtered = active)
    }

    /**
     * Every workspace the filter could name: the selected one, the desktop's
     * recents, and any workspace only a session knows about.
     */
    public fun workspaceOptions(
        sessions: List<RemoteSession>,
        workspace: SessionWorkspaceContext,
    ): List<SessionWorkspaceOption> {
        val result = mutableListOf<SessionWorkspaceOption>()
        fun add(path: String, name: String) {
            if (path.isEmpty()) return
            if (result.any { SessionWorkspacePaths.equal(it.path, path) }) return
            result += SessionWorkspaceOption(
                path = path,
                name = name.ifBlank { SessionWorkspacePaths.basename(path) },
            )
        }
        add(workspace.selectedPath, workspace.selectedName)
        workspace.recent.forEach { add(it.path, it.name) }
        sessions.forEach { add(it.workspacePath.orEmpty(), it.workspaceName.orEmpty()) }
        return result
    }

    /** The agent groups actually present, in the source's fixed order. */
    public fun agentGroups(
        sessions: List<RemoteSession>,
        workspace: SessionWorkspaceContext,
    ): List<SessionAgentGroup> {
        val present = sessions.map { agentGroup(it, workspace) }.toSet()
        return listOf(
            SessionAgentGroup.CHAT,
            SessionAgentGroup.CODE,
            SessionAgentGroup.COWORK,
        ).filter { it in present }
    }

    /** The statuses actually present, lowercased and sorted. */
    public fun statusOptions(sessions: List<RemoteSession>): List<String> =
        sessions.map { it.status.trim().lowercase() }
            .filter { it.isNotEmpty() }
            .distinct()
            .sorted()

    /** Reveals three rows initially and three more for every completed step. */
    public fun batch(sessions: List<RemoteSession>, revealedSteps: Int): SessionListBatch {
        val limit = INITIAL_VISIBLE + revealedSteps.coerceAtLeast(0) * REVEAL_STEP
        val visible = sessions.take(limit)
        val remaining = (sessions.size - visible.size).coerceAtLeast(0)
        return SessionListBatch(
            visible = visible,
            remaining = remaining,
            nextCount = minOf(REVEAL_STEP, remaining),
        )
    }

    /** Which of our words, if any, names [status]. */
    public fun statusLabel(status: String): SessionStatusLabel =
        when (status.trim().lowercase()) {
            "active", "running" -> SessionStatusLabel.RUNNING
            "ready", "idle" -> SessionStatusLabel.READY
            ARCHIVED -> SessionStatusLabel.ARCHIVED
            else -> SessionStatusLabel.RAW
        }

    /** Which group a single row belongs to; exposed because the row shows it. */
    public fun agentGroup(
        session: RemoteSession,
        workspace: SessionWorkspaceContext,
    ): SessionAgentGroup = when {
        isAssistant(session, workspace) -> SessionAgentGroup.CHAT
        SessionAgentTypes.isCowork(session.agentType) -> SessionAgentGroup.COWORK
        else -> SessionAgentGroup.CODE
    }

    private fun SessionListOptions.hasActiveFilter(): Boolean =
        query.trim().isNotEmpty() ||
            workspaceFilter.isNotEmpty() ||
            agentFilter != null ||
            statusFilter.isNotEmpty()

    private fun matches(
        session: RemoteSession,
        workspace: SessionWorkspaceContext,
        options: SessionListOptions,
    ): Boolean {
        // An archived session is not hidden by a filter — it is not part of the
        // list at all, the way the source drops it before anything else runs.
        if (session.id.isEmpty() || session.status == ARCHIVED) return false

        val query = options.query.trim().lowercase()
        if (query.isNotEmpty() && !session.title.lowercase().contains(query)) return false

        val assistant = isAssistant(session, workspace)
        if (options.workspaceFilter.isNotEmpty() &&
            !SessionWorkspacePaths.equal(sessionPath(session, workspace, assistant), options.workspaceFilter)
        ) {
            return false
        }

        if (options.agentFilter != null && agentGroup(session, workspace) != options.agentFilter) {
            return false
        }

        val status = session.status.trim().lowercase()
        return options.statusFilter.isEmpty() || status == options.statusFilter
    }

    /**
     * A chat session has no project of its own, so it must not inherit the
     * selected workspace the way a code session does.
     */
    private fun sessionPath(
        session: RemoteSession,
        workspace: SessionWorkspaceContext,
        assistant: Boolean,
    ): String {
        val own = session.workspacePath.orEmpty()
        if (own.isNotEmpty()) return own
        return if (assistant) "" else workspace.selectedPath
    }

    private fun isAssistant(session: RemoteSession, workspace: SessionWorkspaceContext): Boolean =
        SessionAgentTypes.isAssistant(session.agentType) ||
            isAssistantWorkspace(session.workspacePath.orEmpty(), workspace)

    private fun isAssistantWorkspace(path: String, workspace: SessionWorkspaceContext): Boolean {
        val selectedIsAssistant = workspace.selectedKind.lowercase() == ASSISTANT
        // No path at all means "wherever we are", which is the selected one.
        if (path.isEmpty()) return selectedIsAssistant
        if (selectedIsAssistant && SessionWorkspacePaths.equal(path, workspace.selectedPath)) return true
        return workspace.recent.any {
            SessionWorkspacePaths.equal(it.path, path) && it.kind.lowercase() == ASSISTANT
        }
    }

    private fun chatSections(chat: List<RemoteSession>): List<SessionListSection> =
        if (chat.isEmpty()) emptyList() else listOf(SessionListSection.Chat(chat))

    private fun projectSections(
        project: List<RemoteSession>,
        workspace: SessionWorkspaceContext,
        activeFilter: Boolean,
    ): List<SessionListSection> {
        val entries = projectEntries(workspace)
        return entries.mapNotNull { entry ->
            val rows = project.filter { session ->
                SessionWorkspacePaths.equal(
                    session.workspacePath.orEmpty().ifEmpty { workspace.selectedPath },
                    entry.path,
                )
            }
            // With no filter on, an empty project still gets a heading: it is
            // how the user creates the first session in it. With one on, an
            // empty heading is just noise the filter was meant to remove.
            if (activeFilter && rows.isEmpty()) {
                null
            } else {
                SessionListSection.Project(path = entry.path, name = entry.name, sessions = rows)
            }
        }
    }

    private fun projectEntries(workspace: SessionWorkspaceContext): List<SessionWorkspaceOption> {
        val result = mutableListOf<SessionWorkspaceOption>()
        fun add(path: String, name: String, kind: String) {
            if (kind.lowercase() == ASSISTANT) return
            if (path.isEmpty() && name.isEmpty()) return
            if (result.any { SessionWorkspacePaths.equal(it.path, path) }) return
            result += SessionWorkspaceOption(
                path = path,
                name = name.ifBlank { SessionWorkspacePaths.basename(path) },
            )
        }
        add(workspace.selectedPath, workspace.selectedName, workspace.selectedKind)
        workspace.recent.forEach { if (it.path.isNotEmpty()) add(it.path, it.name, it.kind) }
        return result
    }

    private fun timeSections(sessions: List<RemoteSession>, nowMs: Long): List<SessionListSection> {
        val ordered = sessions.sortedByDescending { timestamp(it) }
        val today = mutableListOf<RemoteSession>()
        val yesterday = mutableListOf<RemoteSession>()
        val earlier = mutableListOf<RemoteSession>()
        val zone = TimeZone.currentSystemDefault()
        val nowDate = Instant.fromEpochMilliseconds(nowMs).toLocalDateTime(zone).date
        ordered.forEach { session ->
            val stamp = timestamp(session)
            val bucket = if (stamp <= 0L) {
                earlier
            } else {
                val date = Instant.fromEpochMilliseconds(stamp).toLocalDateTime(zone).date
                when (nowDate.toEpochDays() - date.toEpochDays()) {
                    0L -> today
                    1L -> yesterday
                    else -> earlier
                }
            }
            bucket += session
        }
        return listOfNotNull(
            today.takeIf { it.isNotEmpty() }?.let { SessionListSection.Today(it) },
            yesterday.takeIf { it.isNotEmpty() }?.let { SessionListSection.Yesterday(it) },
            earlier.takeIf { it.isNotEmpty() }?.let { SessionListSection.Earlier(it) },
        )
    }

    /** `updatedAt` if the desktop sent a readable one, else `createdAt`, else 0. */
    private fun timestamp(session: RemoteSession): Long {
        val updated = SessionTimePresentation.timestampMs(session.updatedAt)
        if (updated != null && updated > 0L) return updated
        val created = SessionTimePresentation.timestampMs(session.createdAt)
        return if (created != null && created > 0L) created else 0L
    }

    private const val ARCHIVED: String = "archived"
    private const val ASSISTANT: String = "assistant"
    private const val INITIAL_VISIBLE: Int = 3
    private const val REVEAL_STEP: Int = 3
}
