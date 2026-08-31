import SwiftUI
import OSLog

struct RemoteCreateSessionView: View {
    @ObservedObject var model: MobileAppModel
    let onBack: () -> Void
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @StateObject private var speech = SpeechInputController()
    @State private var instruction = ""
    @State private var selectedWorkspacePath = ""
    @State private var selectedModelID: String?
    @State private var pickerKind: RemoteCreateSelectionKind? = ProcessInfo.processInfo.arguments.contains(
        "--remote-create-workspace-picker"
    ) ? .workspace : nil
    private let log = Logger(subsystem: "com.bitfun.mobile.ios", category: "remote-create-ui")

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Button(action: onBack) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 19, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 44, height: 44)
                        .background(BitFunTheme.card)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.localized("返回"))
                Spacer()
            }
            .frame(height: 78, alignment: .top)
            .padding(.leading, 18)
            .padding(.top, 14)

            Spacer(minLength: 12)

            if !model.remoteConnected {
                createStatus(message: model.localized("连接不可用，请重新连接"), retryTitle: model.localized("重试"), action: model.verifyRemoteConnection)
            } else if let error = model.remoteCreateError ?? model.remoteCreateDeviceError ??
                        (model.workspaceLoadFailed ? (model.coreErrorMessage ?? model.localized("工作区加载失败，请重试")) : nil) {
                createStatus(message: error, retryTitle: model.localized("重试"), action: retryCreate)
            }

            contextButton(
                kind: .device,
                icon: "desktopcomputer",
                label: deviceLabel,
                automationIdentifier: selectedDeviceAutomationIdentifier
            )
            contextButton(
                kind: .workspace,
                icon: selectedWorkspacePath.isEmpty ? "message" : "folder",
                label: model.workspaceLoading ? model.localized("正在加载工作区") : selectedWorkspaceName,
                automationIdentifier: selectedWorkspaceAutomationIdentifier
            )
            createComposer
        }
        .background(BitFunTheme.page)
        .overlayPreferenceValue(RemoteCreateSelectionAnchorKey.self) { anchors in
            GeometryReader { proxy in
                if horizontalSizeClass == .regular,
                   let kind = pickerKind,
                   let anchor = anchors[kind] {
                    let frame = proxy[anchor]
                    ZStack(alignment: .topLeading) {
                        BitFunTheme.transparent
                            .contentShape(Rectangle())
                            .onTapGesture { pickerKind = nil }
                        selectionContent(kind: kind, includeHeader: false)
                            .bitFunPopoverSurface()
                            .fixedSize(horizontal: false, vertical: true)
                            .position(
                                x: min(
                                    max(MobileDesignGeometry.popoverWidth / 2 + 8, frame.midX),
                                    proxy.size.width - MobileDesignGeometry.popoverWidth / 2 - 8
                                ),
                                y: max(120, frame.minY - selectionHeight(kind) / 2 - 8)
                            )
                    }
                }
            }
        }
        .sheet(item: compactPicker) { kind in
            selectionContent(kind: kind, includeHeader: true)
                .presentationDetents([.height(selectionHeight(kind))])
                .presentationDragIndicator(.visible)
        }
        .onAppear {
            if let selected = model.remoteWorkspaces.first(where: \.selected) {
                selectedWorkspacePath = selected.path
            }
            selectedModelID = model.modelOptions.first(where: \.selected)?.id ?? model.modelOptions.first?.id
        }
    }

    private var compactPicker: Binding<RemoteCreateSelectionKind?> {
        Binding(
            get: { horizontalSizeClass == .regular ? nil : pickerKind },
            set: { pickerKind = $0 }
        )
    }

    private var deviceLabel: String {
        if model.accountRefreshing || model.accountBusy { return model.localized("正在加载") }
        return model.accountDeviceName ?? model.localized("选择桌面设备")
    }

    private var selectedWorkspaceName: String {
        guard !selectedWorkspacePath.isEmpty else { return model.localized("对话") }
        return model.remoteWorkspaces.first(where: { $0.path == selectedWorkspacePath })?.name
            ?? selectedWorkspacePath
    }

    private var selectedModel: ComposerModelOption? {
        model.modelOptions.first(where: { $0.id == selectedModelID }) ?? model.modelOptions.first
    }

    private var selectedDeviceAutomationIdentifier: String {
        guard let deviceID = model.accountSelectedDeviceID,
              !deviceID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return "remoteCreate.device.unselected"
        }
        return "remoteCreate.device.\(deviceID)"
    }

    private var selectedWorkspaceAutomationIdentifier: String {
        selectedWorkspacePath.isEmpty
            ? "remoteCreate.workspace.chat"
            : "remoteCreate.workspace.\(selectedWorkspacePath)"
    }

    private func contextButton(
        kind: RemoteCreateSelectionKind,
        icon: String,
        label: String,
        automationIdentifier: String
    ) -> some View {
        Button { pickerKind = kind } label: {
            HStack(spacing: 13) {
                Image(systemName: icon)
                    .font(.system(size: 20, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(width: 26, height: 26)
                Text(label)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Image(systemName: pickerKind == kind ? "chevron.up" : "chevron.down")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(BitFunTheme.muted)
                Spacer(minLength: 0)
            }
            .frame(height: 48)
            .padding(.horizontal, 28)
        }
        .buttonStyle(.plain)
        .disabled(model.busy || model.remoteCreateSubmitting || model.accountBusy)
        .accessibilityIdentifier(automationIdentifier)
        .accessibilityLabel(model.localized(kind.accessibilityLabelKey))
        .accessibilityValue(label)
        .accessibilityHint(model.localized(kind.accessibilityHintKey))
        .anchorPreference(key: RemoteCreateSelectionAnchorKey.self, value: .bounds) {
            [kind: $0]
        }
    }

    private var createComposer: some View {
        VStack(spacing: 2) {
            TextField(
                "",
                text: $instruction,
                prompt: Text(model.localized(speech.isListening ? "正在聆听" : "告诉 BitFun 要做什么"))
                    .foregroundColor(speech.isListening ? BitFunTheme.statusSuccess : BitFunTheme.muted),
                axis: .vertical
            )
            .font(MobileDesignTypography.bodyLarge.font)
            .lineLimit(1...4)
            .padding(.horizontal, 6)
            .frame(minHeight: MobileDesignGeometry.composerExpandedInputRowHeight)

            HStack(spacing: 8) {
                if let selectedModel {
                    Button { pickerKind = .model } label: {
                        HStack(spacing: 4) {
                            Text(selectedModel.primaryLabel)
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(BitFunTheme.ink)
                                .lineLimit(1)
                            Image(systemName: pickerKind == .model ? "chevron.up" : "chevron.down")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(BitFunTheme.muted)
                        }
                        .frame(height: 34)
                    }
                    .buttonStyle(.plain)
                    .anchorPreference(key: RemoteCreateSelectionAnchorKey.self, value: .bounds) {
                        [.model: $0]
                    }
                    .accessibilityLabel(model.localized(RemoteCreateSelectionKind.model.accessibilityLabelKey))
                    .accessibilityValue(selectedModel.primaryLabel)
                    .accessibilityHint(model.localized(RemoteCreateSelectionKind.model.accessibilityHintKey))
                }
                Spacer(minLength: 0)
                Button(action: primaryAction) {
                    Group {
                        if model.remoteCreateSubmitting {
                            ProgressView()
                                .tint(BitFunTheme.contentOnAction)
                        } else {
                            Image(systemName: instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                ? (speech.isListening ? "stop.fill" : "mic.fill")
                                : "arrow.up")
                            .font(.system(size: 17, weight: .semibold))
                            .foregroundStyle(canSubmit ? BitFunTheme.contentOnAction : BitFunTheme.ink)
                        }
                    }
                    .frame(
                        width: MobileDesignGeometry.composerActionSize,
                        height: MobileDesignGeometry.composerActionSize
                    )
                    .background(canSubmit ? BitFunTheme.accent : BitFunTheme.soft)
                    .clipShape(Circle())
                }
                .buttonStyle(.plain)
                // A session-list refresh is not an active turn and must not disable creation here.
                .disabled(model.remoteCreateSubmitting || !model.remoteConnected)
                .accessibilityLabel(model.remoteCreateSubmitting ? model.localized("正在加载") : model.localized("发送"))
            }
            .frame(height: MobileDesignGeometry.composerExpandedActionRowHeight)
        }
        .padding(.horizontal, 8)
        .padding(.top, 4)
        .padding(.bottom, 2)
        .frame(minHeight: MobileDesignGeometry.composerExpandedHeight)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.composerExpandedRadius))
        .shadow(color: BitFunTheme.shadowSubtle, radius: 10, y: 2)
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .padding(.top, 8)
        .padding(.bottom, 14)
    }

    private var canSubmit: Bool {
        !instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            model.remoteConnected && !model.remoteCreateSubmitting
    }

    private func createStatus(message: String, retryTitle: String, action: @escaping () -> Void) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(BitFunTheme.statusDanger)
            Text(message)
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.ink)
                .multilineTextAlignment(.leading)
            Spacer(minLength: 4)
            Button(retryTitle, action: action)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(BitFunTheme.accent)
                .disabled(model.remoteCreateSubmitting || model.accountBusy)
                .accessibilityLabel(retryTitle)
                .accessibilityHint(model.localized("选择"))
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .background(BitFunTheme.soft)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(model.localized("状态")): \(message)")
    }

    private func retryCreate() {
        if model.remoteCreateError != nil {
            model.createRemoteSession(
                agentType: selectedWorkspacePath.isEmpty ? "Claw" : "code",
                title: "",
                instruction: instruction,
                modelID: selectedModelID,
                workspacePath: selectedWorkspacePath.isEmpty ? nil : selectedWorkspacePath
            )
        } else if model.workspaceLoadFailed {
            model.retryRemoteWorkspaces()
        } else {
            model.refreshRemoteDevices()
        }
    }

    private func primaryAction() {
        let value = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        log.info("Remote create primary action invoked: hasInput=\(!value.isEmpty, privacy: .public) connected=\(model.remoteConnected, privacy: .public) busy=\(model.busy, privacy: .public) submitting=\(model.remoteCreateSubmitting, privacy: .public)")
        if !value.isEmpty {
            guard canSubmit else {
                log.error("Remote create primary action blocked by model state: connected=\(model.remoteConnected, privacy: .public) busy=\(model.busy, privacy: .public) submitting=\(model.remoteCreateSubmitting, privacy: .public)")
                return
            }
            model.createRemoteSession(
                agentType: selectedWorkspacePath.isEmpty ? "Claw" : "code",
                title: "",
                instruction: value,
                modelID: selectedModelID,
                workspacePath: selectedWorkspacePath.isEmpty ? nil : selectedWorkspacePath
            )
            return
        }
        if speech.isListening {
            speech.stop()
            return
        }
        speech.start(
            localeIdentifier: model.appLanguage == .simplifiedChinese ? "zh-CN" : "en-US",
            onPartial: { instruction = $0 },
            onFailure: { model.showToast(model.localized($0)) }
        )
    }

    @ViewBuilder
    private func selectionContent(kind: RemoteCreateSelectionKind, includeHeader: Bool) -> some View {
        VStack(spacing: 0) {
            if includeHeader {
                BitFunSelectionHeader(title: model.localized(kind.titleKey), onClose: { pickerKind = nil })
            }
            ScrollView(showsIndicators: false) {
                VStack(spacing: 0) {
                    switch kind {
                    case .device:
                        ForEach(model.accountDevices) { device in
                            selectionRow(
                                kind: .device,
                                icon: "desktopcomputer",
                                title: device.name.isEmpty ? device.id : device.name,
                                subtitle: model.localized(device.online ? "在线" : "离线"),
                                selected: device.selected,
                                enabled: device.online || device.selected
                            ) {
                                pickerKind = nil
                                selectedWorkspacePath = ""
                                model.selectRemoteDevice(device)
                            }
                        }
                    case .workspace:
                        selectionRow(
                            kind: .workspace,
                            icon: "message",
                            title: model.localized("对话"),
                            subtitle: "",
                            selected: selectedWorkspacePath.isEmpty,
                            enabled: true
                        ) {
                            selectedWorkspacePath = ""
                            pickerKind = nil
                            if let assistant = model.remoteAssistants.first {
                                model.selectRemoteAssistant(assistant)
                            }
                        }
                        ForEach(model.remoteWorkspaces) { workspace in
                            selectionRow(
                                kind: .workspace,
                                icon: "folder",
                                title: workspace.name,
                                subtitle: workspace.path,
                                selected: workspace.path == selectedWorkspacePath,
                                enabled: true
                            ) {
                                selectedWorkspacePath = workspace.path
                                pickerKind = nil
                                model.selectRemoteWorkspace(workspace)
                            }
                        }
                    case .model:
                        if model.modelOptions.isEmpty {
                            Text(model.localized("暂无可用模型"))
                                .font(.system(size: 13))
                                .foregroundStyle(BitFunTheme.muted)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(18)
                                .accessibilityElement()
                                .accessibilityLabel(model.localized("暂无可用模型"))
                        } else {
                            ForEach(model.modelOptions) { option in
                                selectionRow(
                                    kind: .model,
                                    icon: option.source == "LOCAL" ? "gearshape" : "cloud",
                                    title: option.primaryLabel,
                                    subtitle: option.secondaryLabel,
                                    selected: option.id == selectedModelID,
                                    enabled: true
                                ) {
                                    selectedModelID = option.id
                                    pickerKind = nil
                                }
                            }
                        }
                    }
                }
            }
        }
        .background(BitFunTheme.card)
    }

    private func selectionRow(
        kind: RemoteCreateSelectionKind,
        icon: String,
        title: String,
        subtitle: String,
        selected: Bool,
        enabled: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Image(systemName: selected ? "checkmark.circle" : "circle")
                    .font(.system(size: 19))
                    .foregroundStyle(selected ? BitFunTheme.ink : BitFunTheme.transparent)
                    .frame(width: 20)
                Image(systemName: icon)
                    .font(.system(size: 19, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(width: 24)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .lineLimit(1)
                    if !subtitle.isEmpty {
                        Text(subtitle)
                            .font(.system(size: 11))
                            .foregroundStyle(BitFunTheme.muted)
                            .lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
            }
            .frame(minHeight: 58)
            .padding(.horizontal, 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.55)
        .accessibilityLabel("\(model.localized(kind.accessibilityLabelKey)): \(title)")
        .accessibilityValue(subtitle)
        .accessibilityHint(model.localized(kind.accessibilityHintKey))
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private func selectionHeight(_ kind: RemoteCreateSelectionKind) -> CGFloat {
        let count: Int
        switch kind {
        case .device: count = max(1, model.accountDevices.count)
        case .workspace: count = max(1, model.remoteWorkspaces.count + 1)
        case .model: count = max(1, model.modelOptions.count)
        }
        let header: CGFloat = horizontalSizeClass == .regular ? 16 : MobileDesignGeometry.sheetHeaderHeight
        return min(440, header + CGFloat(count * 64) + 24)
    }
}

enum RemoteCreateSelectionKind: String, Identifiable, Hashable {
    case device
    case workspace
    case model

    var id: String { rawValue }

    /// Stable localization keys owned by the mobile W3 catalog. The view is
    /// responsible for resolving them with the active app language.
    var titleKey: String {
        switch self {
        case .device: return "桌面设备"
        case .workspace: return "工作区"
        case .model: return "选择模型"
        }
    }

    var accessibilityLabelKey: String {
        switch self {
        case .device: return "桌面设备"
        case .workspace: return "工作区"
        case .model: return "选择模型"
        }
    }

    var accessibilityHintKey: String { "选择" }
}

struct RemoteCreateSelectionAnchorKey: PreferenceKey {
    static var defaultValue: [RemoteCreateSelectionKind: Anchor<CGRect>] = [:]

    static func reduce(
        value: inout [RemoteCreateSelectionKind: Anchor<CGRect>],
        nextValue: () -> [RemoteCreateSelectionKind: Anchor<CGRect>]
    ) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}
