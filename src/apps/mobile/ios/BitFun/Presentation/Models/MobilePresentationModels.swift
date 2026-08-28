import Foundation

enum ConnectionPhase {
    case connected
    case reconnecting
    case disconnected
}

enum MobileSurface: String {
    case local
    case remote
}

struct ChatMessage: Identifiable, Equatable {
    let id: UUID
    let role: Role
    let text: String

    enum Role { case user, assistant }
}

struct MobileTimelineImage: Identifiable, Equatable {
    var id: String { dataURL }
    let name: String
    let dataURL: String
}

struct MobileTimelineOption: Identifiable, Equatable {
    let label: String
    let description: String?
    var id: String { label }
}

struct MobileTimelineQuestion: Identifiable, Equatable {
    let index: Int
    let header: String
    let question: String
    let options: [MobileTimelineOption]
    let multiSelect: Bool
    var id: Int { index }
}

struct MobileTimelineTool: Identifiable, Equatable {
    let id: String
    let name: String
    let phase: String
    let kind: String
    let operation: String
    let target: String
    let filePath: String
    let fileLabel: String
    let input: String
    let output: String
    let question: String?
    let questions: [MobileTimelineQuestion]
    let actions: Set<String>
}

indirect enum MobileTimelineBlock: Identifiable, Equatable {
    case text(id: String, text: String, streaming: Bool)
    case thinking(id: String, text: String, streaming: Bool)
    case tools(id: String, tools: [MobileTimelineTool])
    case subagent(
        id: String,
        title: String,
        running: Bool,
        text: String,
        children: [MobileTimelineBlock]
    )

    var id: String {
        switch self {
        case let .text(id, _, _), let .thinking(id, _, _), let .tools(id, _),
             let .subagent(id, _, _, _, _):
            return id
        }
    }
}

struct MobileConversationRow: Identifiable, Equatable {
    let id: String
    let kind: String
    let text: String
    let thinking: String?
    let images: [MobileTimelineImage]
    let tools: [MobileTimelineTool]
    let blocks: [MobileTimelineBlock]
    let streaming: Bool
    let typing: Bool
    let pending: Bool
    let showRetry: Bool
}

enum MobileFilePreviewFailureKind: String {
    case notFound, unavailable, accessDenied, tooLarge, connection, loadFailed
}

struct MobileFilePreview: Identifiable, Equatable {
    let id: String
    let sessionID: String
    let controlTargetEpoch: Int32
    let name: String
    let content: String
    let mimeType: String
    let imageData: Data?
    let truncated: Bool
    let loadedBytes: Int64
    let sizeBytes: Int64
    let markdown: Bool
    let lineStart: Int32
    let failure: String?
    let failureKind: MobileFilePreviewFailureKind?
    let retryable: Bool
    let unsupported: Bool

    init(
        id: String,
        sessionID: String = "",
        controlTargetEpoch: Int32 = 0,
        name: String,
        content: String,
        mimeType: String,
        imageData: Data?,
        truncated: Bool,
        loadedBytes: Int64 = 0,
        sizeBytes: Int64 = 0,
        markdown: Bool = false,
        lineStart: Int32 = 0,
        failure: String?,
        failureKind: MobileFilePreviewFailureKind? = nil,
        retryable: Bool = false,
        unsupported: Bool = false
    ) {
        self.id = id
        self.sessionID = sessionID
        self.controlTargetEpoch = controlTargetEpoch
        self.name = name
        self.content = content
        self.mimeType = mimeType
        self.imageData = imageData
        self.truncated = truncated
        self.loadedBytes = loadedBytes
        self.sizeBytes = sizeBytes
        self.markdown = markdown
        self.lineStart = lineStart
        self.failure = failure
        self.failureKind = failureKind
        self.retryable = retryable
        self.unsupported = unsupported
    }
}

struct MobilePendingDownload: Identifiable, Equatable {
    var id: String { reference }
    let reference: String
    let remotePath: String
    let name: String
    let mimeType: String
    let data: Data
    let sessionID: String
    let controlTargetEpoch: Int32

    init(reference: String, remotePath: String, name: String, mimeType: String, data: Data,
         sessionID: String = "", controlTargetEpoch: Int32 = 0) {
        self.reference = reference
        self.remotePath = remotePath
        self.name = name
        self.mimeType = mimeType
        self.data = data
        self.sessionID = sessionID
        self.controlTargetEpoch = controlTargetEpoch
    }
}

struct ChatSession: Identifiable, Equatable {
    let id: String
    var title: String
    var updatedLabel: String
    var pinned: Bool = false
    var status: String = "active"
    var agentType: String = "general_chat"
    var workspacePath: String?
    var workspaceName: String?
    var deviceKey: String? = nil
    var createdAt: String = ""
    var messageCount: Int = 0
}

struct CommittedRemoteCreate {
    let targetKey: String
    let epoch: UInt64
    let session: ChatSession
    /// First authoritative Ready revision guaranteed to contain this commit.
    let minimumAuthorityRevision: Int64
}

struct PendingDirectoryRemoteDraft {
    let targetKey: String
    let rawDeviceKey: String
    let workspacePath: String
    let normalizedWorkspacePath: String
    let epoch: UInt64
    var selectionRequested: Bool
}

struct MobileAccountDevice: Identifiable, Equatable {
    let id: String
    let name: String
    let online: Bool
    let selected: Bool
}

struct MobileDeviceDirectoryEntry: Identifiable, Equatable {
    let id: String
    let name: String
    let online: Bool
    let expanded: Bool
    let status: String
    let error: String?
    let workspaces: [MobileWorkspaceGroup]
    let sessions: [ChatSession]
}

struct MobileWorkspaceGroup: Identifiable, Equatable {
    var id: String { (deviceKey ?? "") + ":" + path }
    let path: String
    let name: String
    let selected: Bool
    let sessions: [ChatSession]
    var deviceKey: String? = nil
}

enum MobileSessionListSectionKind: Equatable {
    case chat
    case project
    case today
    case yesterday
    case earlier
}

struct MobileSessionListSectionProjection: Identifiable {
    let id: String
    let kind: MobileSessionListSectionKind
    let path: String
    let name: String
    let sessions: [ChatSession]
}

struct MobileSessionWorkspaceOption: Identifiable {
    var id: String { path }
    let path: String
    let name: String
}

struct MobileAssistantOption: Identifiable, Equatable {
    var id: String { path }
    let path: String
    let name: String
}

struct ComposerAttachment: Identifiable, Equatable {
    let id: String
    let data: Data
    let mimeType: String

    var dataURL: String {
        "data:\(mimeType);base64,\(data.base64EncodedString())"
    }
}

struct ComposerModelOption: Identifiable, Equatable {
    let id: String
    let primaryLabel: String
    let secondaryLabel: String
    let source: String
    let selected: Bool
}

enum MobileDownloadPhase {
    case idle
    case preparing
    case downloading
    case saving
    case saved
    case failed
}
