package com.bitfun.mobile.core.domain

public data class RecentWorkspace public constructor(
    public val path: String,
    public val name: String,
    public val lastOpened: String,
    public val kind: String,
)

public data class WorkspaceAssistant public constructor(
    public val path: String,
    public val name: String,
    public val assistantId: String?,
)

public data class SelectedWorkspace public constructor(
    public val path: String,
    public val name: String,
    public val gitBranch: String,
    public val kind: String,
    public val assistantId: String?,
)
