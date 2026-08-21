import BitFunMobileCore
import Foundation

/// Swift owns presentation state; this adapter owns the shared KMP feature seam.
@MainActor
final class MobileCoreAdapter {
    private let scope: any CoroutineScope
    private let generalChat: GeneralChatStore
    private let pairing: PairingStore
    private let account: AccountStore
    private var remoteSession: RemoteSessionStore?
    private var observations: [Task<Void, Never>] = []

    var onState: ((GeneralChatUiState) -> Void)?
    var onPairingState: ((PairingUiState) -> Void)?
    var onAccountState: ((AccountUiState) -> Void)?
    var onRemoteState: ((RemoteSessionUiState) -> Void)?

    init(
        onState: ((GeneralChatUiState) -> Void)? = nil,
        onPairingState: ((PairingUiState) -> Void)? = nil,
        onAccountState: ((AccountUiState) -> Void)? = nil,
        onRemoteState: ((RemoteSessionUiState) -> Void)? = nil,
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
    }

    func updateDraft(_ text: String) {
        generalChat.dispatch(intent: GeneralChatIntentUpdateDraft(text: text))
    }

    func send() {
        generalChat.dispatch(intent: GeneralChatIntentSend.shared)
    }

    func submitPairing(url: String) {
        pairing.dispatch(intent: PairingIntentSubmit(pairingUrl: url))
    }

    func loginAccount(relayURL: String, username: String, password: String) {
        account.dispatch(intent: AccountIntentLogin(relayUrl: relayURL, username: username, password: password))
    }

    func selectAccountDevice(id: String) {
        account.dispatch(intent: AccountIntentSelectDevice(deviceId: id))
    }

    func disconnect() {
        pairing.dispatch(intent: PairingIntentDisconnect.shared)
        remoteSession?.dispatch(intent: RemoteSessionIntentStop.shared)
        remoteSession = nil
    }

    func sendRemote(sessionID: String, content: String) {
        remoteSession?.dispatch(intent: RemoteSessionIntentSendMessage(sessionId: sessionID, content: content))
    }

    private func startRemoteSessionStoreIfNeeded(paired: PairingUiStatePaired) {
        guard remoteSession == nil, let store = pairing.createSessionStore(scope: scope) else { return }
        remoteSession = store
        let flow = SkieSwiftStateFlow<RemoteSessionUiState>(store.state)
        onRemoteState?(flow.value)
        store.dispatch(intent: RemoteSessionIntentLoad.shared)
        observations.append(Task { [weak self] in
            for await state in flow {
                guard !Task.isCancelled else { return }
                self?.onRemoteState?(state)
            }
        })
    }

    private func startAccountRemoteSessionIfNeeded(ready: AccountUiStateReady) {
        guard remoteSession == nil, let store = account.createSessionStore(scope: scope) else { return }
        remoteSession = store
        let flow = SkieSwiftStateFlow<RemoteSessionUiState>(store.state)
        onRemoteState?(flow.value)
        store.dispatch(intent: RemoteSessionIntentLoad.shared)
        observations.append(Task { [weak self] in
            for await state in flow {
                guard !Task.isCancelled else { return }
                self?.onRemoteState?(state)
            }
        })
    }

    func stop() {
        observations.forEach { $0.cancel() }
        observations.removeAll()
        remoteSession?.stop()
        remoteSession = nil
        pairing.dispatch(intent: PairingIntentDisconnect.shared)
        account.stop()
        generalChat.stop()
    }
}
