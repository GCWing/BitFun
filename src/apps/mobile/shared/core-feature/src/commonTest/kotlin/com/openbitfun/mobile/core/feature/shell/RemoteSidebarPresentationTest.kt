package com.openbitfun.mobile.core.feature.shell

import com.openbitfun.mobile.core.domain.RecentWorkspace
import com.openbitfun.mobile.core.domain.RemoteSession
import com.openbitfun.mobile.core.domain.SelectedWorkspace
import com.openbitfun.mobile.core.feature.session.RemoteSessionUiState
import com.openbitfun.mobile.core.feature.session.SessionAgentFilter
import com.openbitfun.mobile.core.feature.workspace.RemoteFilePreviewUiState
import com.openbitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RemoteSidebarPresentationTest {
    @Test
    fun selectedWorkspaceComesFirstWithoutDuplicatingItsRecentEntry() {
        val rows = RemoteSidebarPresentation.workspaces(
            workspaceState = workspaceReady(
                selected = selected("/repo/b", "Beta"),
                recent = listOf(
                    recent("/repo/a", "Alpha"),
                    recent("/repo/b", "Old Beta"),
                ),
            ),
            sessionState = sessionReady(
                listOf(
                    session("b-1", "/repo/b", "code"),
                    session("a-1", "/repo/a", "chat"),
                ),
            ),
        )

        assertEquals(listOf("/repo/b", "/repo/a"), rows.map { it.path })
        assertEquals("Beta", rows.first().name)
        assertTrue(rows.first().selected)
        assertEquals(listOf("b-1"), rows.first().sessions.map { it.id })
        assertEquals(listOf("a-1"), rows.last().sessions.map { it.id })
    }

    @Test
    fun sessionWithoutAWorkspacePathBelongsToTheSelectedWorkspace() {
        val rows = RemoteSidebarPresentation.workspaces(
            workspaceState = workspaceReady(selected("/repo/current", "Current"), emptyList()),
            sessionState = sessionReady(listOf(session("legacy", null, "assistant"))),
        )

        assertEquals(listOf("legacy"), rows.single().sessions.map { it.id })
        assertEquals("assistant", rows.single().sessions.single().agentType)
    }

    private fun workspaceReady(
        selected: SelectedWorkspace,
        recent: List<RecentWorkspace>,
    ) = RemoteWorkspaceUiState.Ready(
        workspaces = recent,
        assistants = emptyList(),
        selected = selected,
        preview = RemoteFilePreviewUiState.None,
        busy = false,
        download = com.openbitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState.None,
    )

    private fun sessionReady(sessions: List<RemoteSession>) = RemoteSessionUiState.Ready(
        sessions = sessions,
        selectedSessionId = null,
        timeline = null,
        busy = false,
        permissionMode = null,
        permissionModeFailure = null,
        query = "",
        agentFilter = SessionAgentFilter.ALL,
        hasMore = false,
        hasMoreMessages = false,
        modelCatalog = null,
    )

    private fun selected(path: String, name: String) = SelectedWorkspace(path, name, "main", "git", null)

    private fun recent(path: String, name: String) = RecentWorkspace(path, name, "", "git")

    private fun session(id: String, workspacePath: String?, agentType: String) = RemoteSession(
        id = id,
        title = id,
        agentType = agentType,
        status = "idle",
        updatedAt = "",
        createdAt = "",
        messageCount = 0,
        workspacePath = workspacePath,
        workspaceName = null,
    )
}
