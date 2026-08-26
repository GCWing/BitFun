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

struct MobileFilePreview: Identifiable, Equatable {
    let id: String
    let name: String
    let content: String
    let mimeType: String
    let imageData: Data?
    let truncated: Bool
    let failure: String?
}

struct MobilePendingDownload: Identifiable, Equatable {
    var id: String { reference }
    let reference: String
    let remotePath: String
    let name: String
    let mimeType: String
    let data: Data
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
    var createdAt: String = ""
    var messageCount: Int = 0
}

struct MobileAccountDevice: Identifiable, Equatable {
    let id: String
    let name: String
    let online: Bool
    let selected: Bool
}

struct MobileWorkspaceGroup: Identifiable, Equatable {
    var id: String { path }
    let path: String
    let name: String
    let selected: Bool
    let sessions: [ChatSession]
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
    @Published var accountDeviceName: String?
    @Published var directPairingDeviceName: String?
    @Published var accountDeviceCount = 0
    @Published var accountDevices: [MobileAccountDevice] = []
    @Published var accountSelectedDeviceID: String?
    @Published var accountRefreshing = false
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
    private var activeTurnID: String?
    private var directPairingConnected = false
    private var accountLoginPreview = false
    private var localActionPreview = false
    var composerModelPickerPreview = false
    private var workspaceCatalog: [(path: String, name: String, selected: Bool)] = []
    private var pendingRemoteWorkspaceCreate: (path: String, agentType: String)?
    private var pendingRemoteAssistantCreate = false
    private var selectedRemoteWorkspaceKind = ""

    private var coreAdapter: MobileCoreAdapter?

    init(sessions: [ChatSession], selectedSessionID: String, messages: [ChatMessage]) {
        self.sessions = sessions
        self.selectedSessionID = selectedSessionID
        self.messages = messages
        self.timelineRows = messages.map(Self.simpleTimelineRow)
        self.coreAdapter = nil
        let adapter = MobileCoreAdapter(
            onState: { [weak self] state in self?.apply(coreState: state) },
            onPairingState: { [weak self] state in self?.apply(pairingState: state) },
            onAccountState: { [weak self] state in self?.apply(accountState: state) },
            onRemoteState: { [weak self] state in self?.apply(remoteState: state) },
            onWorkspaceState: { [weak self] state in self?.apply(workspaceState: state) },
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
        if arguments.contains("--remote-create") {
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

    func send() {
        if surface == .remote {
            sendRemote()
            return
        }
        let value = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty || !composerImages.isEmpty else { return }
        guard !isSending && !busy else { return }
        if surface == .local {
            localSessionSelected = true
            if selectedSession == nil, let first = sessions.first {
                selectedSessionID = first.id
            }
        }
        let optimisticMessage = ChatMessage(id: UUID(), role: .user, text: value)
        messages.append(optimisticMessage)
        timelineRows.append(Self.simpleTimelineRow(optimisticMessage, images: composerImages))
        draft = ""
        isSending = true
        busy = true
        coreAdapter?.updateDraft(value)
        coreAdapter?.setGeneralChatImages(composerImages)
        composerImages = []
        coreAdapter?.send()
    }

    func select(_ session: ChatSession) {
        selectedSessionID = session.id
        if surface == .remote {
            remoteSessionSelected = true
            coreAdapter?.openRemoteSession(sessionID: session.id)
        } else {
            localSessionSelected = true
            coreAdapter?.selectGeneralSession(sessionID: session.id)
        }
        drawerOpen = false
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
        coreAdapter?.disconnect()
        directPairingConnected = false
        directPairingDeviceName = nil
        remoteConnected = false
        remoteSessionSelected = false
        remoteSessions = []
        remoteWorkspaces = []
        workspaceCatalog = []
        pendingRemoteWorkspaceCreate = nil
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

    func newLocalChat() {
        surface = .local
        drawerOpen = false
        localSessionSelected = false
        selectedSessionID = ""
        messages = []
        timelineRows = []
        draft = ""
        composerImages = []
        coreAdapter?.newGeneralSession()
    }

    func selectRemoteDevice(_ device: MobileAccountDevice) {
        guard device.online else {
            showToast(localized("这台桌面设备当前离线"))
            return
        }
        surface = .remote
        drawerOpen = false
        guard !device.selected else { return }
        directPairingConnected = false
        directPairingDeviceName = nil
        accountBusy = true
        remoteSessionSelected = false
        remoteConnected = directPairingConnected
        remoteSessions = []
        remoteWorkspaces = []
        workspaceCatalog = []
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false
        selectedRemoteWorkspaceKind = ""
        messages = []
        timelineRows = []
        coreAdapter?.selectAccountDevice(id: device.id)
    }

    func refreshRemoteDevices() {
        guard accountUser != nil else { return }
        coreAdapter?.refreshAccountDevices()
    }

    func logoutAccount() {
        coreAdapter?.logoutAccount()
        accountUser = nil
        accountUserID = nil
        accountDeviceName = nil
        accountDeviceCount = 0
        accountDevices = []
        accountSelectedDeviceID = nil
        remoteConnected = directPairingConnected
        if !directPairingConnected {
            remoteSessionSelected = false
            remoteSessions = []
            remoteWorkspaces = []
            workspaceCatalog = []
            pendingRemoteWorkspaceCreate = nil
            pendingRemoteAssistantCreate = false
            selectedRemoteWorkspaceKind = ""
            surface = .local
        }
    }

    func selectRemoteWorkspace(_ workspace: MobileWorkspaceGroup) {
        guard remoteConnected else {
            showToast(localized("请先连接桌面设备"))
            return
        }
        surface = .remote
        drawerOpen = false
        coreAdapter?.selectRemoteWorkspace(path: workspace.path)
    }

    func createRemoteSession(in workspace: MobileWorkspaceGroup, agentType: String) {
        guard remoteConnected, !busy else { return }
        drawerOpen = false
        surface = .remote
        createRemoteSession(
            agentType: agentType,
            title: "",
            instruction: "",
            workspacePath: workspace.path
        )
    }

    func createRemoteAssistantSession() {
        guard remoteConnected, !busy else { return }
        drawerOpen = false
        surface = .remote
        if selectedRemoteWorkspaceKind.lowercased() == "assistant" {
            createRemoteSession(agentType: "Claw", title: "", instruction: "")
            return
        }
        guard let assistant = remoteAssistants.first else {
            showToast(localized("暂无可用工作区"))
            return
        }
        pendingRemoteAssistantCreate = true
        coreAdapter?.selectRemoteAssistant(path: assistant.path)
    }

    func selectRemoteAssistant(_ assistant: MobileAssistantOption) {
        guard remoteConnected else { return }
        coreAdapter?.selectRemoteAssistant(path: assistant.path)
    }

    func createRemoteSession(
        agentType: String,
        title: String,
        instruction: String,
        modelID: String? = nil,
        workspacePath: String? = nil
    ) {
        guard remoteConnected, !busy else { return }
        let normalizedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalizedInstruction = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        let selectedModel = modelID ?? modelOptions.first(where: \.selected)?.id
        coreAdapter?.createRemoteSession(
            agentType: agentType,
            title: normalizedTitle,
            instruction: normalizedInstruction,
            modelID: selectedModel,
            workspacePath: workspacePath
        )
        remoteCreateOpen = false
        surface = .remote
    }

    func deleteRemoteSession(_ session: ChatSession) {
        guard !busy else { return }
        coreAdapter?.deleteRemoteSession(sessionID: session.id)
        if selectedSessionID == session.id {
            remoteSessionSelected = false
            timelineRows = []
            messages = []
        }
    }

    func searchRemoteSessions(_ query: String) {
        remoteQuery = query
        guard remoteConnected else { return }
        coreAdapter?.searchRemoteSessions(query: query)
    }

    func loadMoreRemoteSessions() {
        guard remoteConnected, remoteHasMore, !busy else { return }
        coreAdapter?.loadMoreRemoteSessions()
    }

    func loadOlderRemoteMessages() {
        guard surface == .remote, remoteConnected, remoteHasMoreMessages, !busy else { return }
        coreAdapter?.loadOlderRemoteMessages()
    }

    func refreshRemoteSessions() {
        guard remoteConnected, !busy else { return }
        coreAdapter?.refreshRemoteSessions()
    }

    func setRemoteAgentFilter(_ name: String) {
        let filter: SessionAgentFilter
        switch name {
        case "CODE": filter = .code
        case "COWORK": filter = .cowork
        default: filter = .all
        }
        remoteAgentFilter = name
        coreAdapter?.setRemoteAgentFilter(filter)
    }

    func refreshRemotePermissionMode() {
        guard remoteConnected else { return }
        coreAdapter?.refreshRemotePermissionMode()
    }

    func setRemotePermissionMode(_ name: String) {
        let mode: SessionPermissionMode
        switch name {
        case "AUTO": mode = .auto
        case "FULL_ACCESS": mode = .fullAccess
        default: mode = .ask
        }
        coreAdapter?.setRemotePermissionMode(mode)
    }

    func retryRemoteWorkspaces() {
        coreAdapter?.loadRemoteWorkspaces()
    }

    func archiveLocalSession(_ session: ChatSession) {
        coreAdapter?.archiveGeneralSession(
            sessionID: session.id,
            archived: session.status.lowercased() != "archived"
        )
    }

    func deleteLocalSession(_ session: ChatSession) {
        coreAdapter?.deleteGeneralSession(sessionID: session.id)
        if selectedSessionID == session.id {
            localSessionSelected = false
        }
    }

    func saveGeneralConfig(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        coreAdapter?.saveGeneralConfig(
            baseURL: baseURL, model: model, apiKey: apiKey, clearAPIKey: clearAPIKey
        )
    }

    func testGeneralConnection(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        coreAdapter?.testGeneralConnection(
            baseURL: baseURL, model: model, apiKey: apiKey, clearAPIKey: clearAPIKey
        )
    }

    func exportSelectedSession() {
        guard surface == .local, let session = selectedSession else { return }
        coreAdapter?.exportGeneralSession(sessionID: session.id)
    }

    func exportLocalSession(_ session: ChatSession) {
        coreAdapter?.exportGeneralSession(sessionID: session.id)
    }

    func showSessionDetails(_ session: ChatSession) {
        sessionDetails = session
    }

    func dismissSessionDetails() {
        sessionDetails = nil
    }

    func finishGeneralExport() {
        generalExportOpen = false
        generalExportData = Data()
        coreAdapter?.clearGeneralExport()
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
        accountDevices = [
            MobileAccountDevice(id: "preview-desktop", name: "DESKTOP-KM3L4UI", online: true, selected: true)
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
        pairingError = nil
        pairingBusy = true
        coreAdapter?.submitPairing(url: url)
    }

    func submitPairing(url: String, userID: String, password: String) {
        pairingError = nil
        pairingBusy = true
        coreAdapter?.submitPairing(url: url, userID: userID, password: password)
    }

    func loginAccount(relayURL: String, username: String, password: String) {
        accountBusy = true
        coreErrorMessage = nil
        coreAdapter?.loginAccount(relayURL: relayURL, username: username, password: password)
    }

    func sendRemote() {
        let value = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty || !composerImages.isEmpty,
              !isSending,
              connectionPhase != .disconnected,
              let sessionID = visibleSessions.first(where: { $0.id == selectedSessionID })?.id else { return }
        let images = composerImages
        draft = ""
        composerImages = []
        isSending = true
        busy = true
        coreAdapter?.sendRemote(sessionID: sessionID, content: value, images: images)
    }

    func syncDraftToCore() {
        if surface == .local {
            coreAdapter?.updateDraft(draft)
        }
    }

    func addComposerImage(data: Data, mimeType: String) {
        guard composerImages.count < 4, data.count <= 10 * 1024 * 1024 else {
            showToast(localized("最多添加 4 张且每张不超过 10 MB 的图片"))
            return
        }
        composerImages.append(
            ComposerAttachment(id: UUID().uuidString, data: data, mimeType: mimeType)
        )
        if surface == .local {
            coreAdapter?.setGeneralChatImages(composerImages)
        }
    }

    func removeComposerImage(id: String) {
        composerImages.removeAll { $0.id == id }
        if surface == .local {
            coreAdapter?.setGeneralChatImages(composerImages)
        }
    }

    func stopSending() {
        if surface == .remote {
            guard remoteSessionSelected else { return }
            coreAdapter?.cancelRemoteTurn(sessionID: selectedSessionID, turnID: activeTurnID)
        } else {
            coreAdapter?.cancelGeneralChat()
        }
    }

    func approveTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.approveRemoteTool(sessionID: selectedSessionID, toolID: toolID)
    }

    func rejectTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.rejectRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            reason: "Rejected from the iOS client"
        )
    }

    func cancelTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.cancelRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            reason: "Cancelled from the iOS client"
        )
    }

    func answerTool(_ toolID: String, answer: String) {
        let normalized = answer.trimmingCharacters(in: .whitespacesAndNewlines)
        guard surface == .remote,
              remoteSessionSelected,
              !toolID.isEmpty,
              !normalized.isEmpty else { return }
        coreAdapter?.answerRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            answer: normalized
        )
    }

    func answerTool(_ toolID: String, answers: [QuestionAnswer]) {
        guard surface == .remote,
              remoteSessionSelected,
              !toolID.isEmpty,
              !answers.isEmpty else { return }
        coreAdapter?.answerRemoteToolStructured(
            sessionID: selectedSessionID,
            toolID: toolID,
            answers: answers
        )
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

    func openRemoteFile(reference: String, label: String) {
        guard surface == .remote, remoteSessionSelected else {
            showToast(localized("仅远程工作区文件支持预览"))
            return
        }
        filePreviewLoading = true
        coreAdapter?.openRemoteFile(
            reference: reference,
            label: label,
            sessionID: selectedSessionID
        )
    }

    func downloadRemoteFile(reference: String, label: String) {
        guard surface == .remote, remoteSessionSelected else { return }
        downloadTargetPath = reference
            .replacingOccurrences(of: "computer://", with: "", options: [.caseInsensitive])
        downloadPhase = .preparing
        downloadStatusText = localized("正在准备下载")
        coreAdapter?.downloadRemoteFile(
            reference: reference,
            label: label,
            sessionID: selectedSessionID
        )
    }

    func finishDownloadExport(success: Bool) {
        guard let download = pendingDownload else { return }
        if success {
            coreAdapter?.remoteDownloadSaved(reference: download.reference)
            downloadPhase = .saved
            downloadStatusText = localized("已下载")
            showToast(localizedFormat("已保存 %@", download.name))
        } else {
            coreAdapter?.remoteDownloadSaveFailed(reference: download.reference)
            downloadPhase = .failed
            downloadStatusText = localized("保存失败")
            showToast(localized("文件保存失败"))
        }
        pendingDownload = nil
        downloadExporterOpen = false
    }

    func downloadStatus(for remotePath: String) -> String? {
        guard downloadTargetPath == remotePath else { return nil }
        return downloadStatusText
    }

    func dismissFilePreview() {
        filePreview = nil
        filePreviewLoading = false
        coreAdapter?.dismissRemoteFilePreview()
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

    func selectModel(_ modelID: String) {
        guard selectedSession != nil else { return }
        if surface == .remote {
            coreAdapter?.selectRemoteModel(sessionID: selectedSessionID, modelID: modelID)
        } else {
            coreAdapter?.selectGeneralModel(modelID: modelID)
        }
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

    private func apply(coreState state: GeneralChatUiState) {
        generalConfigured = state.configured
        generalConfigBaseURL = state.config.baseUrl
        generalConfigModel = state.config.model
        generalConfigHasAPIKey = state.config.hasApiKey
        generalConfigFailure = state.configFailure?.name
        generalConnectionTestRunning = state.connectionTest.running
        if state.connectionTest.passed {
            generalConnectionTestMessage = localized("连接成功")
        } else if let failure = state.connectionTest.failure {
            generalConnectionTestMessage = localizedFormat("连接失败：%@", failure.name)
        } else {
            generalConnectionTestMessage = nil
        }
        if let exported = state.export {
            let safeTitle = exported.title
                .replacingOccurrences(of: "/", with: "-")
                .replacingOccurrences(of: "\\", with: "-")
                .replacingOccurrences(of: ":", with: "-")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            generalExportName = safeTitle.isEmpty ? "conversation.md" : "\(safeTitle).md"
            generalExportData = Data(exported.markdown.utf8)
            generalExportOpen = true
        }
        if !state.sessions.isEmpty {
            sessions = state.sessions.map { session in
                ChatSession(
                    id: session.id,
                    title: session.title.isEmpty ? localized("未命名会话") : session.title,
                    updatedLabel: session.updatedAt,
                    pinned: session.pinned,
                    status: session.status,
                )
            }
        }
        if !state.messages.isEmpty {
            messages = state.messages.map { message in
                let text = message.blocks.map(\.text).joined(separator: "\n")
                return ChatMessage(
                    id: UUID(uuidString: message.id) ?? UUID(),
                    role: message.role.lowercased() == "user" ? .user : .assistant,
                    text: text,
                )
            }
            timelineRows = messages.map(Self.simpleTimelineRow)
        }
        if !composerModelPickerPreview, draft != state.draft { draft = state.draft }
        isSending = state.busy
        busy = state.busy
        if !composerModelPickerPreview {
            modelOptions = state.models.map { model in
                ComposerModelOption(
                    id: model.id,
                    primaryLabel: model.label,
                    secondaryLabel: model.source.name,
                    source: model.source.name,
                    selected: model.id == state.activeModelId
                )
            }
        }
        if !accountLoginPreview {
            if let failure = state.failure {
                coreErrorMessage = failure.name
            } else {
                coreErrorMessage = nil
            }
        }
    }

    private func apply(pairingState state: PairingUiState) {
        guard !localActionPreview else { return }
        pairingBusy = state is PairingUiStateConnecting
        if let failed = state as? PairingUiStateFailed {
            pairingBusy = false
            pairingError = pairingErrorMessage(failed.failure)
        } else if let paired = state as? PairingUiStatePaired {
            pairingBusy = false
            pairingError = nil
            directPairingConnected = true
            directPairingDeviceName = paired.workspace.roomLabel
            remoteConnected = true
            surface = .remote
            switch paired.liveness {
            case .checking: connectionPhase = .reconnecting
            case .lost: connectionPhase = .disconnected
            default: connectionPhase = .connected
            }
            pairingSheetOpen = false
        }
    }

    private func apply(accountState state: AccountUiState) {
        guard !accountLoginPreview, !localActionPreview else { return }
        accountBusy = state is AccountUiStateSigningIn
        if let ready = state as? AccountUiStateReady {
            accountBusy = false
            accountUser = ready.username
            accountUserID = ready.userId
            accountDeviceName = ready.selectedDeviceName
            accountDeviceCount = ready.devices.count
            accountSelectedDeviceID = ready.selectedDeviceId
            accountRefreshing = ready.refreshing
            accountDevices = ready.devices.map { device in
                MobileAccountDevice(
                    id: device.id,
                    name: device.name,
                    online: device.online,
                    selected: device.id == ready.selectedDeviceId
                )
            }
            if !directPairingConnected,
               ready.selectedDeviceId == nil,
               let target = ready.devices.first(where: { $0.online }) {
                accountBusy = true
                coreAdapter?.selectAccountDevice(id: target.id)
                return
            }
            remoteConnected = directPairingConnected || ready.selectedDeviceId != nil
            surface = .remote
            connectionPhase = .connected
            if ready.refreshFailure != nil {
                showToast(localized("设备列表刷新失败，仍显示上次结果"))
            }
        } else if let failed = state as? AccountUiStateFailed {
            accountBusy = false
            coreErrorMessage = accountErrorMessage(failed.reason.name)
            if !directPairingConnected { connectionPhase = .disconnected }
            if failed.reason.name == "AUTHENTICATION" {
                accountUser = nil
                accountUserID = nil
                accountDevices = []
                accountSelectedDeviceID = nil
                accountDeviceName = nil
                accountDeviceCount = 0
                accountRefreshing = false
                remoteConnected = directPairingConnected
            }
        } else if state is AccountUiStateSignedOut {
            accountBusy = false
            accountUser = nil
            accountUserID = nil
            accountDevices = []
            accountSelectedDeviceID = nil
            accountDeviceName = nil
            accountDeviceCount = 0
            accountRefreshing = false
            if !directPairingConnected {
                remoteConnected = false
                remoteSessionSelected = false
                remoteSessions = []
                remoteWorkspaces = []
                workspaceCatalog = []
            }
        }
    }

    private func apply(remoteState state: RemoteSessionUiState) {
        guard !localActionPreview, !accountLoginPreview else { return }
        guard let ready = state as? RemoteSessionUiStateReady else {
            if let failed = state as? RemoteSessionUiStateFailed {
                connectionPhase = .disconnected
                coreErrorMessage = failed.remoteMessage ?? failed.reason.name
            }
            return
        }
        remoteConnected = true
        surface = .remote
        connectionPhase = .connected
        remoteSessions = ready.sessions.map { session in
            ChatSession(
                id: session.id,
                title: session.title.isEmpty ? localized("未命名会话") : session.title,
                updatedLabel: session.updatedAt,
                status: session.status,
                agentType: session.agentType,
                workspacePath: session.workspacePath,
                workspaceName: session.workspaceName,
                createdAt: session.createdAt,
                messageCount: Int(session.messageCount),
            )
        }
        rebuildRemoteWorkspaceGroups()
        if let selected = ready.selectedSessionId {
            selectedSessionID = selected
        }
        remoteSessionSelected = ready.selectedSessionId != nil
        busy = ready.busy
        remoteQuery = ready.query
        remoteAgentFilter = ready.agentFilter.name
        remoteHasMore = ready.hasMore
        remoteHasMoreMessages = ready.hasMoreMessages
        remotePermissionMode = ready.permissionMode?.name ?? remotePermissionMode
        remotePermissionFailure = ready.permissionModeFailure?.name
        activeTurnID = ready.timeline?.activeTurn?.turnId
        isSending = ready.timeline?.activeTurn != nil
        modelOptions = ready.createModelOptions(fallbackLabel: localized("模型")).map { option in
            ComposerModelOption(
                id: option.id,
                primaryLabel: option.primaryLabel,
                secondaryLabel: option.secondaryLabel,
                source: "REMOTE",
                selected: option.selected
            )
        }
        if let timeline = ready.timeline {
            timelineRows = timeline.conversationRows().map(Self.mapConversationRow)
            messages = timelineRows.compactMap { row in
                guard row.kind != "EMPTY" else { return nil }
                return ChatMessage(
                    id: UUID(uuidString: row.id) ?? UUID(),
                    role: row.kind == "USER" ? .user : .assistant,
                    text: row.text
                )
            }
        } else {
            timelineRows = []
            messages = []
        }
    }

    private func apply(workspaceState state: RemoteWorkspaceUiState) {
        workspaceLoading = state is RemoteWorkspaceUiStateLoading
        workspaceLoadFailed = state is RemoteWorkspaceUiStateFailed
        if state is RemoteWorkspaceUiStateFailed {
            if pendingRemoteWorkspaceCreate != nil || pendingRemoteAssistantCreate {
                pendingRemoteWorkspaceCreate = nil
                pendingRemoteAssistantCreate = false
                showToast(localized("工作区加载失败，点按重试"))
            }
            return
        }
        guard let ready = state as? RemoteWorkspaceUiStateReady else { return }

        workspaceLoading = false
        workspaceLoadFailed = false
        selectedRemoteWorkspaceKind = ready.selected?.kind ?? ""
        var seen = Set<String>()
        var catalog: [(path: String, name: String, selected: Bool)] = []
        if let selected = ready.selected,
           !selected.path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            seen.insert(selected.path)
            catalog.append((selected.path, selected.name, true))
        }
        for workspace in ready.workspaces where !workspace.path.isEmpty && !seen.contains(workspace.path) {
            seen.insert(workspace.path)
            catalog.append((workspace.path, workspace.name, false))
        }
        workspaceCatalog = catalog
        remoteAssistants = ready.assistants.map {
            MobileAssistantOption(path: $0.path, name: $0.name)
        }
        rebuildRemoteWorkspaceGroups()
        apply(filePreviewState: ready.preview)
        apply(downloadState: ready.download)
        if let pending = pendingRemoteWorkspaceCreate,
           ready.selected?.path == pending.path {
            pendingRemoteWorkspaceCreate = nil
            createRemoteSession(agentType: pending.agentType, title: "", instruction: "")
        }
        if pendingRemoteAssistantCreate,
           ready.selected?.kind.lowercased() == "assistant" {
            pendingRemoteAssistantCreate = false
            createRemoteSession(agentType: "Claw", title: "", instruction: "")
        }
    }

    private func apply(downloadState state: RemoteFileDownloadUiState) {
        if state is RemoteFileDownloadUiStateNone { return }
        if let loading = state as? RemoteFileDownloadUiStateLoading {
            downloadTargetPath = loading.target.remotePath
            downloadPhase = .downloading
            if loading.totalBytes > 0 {
                downloadStatusText = localizedFormat(
                    "正在下载 %@ / %@",
                    FilePreviewFormat.shared.bytes(value: loading.downloadedBytes),
                    FilePreviewFormat.shared.bytes(value: loading.totalBytes)
                )
            } else {
                downloadStatusText = localized("正在下载")
            }
        } else if let awaiting = state as? RemoteFileDownloadUiStateAwaitingSave {
            let reference = awaiting.target.path
            downloadTargetPath = awaiting.target.remotePath
            downloadPhase = .saving
            downloadStatusText = localized("正在保存")
            if pendingDownload?.reference != reference {
                pendingDownload = MobilePendingDownload(
                    reference: reference,
                    remotePath: awaiting.target.remotePath,
                    name: awaiting.name,
                    mimeType: awaiting.mimeType,
                    data: Self.data(from: awaiting.bytes)
                )
                downloadExporterOpen = true
            }
        } else if let saved = state as? RemoteFileDownloadUiStateSaved {
            downloadTargetPath = saved.target.remotePath
            downloadPhase = .saved
            downloadStatusText = localized("已下载")
        } else if let failed = state as? RemoteFileDownloadUiStateFailed {
            downloadTargetPath = failed.target.remotePath
            downloadPhase = .failed
            downloadStatusText = localized("下载失败")
        }
    }

    private func apply(filePreviewState state: RemoteFilePreviewUiState) {
        if let loading = state as? RemoteFilePreviewUiStateLoading {
            filePreviewLoading = true
            filePreview = MobileFilePreview(
                id: loading.target.remotePath,
                name: loading.target.displayName,
                content: "",
                mimeType: "",
                imageData: nil,
                truncated: false,
                failure: nil
            )
            return
        }
        filePreviewLoading = false
        if state is RemoteFilePreviewUiStateNone {
            filePreview = nil
        } else if let text = state as? RemoteFilePreviewUiStateText {
            filePreview = MobileFilePreview(
                id: text.target.remotePath,
                name: text.name,
                content: text.content,
                mimeType: text.mimeType,
                imageData: nil,
                truncated: text.truncated,
                failure: nil
            )
        } else if let image = state as? RemoteFilePreviewUiStateImage {
            filePreview = MobileFilePreview(
                id: image.target.remotePath,
                name: image.name,
                content: "",
                mimeType: image.mimeType,
                imageData: Self.data(from: image.bytes),
                truncated: false,
                failure: nil
            )
        } else if let unsupported = state as? RemoteFilePreviewUiStateUnsupported {
            filePreview = MobileFilePreview(
                id: unsupported.target.remotePath,
                name: unsupported.target.displayName,
                content: "",
                mimeType: unsupported.mimeType,
                imageData: nil,
                truncated: false,
                failure: localized("此文件类型暂不支持预览")
            )
        } else if let failed = state as? RemoteFilePreviewUiStateFailed {
            filePreview = MobileFilePreview(
                id: failed.target.remotePath,
                name: failed.target.displayName,
                content: "",
                mimeType: failed.mimeType,
                imageData: nil,
                truncated: false,
                failure: localizedFormat("文件预览失败：%@", failed.kind.name)
            )
        }
    }

    private func rebuildRemoteWorkspaceGroups() {
        let selectedPath = workspaceCatalog.first(where: { $0.selected })?.path
        remoteWorkspaces = workspaceCatalog.map { workspace in
            MobileWorkspaceGroup(
                path: workspace.path,
                name: workspace.name.isEmpty ? workspace.path : workspace.name,
                selected: workspace.selected,
                sessions: remoteSessions.filter { session in
                    (session.workspacePath ?? selectedPath) == workspace.path
                }
            )
        }
    }

    private static func simpleTimelineRow(_ message: ChatMessage) -> MobileConversationRow {
        simpleTimelineRow(message, images: [])
    }

    private static func simpleTimelineRow(
        _ message: ChatMessage,
        images: [ComposerAttachment]
    ) -> MobileConversationRow {
        MobileConversationRow(
            id: message.id.uuidString,
            kind: message.role == .user ? "USER" : "ASSISTANT",
            text: message.text,
            thinking: nil,
            images: images.map {
                MobileTimelineImage(name: "image", dataURL: $0.dataURL)
            },
            tools: [],
            blocks: [],
            streaming: false,
            typing: false,
            pending: false,
            showRetry: false
        )
    }

    private static func mapConversationRow(_ row: ConversationRow) -> MobileConversationRow {
        MobileConversationRow(
            id: row.id,
            kind: row.kind.name,
            text: row.text,
            thinking: row.thinking,
            images: row.images.map {
                MobileTimelineImage(name: $0.name, dataURL: $0.dataUrl)
            },
            tools: row.tools.map(mapTool),
            blocks: row.blocks.map(mapBlock),
            streaming: row.streaming,
            typing: row.typing,
            pending: row.pending,
            showRetry: row.showRetry
        )
    }

    private static func mapTool(_ tool: ToolCard) -> MobileTimelineTool {
        MobileTimelineTool(
            id: tool.id,
            name: tool.name,
            phase: tool.phase.name,
            kind: tool.kind.name,
            operation: tool.operation.name,
            target: tool.target,
            filePath: tool.filePath,
            fileLabel: tool.fileLabel,
            input: tool.input,
            output: tool.output,
            question: tool.question,
            questions: tool.questions.map { question in
                MobileTimelineQuestion(
                    index: Int(question.index),
                    header: question.header,
                    question: question.question,
                    options: question.options.map {
                        MobileTimelineOption(label: $0.label, description: $0.description_)
                    },
                    multiSelect: question.multiSelect
                )
            },
            actions: Set(tool.actions.map(\.name))
        )
    }

    private static func mapBlock(_ block: MessageBlock) -> MobileTimelineBlock {
        if let text = block as? MessageBlockText {
            return .text(id: text.id, text: text.text, streaming: text.streaming)
        }
        if let thinking = block as? MessageBlockThinking {
            return .thinking(id: thinking.id, text: thinking.text, streaming: thinking.streaming)
        }
        if let tools = block as? MessageBlockTools {
            return .tools(id: tools.id, tools: tools.tools.map(mapTool))
        }
        if let subagent = block as? MessageBlockSubagent {
            return .subagent(
                id: subagent.id,
                title: subagent.title,
                running: subagent.running,
                text: subagent.text,
                children: subagent.children.map(mapBlock)
            )
        }
        return .text(id: block.id, text: "", streaming: false)
    }

    private static func data(from bytes: KotlinByteArray) -> Data {
        Data((0..<Int(bytes.size)).map { index in
            UInt8(bitPattern: bytes.get(index: Int32(index)))
        })
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

    private func accountErrorMessage(_ reason: String) -> String {
        switch reason {
        case "INVALID_CREDENTIALS", "UNAUTHORIZED":
            return localized("账号或密码错误")
        case "NETWORK":
            return localized("网络不可用，请检查 relay 地址")
        case "TIMEOUT":
            return localized("登录超时，请稍后重试")
        default:
            return localized("登录失败，请检查账号、密码和 relay 地址")
        }
    }
}

extension MobileAppModel {
    var sessionListWorkspaceOptions: [MobileSessionWorkspaceOption] {
        SessionListPresentation.shared
            .workspaceOptions(sessions: sessionListCoreSessions, workspace: sessionListWorkspaceContext)
            .map { MobileSessionWorkspaceOption(path: $0.path, name: $0.name) }
    }

    var sessionListAgentGroups: [String] {
        SessionListPresentation.shared
            .agentGroups(sessions: sessionListCoreSessions, workspace: sessionListWorkspaceContext)
            .map(\.name)
    }

    var sessionListStatusOptions: [String] {
        SessionListPresentation.shared.statusOptions(sessions: sessionListCoreSessions)
    }

    var sessionListSections: [MobileSessionListSectionProjection] {
        let groupMode: SessionGroupMode = switch remoteGroupMode {
        case "TIME": .time
        case "CHAT": .chat
        default: .project
        }
        let agentFilter: SessionAgentGroup? = switch remoteViewAgentFilter {
        case "CHAT": .chat
        case "CODE": .code
        case "COWORK": .cowork
        default: nil
        }
        let view = SessionListPresentation.shared.view(
            sessions: sessionListCoreSessions,
            workspace: sessionListWorkspaceContext,
            options: SessionListOptions(
                groupMode: groupMode,
                query: "",
                workspaceFilter: remoteWorkspaceFilter,
                agentFilter: agentFilter,
                statusFilter: remoteStatusFilter
            ),
            nowMs: Int64(Date().timeIntervalSince1970 * 1_000)
        )
        let byID = Dictionary(uniqueKeysWithValues: remoteSessions.map { ($0.id, $0) })
        return view.sections.compactMap { section in
            switch onEnum(of: section) {
            case .chat(let value):
                return projection(id: "chat", kind: .chat, section: value, byID: byID)
            case .project(let value):
                return MobileSessionListSectionProjection(
                    id: "project:\(value.path)",
                    kind: .project,
                    path: value.path,
                    name: value.name,
                    sessions: value.sessions.compactMap { byID[$0.id] }
                )
            case .today(let value):
                return projection(id: "today", kind: .today, section: value, byID: byID)
            case .yesterday(let value):
                return projection(id: "yesterday", kind: .yesterday, section: value, byID: byID)
            case .earlier(let value):
                return projection(id: "earlier", kind: .earlier, section: value, byID: byID)
            }
        }
    }

    private var sessionListCoreSessions: [RemoteSession] {
        remoteSessions.map { session in
            RemoteSession(
                id: session.id,
                title: session.title,
                agentType: session.agentType,
                status: session.status,
                updatedAt: session.updatedLabel,
                createdAt: session.createdAt,
                messageCount: Int32(session.messageCount),
                workspacePath: session.workspacePath,
                workspaceName: session.workspaceName
            )
        }
    }

    private var sessionListWorkspaceContext: SessionWorkspaceContext {
        let assistantPaths = Set(remoteAssistants.map { normalizedSessionWorkspacePath($0.path) })
        let selected = remoteWorkspaces.first(where: \.selected)
        let recent = remoteWorkspaces.map { workspace in
            RecentWorkspace(
                path: workspace.path,
                name: workspace.name,
                lastOpened: "",
                kind: assistantPaths.contains(normalizedSessionWorkspacePath(workspace.path))
                    ? "assistant"
                    : "normal"
            )
        }
        let selectedKind = selected.map {
            assistantPaths.contains(normalizedSessionWorkspacePath($0.path)) ? "assistant" : "normal"
        } ?? ""
        return SessionWorkspaceContext(
            selectedPath: selected?.path ?? "",
            selectedName: selected?.name ?? "",
            selectedKind: selectedKind,
            recent: recent
        )
    }

    private func projection(
        id: String,
        kind: MobileSessionListSectionKind,
        section: any SessionListSection,
        byID: [String: ChatSession]
    ) -> MobileSessionListSectionProjection {
        MobileSessionListSectionProjection(
            id: id,
            kind: kind,
            path: "",
            name: "",
            sessions: section.sessions.compactMap { byID[$0.id] }
        )
    }

    private func normalizedSessionWorkspacePath(_ path: String) -> String {
        var result = path.trimmingCharacters(in: .whitespacesAndNewlines)
        while result.count > 1 && (result.hasSuffix("/") || result.hasSuffix("\\")) {
            result.removeLast()
        }
        return result
    }
}

private extension Array where Element == String {
    func value(after flag: String) -> String? {
        guard let position = firstIndex(of: flag), position < self.index(before: endIndex) else { return nil }
        return self[self.index(after: position)]
    }
}
