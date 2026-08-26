import BitFunMobileCore
import Foundation

/// Swift owns presentation state; this adapter owns the shared KMP feature seam.
@MainActor
final class MobileCoreAdapter {
    let deviceID: String
    private let scope: any CoroutineScope
    private let generalChat: GeneralChatStore
    private let pairing: PairingStore
    private let account: AccountStore
    private var remoteSession: RemoteSessionStore?
    private var remoteWorkspace: RemoteWorkspaceStore?
    private var remoteTargetKey: String?
    private var observations: [Task<Void, Never>] = []
    private var remoteObservations: [Task<Void, Never>] = []

    var onState: ((GeneralChatUiState) -> Void)?
    var onPairingState: ((PairingUiState) -> Void)?
    var onAccountState: ((AccountUiState) -> Void)?
    var onRemoteState: ((RemoteSessionUiState) -> Void)?
    var onWorkspaceState: ((RemoteWorkspaceUiState) -> Void)?

    init(
        onState: ((GeneralChatUiState) -> Void)? = nil,
        onPairingState: ((PairingUiState) -> Void)? = nil,
        onAccountState: ((AccountUiState) -> Void)? = nil,
        onRemoteState: ((RemoteSessionUiState) -> Void)? = nil,
        onWorkspaceState: ((RemoteWorkspaceUiState) -> Void)? = nil,
    ) {
        self.scope = MainScope()
        self.generalChat = GeneralChatStore.companion.create(scope: scope)
        let defaults = UserDefaults.standard
        let installID: String
        if let stored = defaults.string(forKey: "bitfun.mobile.install_id") {
            installID = stored
        } else {
            installID = UUID().uuidString
            defaults.set(installID, forKey: "bitfun.mobile.install_id")
        }
        self.deviceID = installID
        self.pairing = PairingStore.companion.create(
            scope: scope,
            device: DeviceIdentity(installId: installID, displayName: "BitFun iPhone"),
            log: CoreLogNone.shared,
        )
        self.account = AccountStore.companion.create(
            scope: scope,
            service: "com.bitfun.mobile.account",
            deviceId: installID,
            deviceName: "BitFun iPhone",
            log: CoreLogNone.shared,
        )
        self.onState = onState
        self.onPairingState = onPairingState
        self.onAccountState = onAccountState
        self.onRemoteState = onRemoteState
        self.onWorkspaceState = onWorkspaceState

        let flow = SkieSwiftStateFlow<GeneralChatUiState>(generalChat.state)
        onState?(flow.value)
        observations.append(Task { [weak self] in
            for await state in flow {
                guard !Task.isCancelled else { return }
                self?.onState?(state)
            }
        })

        let accountFlow = SkieSwiftStateFlow<AccountUiState>(account.state)
        onAccountState?(accountFlow.value)
        observations.append(Task { [weak self] in
            for await state in accountFlow {
                guard !Task.isCancelled else { return }
                self?.onAccountState?(state)
                if let ready = state as? AccountUiStateReady {
                    self?.startAccountRemoteSessionIfNeeded(ready: ready)
                }
            }
        })

        let pairingFlow = SkieSwiftStateFlow<PairingUiState>(pairing.state)
        onPairingState?(pairingFlow.value)
        observations.append(Task { [weak self] in
            for await state in pairingFlow {
                guard !Task.isCancelled else { return }
                self?.onPairingState?(state)
                if let paired = state as? PairingUiStatePaired {
                    self?.startRemoteSessionStoreIfNeeded(paired: paired)
                }
            }
        })

        account.dispatch(intent: AccountIntentRestore.shared)
        pairing.dispatch(intent: PairingIntentForeground.shared)
    }

    func updateDraft(_ text: String) {
        generalChat.dispatch(intent: GeneralChatIntentUpdateDraft(text: text))
    }

    func send() {
        generalChat.dispatch(intent: GeneralChatIntentSend.shared)
    }

    func cancelGeneralChat() {
        generalChat.dispatch(intent: GeneralChatIntentCancel.shared)
    }

    func setGeneralChatImages(_ images: [ComposerAttachment]) {
        generalChat.dispatch(intent: GeneralChatIntentSetImages(images: images.map(\.coreImage)))
    }

    func renameGeneralSession(sessionID: String, title: String) {
        generalChat.dispatch(intent: GeneralChatIntentRenameSession(sessionId: sessionID, title: title))
    }

    func pinGeneralSession(sessionID: String, pinned: Bool) {
        generalChat.dispatch(intent: GeneralChatIntentPinSession(sessionId: sessionID, pinned: pinned))
    }

    func archiveGeneralSession(sessionID: String, archived: Bool) {
        generalChat.dispatch(intent: GeneralChatIntentArchiveSession(sessionId: sessionID, archived: archived))
    }

    func deleteGeneralSession(sessionID: String) {
        generalChat.dispatch(intent: GeneralChatIntentDeleteSession(sessionId: sessionID))
    }

    func selectGeneralModel(modelID: String) {
        generalChat.dispatch(intent: GeneralChatIntentSelectModel(modelId: modelID))
    }

    func selectGeneralSession(sessionID: String) {
        generalChat.dispatch(intent: GeneralChatIntentSelectSession(sessionId: sessionID))
    }

    func saveGeneralConfig(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        generalChat.dispatch(
            intent: GeneralChatIntentSaveConfig(
                baseUrl: baseURL, model: model, apiKey: apiKey, clearApiKey: clearAPIKey
            )
        )
    }

    func testGeneralConnection(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        generalChat.dispatch(
            intent: GeneralChatIntentTestConnection(
                baseUrl: baseURL, model: model, apiKey: apiKey, clearApiKey: clearAPIKey
            )
        )
    }

    func exportGeneralSession(sessionID: String) {
        generalChat.dispatch(
            intent: GeneralChatIntentExportSession(
                sessionId: sessionID,
                untitledLabel: "未命名会话",
                userLabel: "用户",
                assistantLabel: "BitFun"
            )
        )
    }

    func clearGeneralExport() {
        generalChat.dispatch(intent: GeneralChatIntentClearExport.shared)
    }

    func newGeneralSession() {
        generalChat.dispatch(intent: GeneralChatIntentNewSession.shared)
    }

    func submitPairing(url: String) {
        pairing.dispatch(intent: PairingIntentSubmit(pairingUrl: url))
    }

    func submitPairing(url: String, userID: String, password: String) {
        pairing.dispatch(
            intent: PairingIntentSubmit(
                pairingUrl: url,
                userId: userID,
                password: password
            )
        )
    }

    func dismissPairingFailure() {
        pairing.dispatch(intent: PairingIntentDismiss.shared)
    }

    func pairingForeground() {
        pairing.dispatch(intent: PairingIntentForeground.shared)
    }

    func pairingBackground() {
        pairing.dispatch(intent: PairingIntentBackground.shared)
    }

    func verifyPairing() {
        pairing.dispatch(intent: PairingIntentVerify.shared)
    }

    func loginAccount(relayURL: String, username: String, password: String) {
        account.dispatch(intent: AccountIntentLogin(relayUrl: relayURL, username: username, password: password))
    }

    func selectAccountDevice(id: String) {
        account.dispatch(intent: AccountIntentSelectDevice(deviceId: id))
    }

    func refreshAccountDevices() {
        account.dispatch(intent: AccountIntentRefreshDevices.shared)
    }

    func logoutAccount() {
        resetRemoteStores()
        account.dispatch(intent: AccountIntentLogout.shared)
    }

    func disconnect() {
        pairing.dispatch(intent: PairingIntentDisconnect.shared)
        resetRemoteStores()
    }

    func sendRemote(sessionID: String, content: String, images: [ComposerAttachment]) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentSendMessage(
                sessionId: sessionID,
                content: content,
                images: images.isEmpty ? nil : images.map(\.coreImage),
            )
        )
    }

    func cancelRemoteTurn(sessionID: String, turnID: String?) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentCancelTurn(sessionId: sessionID, turnId: turnID)
        )
    }

    func approveRemoteTool(sessionID: String, toolID: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentApproveTool(sessionId: sessionID, toolId: toolID)
        )
    }

    func rejectRemoteTool(sessionID: String, toolID: String, reason: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentRejectTool(sessionId: sessionID, toolId: toolID, reason: reason)
        )
    }

    func cancelRemoteTool(sessionID: String, toolID: String, reason: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentCancelTool(sessionId: sessionID, toolId: toolID, reason: reason)
        )
    }

    func answerRemoteTool(sessionID: String, toolID: String, answer: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentAnswerQuestion(sessionId: sessionID, toolId: toolID, answer: answer)
        )
    }

    func answerRemoteToolStructured(sessionID: String, toolID: String, answers: [QuestionAnswer]) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentAnswerStructuredQuestion(
                sessionId: sessionID,
                toolId: toolID,
                answers: answers
            )
        )
    }

    func renameRemoteSession(sessionID: String, title: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentRenameSession(sessionId: sessionID, title: title)
        )
    }

    func selectRemoteModel(sessionID: String, modelID: String) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentSelectModel(sessionId: sessionID, modelId: modelID)
        )
    }

    func openRemoteSession(sessionID: String) {
        remoteSession?.dispatch(intent: RemoteSessionIntentOpen(sessionId: sessionID))
    }

    func createRemoteSession(
        agentType: String,
        title: String,
        instruction: String,
        modelID: String?,
        workspacePath: String? = nil
    ) {
        remoteSession?.dispatch(
            intent: RemoteSessionIntentCreateSession(
                agentType: agentType,
                title: title,
                instruction: instruction,
                modelId: modelID,
                workspacePath: workspacePath
            )
        )
    }

    func deleteRemoteSession(sessionID: String) {
        remoteSession?.dispatch(intent: RemoteSessionIntentDeleteSession(sessionId: sessionID))
    }

    func searchRemoteSessions(query: String) {
        remoteSession?.dispatch(intent: RemoteSessionIntentSearch(query: query))
    }

    func loadMoreRemoteSessions() {
        remoteSession?.dispatch(intent: RemoteSessionIntentLoadMore.shared)
    }

    func loadOlderRemoteMessages() {
        remoteSession?.dispatch(intent: RemoteSessionIntentLoadOlderMessages.shared)
    }

    func refreshRemoteSessions() {
        remoteSession?.dispatch(intent: RemoteSessionIntentRefresh.shared)
    }

    func setRemoteAgentFilter(_ filter: SessionAgentFilter) {
        remoteSession?.dispatch(intent: RemoteSessionIntentSetAgentFilter(filter: filter))
    }

    func refreshRemotePermissionMode() {
        remoteSession?.dispatch(intent: RemoteSessionIntentRefreshPermissionMode.shared)
    }

    func setRemotePermissionMode(_ mode: SessionPermissionMode) {
        remoteSession?.dispatch(intent: RemoteSessionIntentSetPermissionMode(mode: mode))
    }

    func selectRemoteWorkspace(path: String) {
        remoteWorkspace?.dispatch(intent: RemoteWorkspaceIntentSelectWorkspace(path: path))
    }

    func selectRemoteAssistant(path: String) {
        remoteWorkspace?.dispatch(intent: RemoteWorkspaceIntentSelectAssistant(path: path))
    }

    func loadRemoteWorkspaces() {
        remoteWorkspace?.dispatch(intent: RemoteWorkspaceIntentLoad.shared)
    }

    func openRemoteFile(reference: String, label: String, sessionID: String) {
        remoteWorkspace?.dispatch(
            intent: RemoteWorkspaceIntentOpenFile(
                reference: reference,
                label: label,
                sessionId: sessionID
            )
        )
    }

    func downloadRemoteFile(reference: String, label: String, sessionID: String) {
        remoteWorkspace?.dispatch(
            intent: RemoteWorkspaceIntentDownloadFile(
                reference: reference,
                label: label,
                sessionId: sessionID
            )
        )
    }

    func remoteDownloadSaved(reference: String) {
        remoteWorkspace?.dispatch(
            intent: RemoteWorkspaceIntentDownloadSaved(reference: reference)
        )
    }

    func remoteDownloadSaveFailed(reference: String) {
        remoteWorkspace?.dispatch(
            intent: RemoteWorkspaceIntentDownloadSaveFailed(reference: reference)
        )
    }

    func dismissRemoteFilePreview() {
        remoteWorkspace?.dispatch(intent: RemoteWorkspaceIntentDismissPreview.shared)
    }

    private func startRemoteSessionStoreIfNeeded(paired: PairingUiStatePaired) {
        guard remoteTargetKey != "pairing",
              let sessionStore = pairing.createSessionStore(scope: scope) else { return }
        bindRemoteStores(
            targetKey: "pairing",
            sessionStore: sessionStore,
            workspaceStore: pairing.createWorkspaceStore(scope: scope)
        )
    }

    private func startAccountRemoteSessionIfNeeded(ready: AccountUiStateReady) {
        guard let deviceID = ready.selectedDeviceId else {
            resetRemoteStores()
            return
        }
        let targetKey = "account:\(deviceID)"
        guard remoteTargetKey != targetKey,
              let sessionStore = account.createSessionStore(scope: scope) else { return }
        bindRemoteStores(
            targetKey: targetKey,
            sessionStore: sessionStore,
            workspaceStore: account.createWorkspaceStore(scope: scope)
        )
    }

    private func bindRemoteStores(
        targetKey: String,
        sessionStore: RemoteSessionStore,
        workspaceStore: RemoteWorkspaceStore?
    ) {
        resetRemoteStores()
        remoteTargetKey = targetKey
        remoteSession = sessionStore
        remoteWorkspace = workspaceStore

        let sessionFlow = SkieSwiftStateFlow<RemoteSessionUiState>(sessionStore.state)
        onRemoteState?(sessionFlow.value)
        sessionStore.dispatch(intent: RemoteSessionIntentLoad.shared)
        remoteObservations.append(Task { [weak self] in
            for await state in sessionFlow {
                guard !Task.isCancelled else { return }
                self?.onRemoteState?(state)
            }
        })

        if let workspaceStore {
            let workspaceFlow = SkieSwiftStateFlow<RemoteWorkspaceUiState>(workspaceStore.state)
            onWorkspaceState?(workspaceFlow.value)
            workspaceStore.dispatch(intent: RemoteWorkspaceIntentLoad.shared)
            remoteObservations.append(Task { [weak self] in
                for await state in workspaceFlow {
                    guard !Task.isCancelled else { return }
                    self?.onWorkspaceState?(state)
                }
            })
        }
    }

    private func resetRemoteStores() {
        remoteObservations.forEach { $0.cancel() }
        remoteObservations.removeAll()
        remoteSession?.dispatch(intent: RemoteSessionIntentStop.shared)
        remoteWorkspace?.dispatch(intent: RemoteWorkspaceIntentStop.shared)
        remoteSession = nil
        remoteWorkspace = nil
        remoteTargetKey = nil
    }

    func stop() {
        observations.forEach { $0.cancel() }
        observations.removeAll()
        resetRemoteStores()
        pairing.dispatch(intent: PairingIntentDisconnect.shared)
        account.stop()
        generalChat.stop()
    }
}

private extension ComposerAttachment {
    var coreImage: ComposerImage {
        ComposerImage(id: id, dataUrl: dataURL, mimeType: mimeType)
    }
}
