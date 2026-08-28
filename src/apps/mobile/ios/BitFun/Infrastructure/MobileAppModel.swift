import Foundation
import SwiftUI
import BitFunMobileCore

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

@MainActor
final class MobileAppModel: ObservableObject {
    @Published var appLanguage: MobileLanguage = MobileLocalization.restoredLanguage()
    @Published var surface: MobileSurface = .local
    @Published var sessions: [ChatSession]
    @Published var remoteSessions: [ChatSession] = []
    @Published var remoteQuery = ""
    @Published var remoteAgentFilter = "ALL"
    @Published var remoteViewAgentFilter = ""
    @Published var remoteGroupMode = "PROJECT"
    @Published var remoteWorkspaceFilter = ""
    @Published var remoteStatusFilter = ""
    @Published var remoteShowWorkspaceMetadata = false
    @Published var remoteShowUpdatedMetadata = false
    @Published var remoteShowStatusMetadata = false
    @Published var remoteViewSettingsOpen = false
    @Published var remoteHasMore = false
    @Published var remoteHasMoreMessages = false
    @Published var remotePermissionMode = "ASK"
    @Published var remotePermissionFailure: String?
    @Published var remoteAssistants: [MobileAssistantOption] = []
    @Published var remoteCreateOpen = false
    @Published var remoteCreateSubmitting = false
    @Published var remoteCreateError: String?
    @Published var remoteCreateDeviceError: String?
    @Published var generalConfigOpen = false
    @Published var generalConfigured = false
    @Published var generalConfigBaseURL = ""
    @Published var generalConfigModel = ""
    @Published var generalConfigHasAPIKey = false
    @Published var generalConfigFailure: String?
    @Published var generalConnectionTestRunning = false
    @Published var generalConnectionTestMessage: String?
    @Published var generalExportOpen = false
    @Published var generalExportName = "conversation.md"
    @Published var generalExportData = Data()
    @Published var selectedSessionID: String
    @Published var messages: [ChatMessage]
    @Published var timelineRows: [MobileConversationRow] = []
    @Published var draft = ""
    @Published var drawerOpen = false
    @Published var settingsOpen = false
    @Published var remoteControlSettingsOpen = false
    @Published var accountSheetOpen = false
    @Published var languagePickerOpen = false
    @Published var connectionPhase: ConnectionPhase = .connected
    @Published var isSending = false
    @Published var busy = false
    @Published var composerImages: [ComposerAttachment] = []
    @Published var modelOptions: [ComposerModelOption] = []
    @Published var toastMessage: String?
    @Published var remoteConnected = false
    @Published var remoteSessionSelected = false
    @Published var localSessionSelected = false
    @Published var pairingSheetOpen = false
    @Published var pairingScanRequested = false
    @Published var pairingBusy = false
    @Published var pairingError: String?
    @Published var coreErrorMessage: String?
    @Published var accountUser: String?
    @Published var accountUserID: String?
    @Published var localDeviceID = ""
    @Published var accountBusy = false
    @Published var accountFailureStage: String?
    @Published var accountFailureCanRetry = false
    @Published var accountDeviceName: String?
    @Published var directPairingDeviceName: String?
    @Published var accountDeviceCount = 0
    @Published var accountDevices: [MobileAccountDevice] = []
    @Published var accountSelectedDeviceID: String?
    @Published var accountRefreshing = false
    @Published var deviceDirectory: [MobileDeviceDirectoryEntry] = []
    @Published var directPairingDirectoryEntry: MobileDeviceDirectoryEntry?
    @Published var remoteWorkspaces: [MobileWorkspaceGroup] = []
    @Published var workspaceLoading = false
    @Published var workspaceLoadFailed = false
    @Published var filePreview: MobileFilePreview?
    @Published var sessionDetails: ChatSession? = nil
    @Published var filePreviewLoading = false
    @Published var pendingDownload: MobilePendingDownload?
    @Published var downloadExporterOpen = false
    @Published var downloadTargetPath: String?
    @Published var downloadStatusText: String?
    @Published var downloadPhase: MobileDownloadPhase = .idle
    var activeTurnID: String?
    var directPairingConnected = false
    var accountLoginPreview = false
    var localActionPreview = false
    var composerModelPickerPreview = false
    var remoteCreatePreview = false
    var directoryFixturePreview = false
    var pairingGeneration: UInt64 = 0
    var accountGeneration: UInt64 = 0
    var pendingAccountOperationPreservesPairing: (generation: UInt64, preserve: Bool)?
    var pairingIntentInFlight = false
    var remoteTargetEpoch: UInt64 = 0
    var remoteExpectedDeviceKey: String?
    var remoteBoundTargetKey: String?
    var remoteBoundTargetEpoch: UInt64?
    var pairingRetainedAccountAuthority: RetainedAccountAuthority?
    var accountDirectoryGeneration: UInt64 = 0
    var pendingDirectorySession: (deviceKey: String, sessionID: String, epoch: UInt64)?
    var remoteInitialSessionReady = false
    var remoteInitialWorkspaceReady = false
    var remoteCreateRequestID: String?
    var remoteCreateRequestEpoch: UInt64 = 0
    var remoteCreateRequestDeviceKey: String?
    var committedRemoteCreate: CommittedRemoteCreate?
    var remoteLastAppliedAuthority: RemoteAuthorityScope?
    var workspaceCatalog: [(path: String, name: String, selected: Bool)] = []
    var pendingRemoteWorkspaceCreate: (path: String, agentType: String)?
    var pendingDirectoryWorkspace: (deviceKey: String, path: String, epoch: UInt64)?
    var pendingDirectoryRemoteDraft: PendingDirectoryRemoteDraft?
    var pendingRemoteAssistantCreate = false
    var selectedRemoteWorkspaceKind = ""

    var coreAdapter: MobileCoreAdapter?

    init(sessions: [ChatSession], selectedSessionID: String, messages: [ChatMessage]) {
        self.sessions = sessions
        self.selectedSessionID = selectedSessionID
        self.messages = messages
        self.timelineRows = messages.map(Self.simpleTimelineRow)
        self.coreAdapter = nil
        let adapter = MobileCoreAdapter(
            onState: { [weak self] state in self?.apply(coreState: state) },
            onPairingState: { [weak self] state, generation in
                self?.apply(pairingState: state, generation: generation)
            },
            onAccountState: { [weak self] state, generation in
                self?.apply(accountState: state, generation: generation)
            },
            onRemoteTargetBound: { [weak self] targetKey, epoch, generation in
                self?.apply(remoteTargetBound: targetKey, epoch: epoch, accountGeneration: generation)
            },
            onRemoteState: { [weak self] state, targetKey, epoch in
                self?.apply(remoteState: state, targetKey: targetKey, epoch: epoch)
            },
            onWorkspaceState: { [weak self] state, targetKey, epoch in
                self?.apply(workspaceState: state, targetKey: targetKey, epoch: epoch)
            },
            onDirectoryState: { [weak self] state, generation in
                self?.apply(directoryState: state, generation: generation)
            },
            onCreateOperation: { [weak self] state, targetKey in
                self?.apply(createOperation: state, targetKey: targetKey)
            },
            onCreateUnavailable: { [weak self] requestID, targetKey in
                self?.failRemoteCreate(requestID: requestID, targetKey: targetKey)
            }
        )
        self.coreAdapter = adapter
        self.localDeviceID = adapter.deviceID
    }

    static let preview: MobileAppModel = {
        let first = ChatSession(id: UUID().uuidString, title: "你好", updatedLabel: "刚刚")
        return MobileAppModel(
            sessions: [first],
            selectedSessionID: first.id,
            messages: [
                ChatMessage(id: UUID(), role: .user, text: "你好"),
                ChatMessage(id: UUID(), role: .assistant, text: "这是 BitFun 的移动端会话界面。你可以从手机连接桌面端，查看工作区、会话和 Agent 的执行状态。")
            ]
        )
    }()

    static var launchConfigured: MobileAppModel {
        let model = preview
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("--english") {
            model.setLanguage(.english)
        } else if arguments.contains("--simplified-chinese") {
            model.setLanguage(.simplifiedChinese)
        }
        if arguments.contains("--remote") {
            model.surface = .remote
        }
        if arguments.contains("--connected") {
            model.configureConnectedPreview()
        }
        if arguments.contains("--remote-chat-section") {
            if !model.remoteConnected { model.configureConnectedPreview() }
            model.remoteSessions.append(
                ChatSession(
                    id: "preview-remote-chat",
                    title: "移动端体验对齐",
                    updatedLabel: "刚刚",
                    status: "idle",
                    agentType: "Claw",
                    workspacePath: nil,
                    workspaceName: nil
                )
            )
            model.rebuildRemoteWorkspaceGroups()
        }
        if arguments.contains("--remote-view-settings") {
            if !model.remoteConnected { model.configureConnectedPreview() }
            model.remoteViewSettingsOpen = true
        }
        if arguments.contains("--remote-view-density") {
            if !model.remoteConnected { model.configureConnectedPreview() }
            let now = ISO8601DateFormatter().string(from: Date())
            for index in model.remoteSessions.indices {
                model.remoteSessions[index].updatedLabel = now
            }
            model.remoteGroupMode = "TIME"
            model.remoteShowWorkspaceMetadata = true
            model.remoteShowUpdatedMetadata = true
            model.remoteShowStatusMetadata = true
            model.rebuildRemoteWorkspaceGroups()
        }
        if arguments.contains("--timeline-preview") {
            model.configureTimelinePreview()
        }
        if arguments.contains("--file-preview") {
            model.filePreview = MobileFilePreview(
                id: "src/main.rs",
                name: "main.rs",
                content: "// Remote workspace preview\nfn main() {\n    println!(\"Hello from BitFun\");\n}\n",
                mimeType: "text/x-rust",
                imageData: nil,
                truncated: false,
                failure: nil
            )
        }
        if arguments.contains("--download-preview") {
            model.pendingDownload = MobilePendingDownload(
                reference: "computer://src/main.rs",
                remotePath: "src/main.rs",
                name: "main.rs",
                mimeType: "text/x-rust",
                data: Data("fn main() {}\n".utf8)
            )
            model.downloadTargetPath = "src/main.rs"
            model.downloadPhase = .saving
            model.downloadStatusText = model.localized("正在保存")
            model.downloadExporterOpen = true
        }
        if let relay = arguments.value(after: "--relay-url"),
           let username = arguments.value(after: "--username"),
           let password = arguments.value(after: "--password") {
            model.loginAccount(relayURL: relay, username: username, password: password)
        }
        if arguments.contains("--drawer") {
            model.drawerOpen = true
        }
        if arguments.contains("--settings") {
            model.settingsOpen = true
        }
        if arguments.contains("--remote-settings") {
            model.surface = .remote
            model.remoteControlSettingsOpen = true
        }
        if arguments.contains("--model-settings") {
            model.settingsOpen = true
            model.generalConfigOpen = true
        }
        if arguments.contains("--composer-model-picker") ||
            ProcessInfo.processInfo.environment["BITFUN_COMPOSER_MODEL_PICKER"] == "1" {
            model.composerModelPickerPreview = true
            model.localSessionSelected = true
            model.draft = "\n"
            model.modelOptions = [
                ComposerModelOption(
                    id: "preview-codex",
                    primaryLabel: "GPT-5.6 Codex",
                    secondaryLabel: "BitFun 账号",
                    source: "ACCOUNT",
                    selected: true
                ),
                ComposerModelOption(
                    id: "preview-local",
                    primaryLabel: "本机自定义模型",
                    secondaryLabel: "OpenAI 兼容服务",
                    source: "LOCAL",
                    selected: false
                ),
            ]
        }
        if arguments.contains("--pairing") || arguments.contains("--pairing-manual") ||
            arguments.contains("--pairing-account") {
            model.pairingSheetOpen = true
        }
        if arguments.contains("--remote-create") || arguments.contains("--remote-create-workspace-picker") {
            model.remoteCreatePreview = true
            if !model.remoteConnected { model.configureConnectedPreview() }
            model.remoteCreateOpen = true
        }
        if arguments.contains("--remote-home-preview") {
            model.remoteSessionSelected = false
            model.selectedSessionID = ""
            model.timelineRows = []
            model.messages = []
        }
        if arguments.contains("--local-actions") {
            model.localActionPreview = true
            model.surface = .local
            model.localSessionSelected = true
            model.remoteSessionSelected = false
            if let localSession = model.sessions.first {
                model.selectedSessionID = localSession.id
            }
        }
        if arguments.contains("--account-login") {
            model.accountLoginPreview = true
            model.accountUser = nil
            model.accountDeviceName = nil
            model.accountSelectedDeviceID = nil
            model.accountDevices = []
            model.accountDeviceCount = 0
            model.coreErrorMessage = nil
            model.settingsOpen = false
            model.accountSheetOpen = true
        }
        if arguments.contains("--account-profile") {
            model.accountLoginPreview = true
            model.accountUser = "bitfun-user"
            model.accountUserID = "user-preview-7A31"
            model.accountDevices = [
                MobileAccountDevice(
                    id: "desktop-preview",
                    name: "Studio Mac",
                    online: true,
                    selected: true
                ),
                MobileAccountDevice(
                    id: "desktop-offline-preview",
                    name: "Office PC",
                    online: false,
                    selected: false
                ),
            ]
            model.accountDeviceName = "Studio Mac"
            model.accountSelectedDeviceID = "desktop-preview"
            model.accountDeviceCount = model.accountDevices.count
            model.coreErrorMessage = nil
            model.settingsOpen = false
            model.accountSheetOpen = true
        }
        return model
    }

    var selectedSession: ChatSession? {
        guard (surface == .local && localSessionSelected) || (surface == .remote && remoteSessionSelected) else {
            return nil
        }
        return visibleSessions.first { $0.id == selectedSessionID }
    }

    var visibleSessions: [ChatSession] {
        surface == .local ? sessions : remoteSessions
    }



    func switchSurface(_ next: MobileSurface) {
        surface = next
        drawerOpen = false
    }

    func setLanguage(_ language: MobileLanguage) {
        UserDefaults.standard.set(language.rawValue, forKey: MobileLocalization.preferenceKey)
        guard appLanguage != language else {
            languagePickerOpen = false
            return
        }
        appLanguage = language
        languagePickerOpen = false
    }

    func localized(_ key: String) -> String {
        MobileLocalization.text(key, language: appLanguage)
    }

    func localizedFormat(_ key: String, _ arguments: CVarArg...) -> String {
        String(
            format: localized(key),
            locale: Locale(identifier: appLanguage.rawValue),
            arguments: arguments
        )
    }

    func connectRemote() {
        pairingError = nil
        pairingScanRequested = false
        pairingSheetOpen = true
    }

    func scanRemote() {
        pairingError = nil
        pairingScanRequested = true
        pairingSheetOpen = true
    }

    func consumePairingScanRequest() {
        pairingScanRequested = false
    }

    func openAccountFromPairing() {
        pairingSheetOpen = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            self.accountSheetOpen = true
        }
    }

    var usesDirectPairing: Bool { directPairingConnected }

    var directPairingSidebarDeviceID: String { "qr:\(directPairingDeviceName ?? "desktop")" }

    func dismissPairing() {
        pairingError = nil
        coreAdapter?.dismissPairingFailure()
    }

    func handleScenePhase(_ phase: ScenePhase) {
        switch phase {
        case .active: coreAdapter?.pairingForeground()
        case .background: coreAdapter?.pairingBackground()
        default: break
        }
    }

    func verifyRemoteConnection() {
        guard accountUser == nil else {
            refreshRemoteDevices()
            return
        }
        connectionPhase = .reconnecting
        coreAdapter?.verifyPairing()
    }

    func disconnectRemote() {
        invalidateTargetScopedFileTransfers()
        committedRemoteCreate = nil
        remoteLastAppliedAuthority = nil
        coreAdapter?.disconnect()
        directPairingConnected = false
        directPairingDeviceName = nil
        pendingAccountOperationPreservesPairing = nil
        remoteConnected = false
        remoteSessionSelected = false
        remoteSessions = []
        remoteWorkspaces = []
        workspaceCatalog = []
        pendingRemoteWorkspaceCreate = nil
        pendingDirectoryRemoteDraft = nil
        pendingRemoteAssistantCreate = false
        selectedRemoteWorkspaceKind = ""
        selectedSessionID = ""
        timelineRows = []
        messages = []
        surface = .local
        connectionPhase = .connected
    }

    func openRemoteSurface() {
        surface = .remote
        drawerOpen = false
    }

























    func showSessionDetails(_ session: ChatSession) {
        sessionDetails = session
    }

    func dismissSessionDetails() {
        sessionDetails = nil
    }


    private func configureConnectedPreview() {
        directPairingConnected = true
        surface = .remote
        remoteConnected = true
        connectionPhase = .connected
        remoteSessionSelected = true
        accountUser = "preview@bitfun"
        accountDeviceName = "DESKTOP-KM3L4UI"
        accountSelectedDeviceID = "preview-desktop"
        directoryFixturePreview = true
        accountDevices = [
            MobileAccountDevice(id: "preview-desktop", name: "DESKTOP-KM3L4UI", online: true, selected: true),
            MobileAccountDevice(id: "preview-mac", name: "Studio Mac", online: true, selected: false),
            MobileAccountDevice(id: "preview-offline", name: "Office PC", online: false, selected: false)
        ]
        accountDeviceCount = accountDevices.count
        let session = ChatSession(
            id: UUID().uuidString,
            title: "你好",
            updatedLabel: "刚刚",
            agentType: "code",
            workspacePath: "/workspace/BitFun",
            workspaceName: "BitFun"
        )
        remoteSessions = [session]
        let extraSessions = (1...5).map { index in
            ChatSession(
                id: "preview-session-\(index)", title: "Review session \(index)", updatedLabel: "2026-01-01T00:00:00Z",
                status: index == 1 ? "running" : "idle", agentType: "code",
                workspacePath: "/workspace/BitFun", workspaceName: "BitFun", deviceKey: "preview-desktop"
            )
        }
        remoteSessions.append(contentsOf: extraSessions)
        let cachedSession = ChatSession(
            id: "preview-offline-session", title: "Cached offline session", updatedLabel: "2026-01-01T00:00:00Z",
            status: "idle", agentType: "code", workspacePath: "/office/project", workspaceName: "Office project", deviceKey: "preview-offline"
        )
        let failedSession = ChatSession(
            id: "preview-failed-session", title: "Cached failed session", updatedLabel: "2026-01-01T00:00:00Z",
            status: "idle", agentType: "code", workspacePath: "/staging/project", workspaceName: "Staging", deviceKey: "preview-mac"
        )
        remoteSessions.append(contentsOf: [cachedSession, failedSession])
        let previewWorkspace = MobileWorkspaceGroup(path: "/workspace/BitFun", name: "BitFun", selected: true, sessions: remoteSessions.filter { $0.deviceKey == "preview-desktop" }, deviceKey: "preview-desktop")
        let offlineWorkspace = MobileWorkspaceGroup(path: "/office/project", name: "Office project", selected: false, sessions: [cachedSession], deviceKey: "preview-offline")
        let failedWorkspace = MobileWorkspaceGroup(path: "/staging/project", name: "Staging", selected: false, sessions: [failedSession], deviceKey: "preview-mac")
        deviceDirectory = [
            MobileDeviceDirectoryEntry(id: "preview-desktop", name: "DESKTOP-KM3L4UI", online: true, expanded: true, status: "READY", error: nil, workspaces: [previewWorkspace], sessions: previewWorkspace.sessions),
            MobileDeviceDirectoryEntry(id: "preview-mac", name: "Studio Mac", online: true, expanded: true, status: "FAILED", error: "REMOTE_UNAVAILABLE", workspaces: [failedWorkspace], sessions: [failedSession]),
            MobileDeviceDirectoryEntry(id: "preview-offline", name: "Office PC", online: false, expanded: false, status: "READY", error: nil, workspaces: [offlineWorkspace], sessions: [cachedSession])
        ]
        workspaceCatalog = [(path: "/workspace/BitFun", name: "BitFun", selected: true)]
        remoteAssistants = [
            MobileAssistantOption(path: "/workspace/BitFun/.bitfun/assistants/review", name: "代码审查助手")
        ]
        remoteHasMore = true
        rebuildRemoteWorkspaceGroups()
        selectedSessionID = session.id
        messages = [
            ChatMessage(id: UUID(), role: .user, text: "你好"),
            ChatMessage(id: UUID(), role: .assistant, text: "这是 BitFun 的远程会话预览。"),
        ]
        timelineRows = messages.map(Self.simpleTimelineRow)
    }

    private func configureTimelinePreview() {
        configureConnectedPreview()
        let userID = UUID().uuidString
        let assistantID = UUID().uuidString
        let readOne = MobileTimelineTool(
            id: "preview-read-1", name: "Read", phase: "COMPLETED", kind: "DOCUMENT",
            operation: "READ_FILE", target: "main.rs", filePath: "computer://src/main.rs",
            fileLabel: "main.rs", input: "src/main.rs", output: "读取完成", question: nil, questions: [], actions: []
        )
        let readTwo = MobileTimelineTool(
            id: "preview-read-2", name: "Search", phase: "COMPLETED", kind: "SEARCH",
            operation: "SEARCH_CODE", target: "MobileShellView", filePath: "", fileLabel: "",
            input: "MobileShellView", output: "找到 4 处结果", question: nil, questions: [], actions: []
        )
        let approval = MobileTimelineTool(
            id: "preview-approval", name: "Bash", phase: "PENDING_CONFIRMATION", kind: "COMMAND",
            operation: "RUN_COMMAND", target: "pnpm test", filePath: "", fileLabel: "",
            input: "pnpm test", output: "", question: nil, questions: [], actions: ["APPROVE", "REJECT"]
        )
        let question = MobileTimelineTool(
            id: "preview-question", name: "AskUserQuestion", phase: "PENDING_CONFIRMATION", kind: "QUESTION",
            operation: "ASK_CONFIRMATION", target: "", filePath: "", fileLabel: "", input: "", output: "",
            question: "要同时运行远程场景回归吗？", questions: [], actions: ["ANSWER"]
        )
        timelineRows = [
            MobileConversationRow(
                id: userID, kind: "USER", text: "请检查移动端的消息、工具和文件交互。", thinking: nil,
                images: [], tools: [], blocks: [], streaming: false, typing: false, pending: false, showRetry: false
            ),
            MobileConversationRow(
                id: assistantID, kind: "ASSISTANT", text: "", thinking: nil, images: [], tools: [],
                blocks: [
                    .thinking(id: "preview-thinking", text: "先对照 HarmonyOS 的消息顺序与工具状态，再核对 Android 的交互策略。", streaming: false),
                    .text(
                        id: "preview-text",
                        text: "## 检查结果\n\n消息按共享投影顺序显示，文件可直接打开：[main.rs](computer://src/main.rs)。\n\n- Markdown 与代码块\n- 思考过程与子任务\n- 工具确认、提问和取消\n\n```swift\nlet parity = true\n```",
                        streaming: false
                    ),
                    .tools(id: "preview-tools", tools: [readOne, readTwo, approval, question]),
                ],
                streaming: false, typing: false, pending: false, showRetry: false
            ),
        ]
        messages = [
            ChatMessage(id: UUID(), role: .user, text: "请检查移动端的消息、工具和文件交互。"),
            ChatMessage(id: UUID(), role: .assistant, text: "检查结果"),
        ]
    }

    func submitPairing(url: String) {
        prepareProjectionForPairingSubmission()
        pairingIntentInFlight = true
        pairingGeneration &+= 1
        pairingError = nil
        pairingBusy = true
        coreAdapter?.submitPairing(url: url)
    }

    func submitPairing(url: String, userID: String, password: String) {
        prepareProjectionForPairingSubmission()
        pairingIntentInFlight = true
        pairingGeneration &+= 1
        pairingError = nil
        pairingBusy = true
        coreAdapter?.submitPairing(url: url, userID: userID, password: password)
    }

    private func prepareProjectionForPairingSubmission() {
        let adapterTargetKey = coreAdapter?.currentRemoteTargetKey
        let adapterEpoch = coreAdapter?.currentRemoteTargetEpoch ?? 0
        let healthyConnected: Bool
        switch connectionPhase {
        case .connected: healthyConnected = remoteConnected
        case .reconnecting, .disconnected: healthyConnected = false
        }
        if let adapterTargetKey,
           adapterTargetKey.hasPrefix("account:"),
           adapterTargetKey == remoteExpectedDeviceKey,
           adapterEpoch == remoteTargetEpoch,
           adapterTargetKey == remoteBoundTargetKey,
           adapterEpoch == remoteBoundTargetEpoch,
           healthyConnected {
            pairingRetainedAccountAuthority = RetainedAccountAuthority(
                targetKey: adapterTargetKey,
                epoch: adapterEpoch
            )
        } else {
            pairingRetainedAccountAuthority = nil
        }
        let transition = RemoteAuthorityGate.pairingAttemptTransition(
            authoritativeTargetKey: adapterTargetKey,
            remoteConnected: remoteConnected
        )
        guard transition.clearBoundRemoteProjection else { return }

        invalidateTargetScopedFileTransfers()
        directPairingConnected = false
        directPairingDeviceName = nil
        directPairingDirectoryEntry = nil
        remoteConnected = transition.remoteConnected
        remoteExpectedDeviceKey = nil
        remoteLastAppliedAuthority = nil
        committedRemoteCreate = nil
        pendingAccountOperationPreservesPairing = nil
        remoteInitialSessionReady = false
        remoteInitialWorkspaceReady = false
        remoteSessionSelected = false
        remoteSessions = []
        remoteWorkspaces = []
        remoteAssistants = []
        remotePermissionFailure = nil
        sessionDetails = nil
        workspaceCatalog = []
        workspaceLoading = false
        workspaceLoadFailed = false
        pendingDirectorySession = nil
        pendingDirectoryWorkspace = nil
        pendingDirectoryRemoteDraft = nil
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false
        selectedRemoteWorkspaceKind = ""
        selectedSessionID = ""
        remoteCreateOpen = false
        remoteCreateSubmitting = false
        remoteCreateRequestID = nil
        remoteCreateRequestEpoch = remoteTargetEpoch
        remoteCreateRequestDeviceKey = nil
        remoteCreateError = nil
        remoteCreateDeviceError = nil
        activeTurnID = nil
        isSending = false
        busy = false
        composerImages = []
        timelineRows = []
        messages = []
        connectionPhase = .reconnecting
    }






    func stopSending() {
        if surface == .remote {
            guard remoteSessionSelected else { return }
            coreAdapter?.cancelRemoteTurn(sessionID: selectedSessionID, turnID: activeTurnID)
        } else {
            coreAdapter?.cancelGeneralChat()
        }
    }






    func retryMessage(_ text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty, !busy, !isSending else { return }
        if surface == .remote {
            guard remoteSessionSelected, connectionPhase != .disconnected else { return }
            isSending = true
            busy = true
            coreAdapter?.sendRemote(sessionID: selectedSessionID, content: normalized, images: [])
        } else {
            draft = normalized
            send()
        }
    }






    func renameSelectedSession(_ title: String) {
        let normalized = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty, selectedSession != nil else { return }
        if surface == .remote {
            coreAdapter?.renameRemoteSession(sessionID: selectedSessionID, title: normalized)
        } else {
            coreAdapter?.renameGeneralSession(sessionID: selectedSessionID, title: normalized)
        }
    }

    func togglePinSelectedSession() {
        guard surface == .local, let session = selectedSession else { return }
        coreAdapter?.pinGeneralSession(sessionID: session.id, pinned: !session.pinned)
    }

    func archiveSelectedSession() {
        guard surface == .local, let session = selectedSession else { return }
        coreAdapter?.archiveGeneralSession(
            sessionID: session.id,
            archived: session.status.lowercased() != "archived"
        )
    }

    func deleteSelectedSession() {
        guard surface == .local, let session = selectedSession else { return }
        coreAdapter?.deleteGeneralSession(sessionID: session.id)
        localSessionSelected = false
    }


    func showUploadedFiles() {
        let count = composerImages.count
        showToast(
            count == 0
                ? localized("当前会话暂无已上传文件")
                : localizedFormat("当前会话已上传 %lld 个文件", Int64(count))
        )
    }

    func showToast(_ message: String) {
        toastMessage = message
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            guard self?.toastMessage == message else { return }
            self?.toastMessage = nil
        }
    }


    private func apply(pairingState state: PairingUiState, generation: UInt64) {
        guard !localActionPreview, generation == pairingGeneration,
              remoteExpectedDeviceKey == nil || remoteExpectedDeviceKey == "pairing" || pairingIntentInFlight else { return }
        pairingBusy = state is PairingUiStateConnecting
        if let failed = state as? PairingUiStateFailed {
            pairingBusy = false
            pairingIntentInFlight = false
            pairingError = pairingErrorMessage(failed.failure)
            let healthyConnected: Bool
            switch connectionPhase {
            case .connected: healthyConnected = remoteConnected
            case .reconnecting, .disconnected: healthyConnected = false
            }
            let retainAccount = RemoteAuthorityGate.shouldRetainAccountAfterPairingFailure(
                captured: pairingRetainedAccountAuthority,
                adapterTargetKey: coreAdapter?.currentRemoteTargetKey,
                adapterEpoch: coreAdapter?.currentRemoteTargetEpoch ?? 0,
                modelTargetKey: remoteExpectedDeviceKey,
                modelEpoch: remoteTargetEpoch,
                healthyConnected: healthyConnected
            )
            let invalidatedAccountAuthority = !retainAccount &&
                (remoteExpectedDeviceKey?.hasPrefix("account:") == true)
            if invalidatedAccountAuthority, let targetKey = remoteExpectedDeviceKey {
                invalidateTargetScopedFileTransfers()
                _ = coreAdapter?.invalidateRemoteAuthority(
                    ifTargetKey: targetKey,
                    epoch: remoteTargetEpoch
                )
                clearInvalidatedRemoteAuthorityProjection(
                    adapterEpoch: coreAdapter?.currentRemoteTargetEpoch ?? remoteTargetEpoch
                )
            } else {
                pairingRetainedAccountAuthority = nil
            }
            remoteConnected = retainAccount
            if !retainAccount {
                let clearingVisibleRemoteConversation = surface == .remote || remoteSessionSelected
                remoteSessionSelected = false
                if clearingVisibleRemoteConversation {
                    selectedSessionID = ""
                    activeTurnID = nil
                    isSending = false
                    busy = false
                    timelineRows = []
                    messages = []
                }
                connectionPhase = .disconnected
            }
        } else if let paired = state as? PairingUiStatePaired {
            pairingBusy = false
            pairingError = nil
            directPairingConnected = true
            pairingIntentInFlight = false
            pairingRetainedAccountAuthority = nil
            remoteConnected = true
            directPairingDeviceName = paired.workspace.roomLabel
            if pendingDirectoryRemoteDraft?.targetKey == "pairing",
               pendingDirectoryRemoteDraft?.rawDeviceKey != directPairingSidebarDeviceID {
                pendingDirectoryRemoteDraft = nil
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            }
            directPairingDirectoryEntry = MobileDeviceDirectoryEntry(
                id: directPairingSidebarDeviceID,
                name: paired.workspace.roomLabel,
                online: true,
                expanded: true,
                status: "READY",
                error: nil,
                workspaces: remoteWorkspaces,
                sessions: remoteSessions
            )
            surface = .remote
            switch paired.liveness {
            case .checking: connectionPhase = .reconnecting
            case .lost: connectionPhase = .disconnected
            default: connectionPhase = .connected
            }
            pairingSheetOpen = false
        }
    }












    private func pairingErrorMessage(_ failure: PairingFailure) -> String {
        if let remote = failure.remoteMessage?.trimmingCharacters(in: .whitespacesAndNewlines), !remote.isEmpty {
            return remote
        }
        switch failure.reason.name {
        case "PAIRING_LINK_EMPTY", "PAIRING_LINK_INCOMPLETE", "PAIRING_LINK_UNDECODABLE", "PAIRING_LINK_KEY_UNUSABLE":
            return localized("连接链接无效，请重新扫描或粘贴桌面端链接")
        case "ACCOUNT_USERNAME_REQUIRED":
            return localized("请输入桌面端账号")
        case "ACCOUNT_PASSWORD_REQUIRED":
            return localized("请输入桌面端密码")
        case "REJECTED", "DESKTOP_REJECTED":
            return localized("桌面端拒绝了这次连接")
        case "ROOM_NOT_FOUND":
            return localized("找不到桌面端房间，请确认桌面端仍在等待连接")
        case "RATE_LIMITED", "TOO_MANY_ATTEMPTS":
            return localized("尝试次数过多，请稍后再试")
        case "RELAY_UNAVAILABLE", "NETWORK_UNREACHABLE":
            return localized("网络不可用，请检查手机与桌面端的网络")
        case "TIMEOUT":
            return localized("连接超时，请重新尝试")
        case "PROTOCOL_MISMATCH":
            return localized("桌面端版本不兼容，请升级后重试")
        default:
            return localized("连接失败，请检查桌面端链接")
        }
    }

}


private extension Array where Element == String {
    func value(after flag: String) -> String? {
        guard let position = firstIndex(of: flag), position < self.index(before: endIndex) else { return nil }
        return self[self.index(after: position)]
    }
}
