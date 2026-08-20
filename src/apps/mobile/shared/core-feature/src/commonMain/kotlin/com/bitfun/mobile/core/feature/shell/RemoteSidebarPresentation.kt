package com.bitfun.mobile.core.feature.shell

import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

/** One remote session with only the facts the unified sidebar renders. */
public data class RemoteSidebarSessionRow public constructor(
    public val id: String,
    public val title: String,
    public val agentType: String,
)

/** One remote workspace and the sessions filed under it in the sidebar tree. */
public data class RemoteSidebarWorkspaceRow public constructor(
    public val path: String,
    public val name: String,
    public val selected: Boolean,
    public val sessions: List<RemoteSidebarSessionRow>,
)

/** Platform-neutral projection for HarmonyOS' device/workspace/session hierarchy. */
public object RemoteSidebarPresentation {
    public fun workspaces(
        workspaceState: RemoteWorkspaceUiState.Ready?,
        sessionState: RemoteSessionUiState.Ready?,
    ): List<RemoteSidebarWorkspaceRow> {
        if (workspaceState == null) return emptyList()
        val selected = workspaceState.selected
        val workspaceRows = buildList {
            if (selected != null && selected.path.isNotBlank()) {
                add(selected.path to selected.name)
            }
            workspaceState.workspaces.forEach { workspace ->
                if (workspace.path.isNotBlank() && none { it.first == workspace.path }) {
                    add(workspace.path to workspace.name)
                }
            }
        }
        return workspaceRows.map { (path, name) ->
            RemoteSidebarWorkspaceRow(
                path = path,
                name = name,
                selected = path == selected?.path,
                sessions = sessionState?.sessions.orEmpty()
                    .filter { (it.workspacePath ?: selected?.path) == path }
                    .map { session ->
                        RemoteSidebarSessionRow(
                            id = session.id,
                            title = session.title,
                            agentType = session.agentType,
                        )
                    },
            )
        }
    }
}
