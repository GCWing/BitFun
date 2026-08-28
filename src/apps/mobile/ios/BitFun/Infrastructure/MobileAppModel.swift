import Foundation
import SwiftUI
import BitFunMobileCore

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
            pairingError = PairingFailureCopy.message(failed.failure, localized: localized)
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
}
