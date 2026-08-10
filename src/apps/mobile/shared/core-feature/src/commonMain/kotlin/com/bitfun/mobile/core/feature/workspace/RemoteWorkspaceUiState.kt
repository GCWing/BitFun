package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.domain.FilePreviewFailure
import com.bitfun.mobile.core.domain.FilePreviewFailureReason
import com.bitfun.mobile.core.domain.FilePreviewTarget
import com.bitfun.mobile.core.domain.RecentWorkspace
import com.bitfun.mobile.core.domain.SelectedWorkspace
import com.bitfun.mobile.core.domain.WorkspaceAssistant

public sealed interface RemoteFilePreviewUiState {
    public data object None : RemoteFilePreviewUiState
    public data class Loading public constructor(public val target: FilePreviewTarget) : RemoteFilePreviewUiState
    public data class Text public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
        public val content: String,
        public val truncated: Boolean,
    ) : RemoteFilePreviewUiState
    public data class Image public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
        public val mimeType: String,
        public val bytes: ByteArray,
    ) : RemoteFilePreviewUiState
    public data class Unsupported public constructor(
        public val target: FilePreviewTarget,
        public val mimeType: String,
    ) : RemoteFilePreviewUiState
    /**
     * @param retryable whether asking again could give a different answer. A
     * file outside the workspace will not appear on a second try, so offering
     * Retry there would be a lie.
     */
    public data class Failed public constructor(
        public val target: FilePreviewTarget,
        public val kind: FilePreviewFailureKind,
        public val retryable: Boolean,
    ) : RemoteFilePreviewUiState
}

/**
 * Why a preview has no content.
 *
 * The desktop's own sentence is not carried across: it is written in the
 * desktop's locale and often names a host path. Apps say it in their own words —
 * see the design doc section 4.3.
 */
public enum class FilePreviewFailureKind {
    NOT_FOUND,
    UNAVAILABLE,
    ACCESS_DENIED,
    TOO_LARGE,
    CONNECTION,
    LOAD_FAILED,
}

internal fun FilePreviewFailure.toKind(): FilePreviewFailureKind = when (reason) {
    FilePreviewFailureReason.NOT_FOUND -> FilePreviewFailureKind.NOT_FOUND
    FilePreviewFailureReason.UNAVAILABLE -> FilePreviewFailureKind.UNAVAILABLE
    FilePreviewFailureReason.ACCESS_DENIED -> FilePreviewFailureKind.ACCESS_DENIED
    FilePreviewFailureReason.TOO_LARGE -> FilePreviewFailureKind.TOO_LARGE
    FilePreviewFailureReason.CONNECTION -> FilePreviewFailureKind.CONNECTION
    FilePreviewFailureReason.LOAD_FAILED -> FilePreviewFailureKind.LOAD_FAILED
}

public sealed interface RemoteWorkspaceUiState {
    public data object Idle : RemoteWorkspaceUiState
    public data object Loading : RemoteWorkspaceUiState
    public data class Ready public constructor(
        public val workspaces: List<RecentWorkspace>,
        public val assistants: List<WorkspaceAssistant>,
        public val selected: SelectedWorkspace?,
        public val preview: RemoteFilePreviewUiState,
        public val busy: Boolean,
    ) : RemoteWorkspaceUiState
    public data class Failed public constructor(public val retryable: Boolean) : RemoteWorkspaceUiState
}

public sealed interface RemoteWorkspaceIntent {
    public data object Load : RemoteWorkspaceIntent
    public data class SelectWorkspace public constructor(public val path: String) : RemoteWorkspaceIntent
    public data class SelectAssistant public constructor(public val path: String) : RemoteWorkspaceIntent
    public data class OpenFile public constructor(
        public val reference: String,
        public val label: String,
        public val sessionId: String,
    ) : RemoteWorkspaceIntent
    public data object DismissPreview : RemoteWorkspaceIntent
    public data object Stop : RemoteWorkspaceIntent
}
