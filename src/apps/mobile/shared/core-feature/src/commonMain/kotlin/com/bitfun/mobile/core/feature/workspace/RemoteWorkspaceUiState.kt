package com.bitfun.mobile.core.feature.workspace

import com.bitfun.mobile.core.domain.FilePreviewFailure
import com.bitfun.mobile.core.domain.FilePreviewFailureReason
import com.bitfun.mobile.core.domain.FilePreviewTarget
import com.bitfun.mobile.core.domain.RecentWorkspace
import com.bitfun.mobile.core.domain.SelectedWorkspace
import com.bitfun.mobile.core.domain.WorkspaceAssistant

public data class PreviewRequestIdentity public constructor(
    public val requestId: String,
    public val deviceKey: String?,
    public val sessionId: String,
    public val path: String,
)

public sealed interface RemoteFilePreviewUiState {
    public data object None : RemoteFilePreviewUiState
    public data class Loading public constructor(
        public val target: FilePreviewTarget,
        public val identity: PreviewRequestIdentity,
    ) : RemoteFilePreviewUiState {
        public constructor(target: FilePreviewTarget) : this(target, PreviewRequestIdentity("", null, target.sessionId, target.remotePath))
    }
    public data class Text public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
        public val content: String,
        public val truncated: Boolean,
        public val loadedBytes: Long,
        public val mimeType: String,
        public val sizeBytes: Long,
        /** Markdown is rendered rather than shown as numbered source. */
        public val markdown: Boolean,
        public val identity: PreviewRequestIdentity,
    ) : RemoteFilePreviewUiState {
        public constructor(
            target: FilePreviewTarget, name: String, content: String, truncated: Boolean,
            loadedBytes: Long, mimeType: String, sizeBytes: Long, markdown: Boolean,
        ) : this(target, name, content, truncated, loadedBytes, mimeType, sizeBytes, markdown,
            PreviewRequestIdentity("", null, target.sessionId, target.remotePath))
    }
    public data class Image public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
        public val mimeType: String,
        public val bytes: ByteArray,
        public val sizeBytes: Long,
        public val identity: PreviewRequestIdentity,
    ) : RemoteFilePreviewUiState {
        public constructor(target: FilePreviewTarget, name: String, mimeType: String, bytes: ByteArray, sizeBytes: Long) :
            this(target, name, mimeType, bytes, sizeBytes, PreviewRequestIdentity("", null, target.sessionId, target.remotePath))
    }
    public data class Unsupported public constructor(
        public val target: FilePreviewTarget,
        public val mimeType: String,
        public val sizeBytes: Long,
        public val identity: PreviewRequestIdentity,
    ) : RemoteFilePreviewUiState {
        public constructor(target: FilePreviewTarget, mimeType: String, sizeBytes: Long) :
            this(target, mimeType, sizeBytes, PreviewRequestIdentity("", null, target.sessionId, target.remotePath))
    }
    /**
     * @param retryable whether asking again could give a different answer. A
     * file outside the workspace will not appear on a second try, so offering
     * Retry there would be a lie.
     */
    public data class Failed public constructor(
        public val target: FilePreviewTarget,
        public val kind: FilePreviewFailureKind,
        public val retryable: Boolean,
        public val mimeType: String,
        public val sizeBytes: Long,
        public val identity: PreviewRequestIdentity,
    ) : RemoteFilePreviewUiState {
        public constructor(target: FilePreviewTarget, kind: FilePreviewFailureKind, retryable: Boolean, mimeType: String, sizeBytes: Long) :
            this(target, kind, retryable, mimeType, sizeBytes, PreviewRequestIdentity("", null, target.sessionId, target.remotePath))
    }
}

/**
 * Byte counts as the preview surface says them, matching HarmonyOS'
 * `RemoteUiState.formatBytes` exactly: whole units, and never a unit smaller
 * than the number deserves. A header line that disagreed across the two apps
 * would be a parity difference no screenshot could explain away.
 */
public object FilePreviewFormat {
    private const val KB: Long = 1024
    private const val MB: Long = 1024 * 1024

    public fun bytes(value: Long): String = when {
        value < KB -> "$value B"
        value < MB -> "${((value.toDouble() / KB) + 0.5).toLong()} KB"
        else -> "${((value.toDouble() / MB) + 0.5).toLong()} MB"
    }
}

public sealed interface RemoteFileDownloadUiState {
    public data object None : RemoteFileDownloadUiState
    public data class Loading public constructor(
        public val target: FilePreviewTarget,
        public val downloadedBytes: Long,
        public val totalBytes: Long,
    ) : RemoteFileDownloadUiState
    public data class AwaitingSave public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
        public val mimeType: String,
        public val bytes: ByteArray,
    ) : RemoteFileDownloadUiState
    public data class Saved public constructor(
        public val target: FilePreviewTarget,
        public val name: String,
    ) : RemoteFileDownloadUiState
    public data class Failed public constructor(
        public val target: FilePreviewTarget,
        public val kind: FilePreviewFailureKind,
        public val retryable: Boolean,
    ) : RemoteFileDownloadUiState
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
        public val download: RemoteFileDownloadUiState,
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
        /** Optional local correlation supplied by Swift; blank values are generated. */
        public val requestId: String,
    ) : RemoteWorkspaceIntent {
        public constructor(reference: String, label: String, sessionId: String) :
            this(reference, label, sessionId, "")
    }
    public data class DownloadFile public constructor(
        public val reference: String,
        public val label: String,
        public val sessionId: String,
    ) : RemoteWorkspaceIntent
    public data class DownloadSaved public constructor(public val reference: String) : RemoteWorkspaceIntent
    public data class DownloadSaveFailed public constructor(public val reference: String) : RemoteWorkspaceIntent
    public data object DismissPreview : RemoteWorkspaceIntent
    public data object Stop : RemoteWorkspaceIntent
}
