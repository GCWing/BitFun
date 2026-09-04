package com.openbitfun.mobile.app.state

import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import com.openbitfun.mobile.core.feature.session.SessionAgentGroup
import com.openbitfun.mobile.core.feature.session.SessionGroupMode
import com.openbitfun.mobile.core.feature.session.SessionListOptions

/**
 * How the user wants the session list drawn, ported from the state
 * `RemoteSurfaceHost.ets` keeps for `ConversationViewSettings`.
 *
 * It is view state, not session state: nothing here reaches the desktop, and
 * losing it on sign-out costs nothing. It survives rotation via [Saver] and
 * nothing more, which is also all the source keeps.
 */
internal data class SessionViewSettings(
    val groupMode: SessionGroupMode,
    val workspaceFilter: String,
    val agentFilter: SessionAgentGroup?,
    val statusFilter: String,
    val showWorkspace: Boolean,
    val showUpdated: Boolean,
    val showStatus: Boolean,
) {
    fun options(query: String): SessionListOptions = SessionListOptions(
        groupMode = groupMode,
        query = query,
        workspaceFilter = workspaceFilter,
        agentFilter = agentFilter,
        statusFilter = statusFilter,
    )

    companion object {
        /** Same defaults as the source: grouped by project, nothing filtered, no metadata. */
        val Default: SessionViewSettings = SessionViewSettings(
            groupMode = SessionGroupMode.PROJECT,
            workspaceFilter = "",
            agentFilter = null,
            statusFilter = "",
            showWorkspace = false,
            showUpdated = false,
            showStatus = false,
        )

        // Enums are not saveable, so both go across as their names — stable
        // identifiers, unlike ordinals, if a case is ever inserted.
        val Saver: Saver<SessionViewSettings, Any> = listSaver(
            save = {
                listOf(
                    it.groupMode.name,
                    it.workspaceFilter,
                    it.agentFilter?.name.orEmpty(),
                    it.statusFilter,
                    it.showWorkspace,
                    it.showUpdated,
                    it.showStatus,
                )
            },
            restore = {
                SessionViewSettings(
                    groupMode = SessionGroupMode.valueOf(it[0] as String),
                    workspaceFilter = it[1] as String,
                    agentFilter = (it[2] as String).takeIf { name -> name.isNotEmpty() }
                        ?.let { name -> SessionAgentGroup.valueOf(name) },
                    statusFilter = it[3] as String,
                    showWorkspace = it[4] as Boolean,
                    showUpdated = it[5] as Boolean,
                    showStatus = it[6] as Boolean,
                )
            },
        )
    }
}
