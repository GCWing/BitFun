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

struct ChatSession: Identifiable, Equatable {
    let id: UUID
    var title: String
    var updatedLabel: String
    var pinned: Bool = false
}

@MainActor
final class MobileAppModel: ObservableObject {
    @Published var surface: MobileSurface = .local
    @Published var sessions: [ChatSession]
    @Published var remoteSessions: [ChatSession] = []
    @Published var selectedSessionID: UUID
    @Published var messages: [ChatMessage]
    @Published var draft = ""
    @Published var drawerOpen = false
    @Published var settingsOpen = false
    @Published var connectionPhase: ConnectionPhase = .connected
    @Published var isSending = false
    @Published var remoteConnected = false
    @Published var remoteSessionSelected = false
    @Published var localSessionSelected = false
    @Published var pairingSheetOpen = false
    @Published var pairingBusy = false
    @Published var pairingError: String?
    @Published var coreErrorMessage: String?
    @Published var accountUser: String?
    @Published var accountBusy = false
    @Published var accountDeviceName: String?
    @Published var accountDeviceCount = 0

    private var coreAdapter: MobileCoreAdapter?

    init(sessions: [ChatSession], selectedSessionID: UUID, messages: [ChatMessage]) {
        self.sessions = sessions
        self.selectedSessionID = selectedSessionID
        self.messages = messages
        self.coreAdapter = nil
        self.coreAdapter = MobileCoreAdapter(
            onState: { [weak self] state in self?.apply(coreState: state) },
            onPairingState: { [weak self] state in self?.apply(pairingState: state) },
            onAccountState: { [weak self] state in self?.apply(accountState: state) },
            onRemoteState: { [weak self] state in self?.apply(remoteState: state) },
        )
    }

    static let preview: MobileAppModel = {
        let first = ChatSession(id: UUID(), title: "你好", updatedLabel: "刚刚")
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
        if arguments.contains("--remote") {
            model.surface = .remote
        }
        if arguments.contains("--connected") {
            model.configureConnectedPreview()
        }
        if let relay = arguments.value(after: "--relay-url"),
           let username = arguments.value(after: "--username"),
           let password = arguments.value(after: "--password") {
            model.loginAccount(relayURL: relay, username: username, password: password)
        }
        if arguments.contains("--drawer") {
            model.drawerOpen = true
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
        guard !value.isEmpty, !isSending else { return }
        if surface == .local {
            localSessionSelected = true
            if selectedSession == nil, let first = sessions.first {
                selectedSessionID = first.id
            }
        }
        messages.append(ChatMessage(id: UUID(), role: .user, text: value))
        draft = ""
        isSending = true
        coreAdapter?.updateDraft(value)
        coreAdapter?.send()
    }

    func select(_ session: ChatSession) {
        selectedSessionID = session.id
        if surface == .remote {
            remoteSessionSelected = true
        } else {
            localSessionSelected = true
        }
        drawerOpen = false
    }

    func switchSurface(_ next: MobileSurface) {
        surface = next
        drawerOpen = false
    }

    func connectRemote() {
        pairingError = nil
        pairingSheetOpen = true
    }

    private func configureConnectedPreview() {
        surface = .remote
        remoteConnected = true
        connectionPhase = .connected
        remoteSessionSelected = true
        accountDeviceName = "DESKTOP-KM3L4UI"
        let session = ChatSession(id: UUID(), title: "你好", updatedLabel: "刚刚")
        remoteSessions = [session]
        selectedSessionID = session.id
        messages = [
            ChatMessage(id: UUID(), role: .user, text: "你好"),
            ChatMessage(id: UUID(), role: .assistant, text: "这是 BitFun 的远程会话预览。"),
        ]
    }

    func submitPairing(url: String) {
        pairingError = nil
        pairingBusy = true
        coreAdapter?.submitPairing(url: url)
    }

    func loginAccount(relayURL: String, username: String, password: String) {
        accountBusy = true
        coreErrorMessage = nil
        coreAdapter?.loginAccount(relayURL: relayURL, username: username, password: password)
    }

    func sendRemote() {
        let value = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty, let sessionID = visibleSessions.first(where: { $0.id == selectedSessionID })?.id else { return }
        draft = ""
        isSending = true
        coreAdapter?.sendRemote(sessionID: sessionID.uuidString, content: value)
    }

    func syncDraftToCore() {
        coreAdapter?.updateDraft(draft)
    }

    private func apply(coreState state: GeneralChatUiState) {
        if !state.sessions.isEmpty {
            sessions = state.sessions.map { session in
                ChatSession(
                    id: UUID(uuidString: session.id) ?? UUID(),
                    title: session.title.isEmpty ? "未命名会话" : session.title,
                    updatedLabel: session.updatedAt,
                    pinned: session.pinned,
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
        }
        if draft != state.draft { draft = state.draft }
        isSending = state.busy
        if let failure = state.failure {
            coreErrorMessage = failure.name
        } else {
            coreErrorMessage = nil
        }
    }

    private func apply(pairingState state: PairingUiState) {
        pairingBusy = state is PairingUiStateConnecting
        if let failed = state as? PairingUiStateFailed {
            pairingBusy = false
            pairingError = pairingErrorMessage(failed.failure)
        } else if state is PairingUiStatePaired {
            pairingBusy = false
            pairingError = nil
            remoteConnected = true
            surface = .remote
            connectionPhase = .connected
            pairingSheetOpen = false
        }
    }

    private func apply(accountState state: AccountUiState) {
        accountBusy = state is AccountUiStateSigningIn
        if let ready = state as? AccountUiStateReady {
            accountBusy = false
            accountUser = ready.username
            accountDeviceName = ready.selectedDeviceName
            accountDeviceCount = ready.devices.count
            if ready.selectedDeviceId == nil,
               let target = ready.devices.first(where: { $0.online }) {
                accountBusy = true
                coreAdapter?.selectAccountDevice(id: target.id)
                return
            }
            remoteConnected = true
            surface = .remote
            connectionPhase = .connected
        } else if let failed = state as? AccountUiStateFailed {
            accountBusy = false
            coreErrorMessage = accountErrorMessage(failed.reason.name)
            connectionPhase = .disconnected
        }
    }

    private func apply(remoteState state: RemoteSessionUiState) {
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
                id: UUID(uuidString: session.id) ?? UUID(),
                title: session.title.isEmpty ? "未命名会话" : session.title,
                updatedLabel: session.updatedAt,
            )
        }
        if let selected = ready.selectedSessionId, let id = UUID(uuidString: selected) {
            selectedSessionID = id
        }
        remoteSessionSelected = ready.selectedSessionId != nil
        isSending = ready.busy
        if let timeline = ready.timeline {
            let allMessages = timeline.persistedMessages + timeline.optimisticMessages
            messages = allMessages.map { message in
                ChatMessage(
                    id: UUID(uuidString: message.id) ?? UUID(),
                    role: message.role.lowercased() == "user" ? .user : .assistant,
                    text: message.text,
                )
            }
        }
    }

    private func pairingErrorMessage(_ failure: PairingFailure) -> String {
        if let remote = failure.remoteMessage?.trimmingCharacters(in: .whitespacesAndNewlines), !remote.isEmpty {
            return remote
        }
        switch failure.reason.name {
        case "PAIRING_LINK_EMPTY", "PAIRING_LINK_INCOMPLETE", "PAIRING_LINK_UNDECODABLE", "PAIRING_LINK_KEY_UNUSABLE":
            return "连接链接无效，请重新扫描或粘贴桌面端链接"
        case "ACCOUNT_USERNAME_REQUIRED":
            return "请输入桌面端账号"
        case "ACCOUNT_PASSWORD_REQUIRED":
            return "请输入桌面端密码"
        case "REJECTED", "DESKTOP_REJECTED":
            return "桌面端拒绝了这次连接"
        case "ROOM_NOT_FOUND":
            return "找不到桌面端房间，请确认桌面端仍在等待连接"
        case "RATE_LIMITED", "TOO_MANY_ATTEMPTS":
            return "尝试次数过多，请稍后再试"
        case "RELAY_UNAVAILABLE", "NETWORK_UNREACHABLE":
            return "网络不可用，请检查手机与桌面端的网络"
        case "TIMEOUT":
            return "连接超时，请重新尝试"
        case "PROTOCOL_MISMATCH":
            return "桌面端版本不兼容，请升级后重试"
        default:
            return "连接失败，请检查桌面端链接"
        }
    }

    private func accountErrorMessage(_ reason: String) -> String {
        switch reason {
        case "INVALID_CREDENTIALS", "UNAUTHORIZED":
            return "账号或密码错误"
        case "NETWORK":
            return "网络不可用，请检查 relay 地址"
        case "TIMEOUT":
            return "登录超时，请稍后重试"
        default:
            return "登录失败，请检查账号、密码和 relay 地址"
        }
    }
}

private extension Array where Element == String {
    func value(after flag: String) -> String? {
        guard let position = firstIndex(of: flag), position < self.index(before: endIndex) else { return nil }
        return self[self.index(after: position)]
    }
}
