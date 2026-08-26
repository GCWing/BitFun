import AVFoundation
import BitFunMobileCore
import SwiftUI
import UniformTypeIdentifiers

struct MobileShellView: View {
    @ObservedObject var model: MobileAppModel
    @State private var wideSidebarCollapsed = false
    @State private var sessionActionsOpen = false
    @State private var sidebarActionSession: ChatSession?

    var body: some View {
        GeometryReader { proxy in
            adaptiveSurface(viewportWidth: proxy.size.width, viewportHeight: proxy.size.height)
        }
        .overlayPreferenceValue(SessionActionsAnchorKey.self) { anchor in
            GeometryReader { proxy in
                if sessionActionsOpen, let anchor {
                    let frame = proxy[anchor]
                    ZStack(alignment: .topLeading) {
                        Color.clear
                            .contentShape(Rectangle())
                            .onTapGesture { sessionActionsOpen = false }
                        ConversationActionsPopover(
                            model: model,
                            onDismiss: { sessionActionsOpen = false }
                        )
                        .offset(
                            x: min(
                                max(8, frame.maxX - MobileDesignGeometry.popoverWidth),
                                proxy.size.width - MobileDesignGeometry.popoverWidth - 8
                            ),
                            y: frame.maxY + 8
                        )
                        .transition(
                            .offset(x: 8, y: -8).combined(with: .opacity)
                        )
                    }
                }
            }
        }
        .overlayPreferenceValue(SidebarSessionActionsAnchorKey.self) { anchors in
            GeometryReader { proxy in
                if let session = sidebarActionSession,
                   let anchor = anchors[session.id] {
                    let frame = proxy[anchor]
                    let remote = model.surface == .remote
                    ZStack(alignment: .topLeading) {
                        Color.clear
                            .contentShape(Rectangle())
                            .onTapGesture { sidebarActionSession = nil }
                        SessionActionSurface(
                            model: model,
                            session: session,
                            presentation: .popover,
                            canViewDetails: true,
                            canArchive: !remote,
                            canExport: !remote,
                            canDelete: true,
                            onViewDetails: {
                                sidebarActionSession = nil
                                DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
                                    model.showSessionDetails(session)
                                }
                            },
                            onArchive: { if !remote { model.archiveLocalSession(session) } },
                            onExport: { if !remote { model.exportLocalSession(session) } },
                            onDelete: {
                                if remote { model.deleteRemoteSession(session) }
                                else { model.deleteLocalSession(session) }
                            },
                            onClose: { sidebarActionSession = nil }
                        )
                        .position(
                            x: frame.maxX + 6 + 150,
                            y: min(max(frame.midY, 170), proxy.size.height - 170)
                        )
                    }
                }
            }
        }
        .animation(.easeOut(duration: 0.24), value: model.drawerOpen)
        .animation(.easeInOut(duration: 0.22), value: wideSidebarCollapsed)
        .overlay(alignment: .bottom) {
            if let message = model.toastMessage {
                Text(message)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Color.white)
                    .padding(.horizontal, 16)
                    .frame(minHeight: 38)
                    .background(Color.black.opacity(0.82))
                    .clipShape(Capsule())
                    .padding(.bottom, 86)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.easeOut(duration: 0.18), value: model.toastMessage)
        .fileExporter(
            isPresented: $model.downloadExporterOpen,
            document: MobileDownloadDocument(data: model.pendingDownload?.data ?? Data()),
            contentType: model.pendingDownload.flatMap { UTType(mimeType: $0.mimeType) } ?? .data,
            defaultFilename: model.pendingDownload?.name ?? "download"
        ) { result in
            switch result {
            case .success: model.finishDownloadExport(success: true)
            case .failure: model.finishDownloadExport(success: false)
            }
        }
        .fileExporter(
            isPresented: $model.generalExportOpen,
            document: MobileDownloadDocument(data: model.generalExportData),
            contentType: UTType(filenameExtension: "md") ?? .plainText,
            defaultFilename: model.generalExportName
        ) { _ in
            model.finishGeneralExport()
        }
    }

    @ViewBuilder
    private func adaptiveSurface(viewportWidth: CGFloat, viewportHeight: CGFloat) -> some View {
        let width = Int32(max(0, viewportWidth.rounded(.down)))
        let height = Int32(max(0, viewportHeight.rounded(.down)))
        let layoutPolicy = ConversationLayoutPolicy.shared
        let wide = layoutPolicy.useMasterDetail(
            viewportWidth: width,
            wideViewportMatched: width >= layoutPolicy.MD_MIN_WIDTH,
            isFolded: false,
            creases: [],
            isExpandedFoldable: false,
            isHover: false
        )
        let geometry = layoutPolicy.resolveWideGeometry(viewportWidth: width, creases: [])
        let adaptiveInput = AdaptiveLayoutInput(
            viewportWidth: width,
            viewportHeight: height,
            isFolded: false,
            isExpandedFoldable: false,
            isHoverOperate: false,
            wideLayoutMatched: width >= layoutPolicy.MD_MIN_WIDTH,
            verticalCreases: [],
            horizontalCreases: [],
            isRtl: false
        )
        let settingsPlacement = SettingsPlacementPolicy.shared.resolve(
            input: adaptiveInput,
            kind: .settings
        )
        let connectPlacement = SettingsPlacementPolicy.shared.resolve(
            input: adaptiveInput,
            kind: .connect
        )
        let sessionDetailsPlacement = SettingsPlacementPolicy.shared.resolve(
            input: adaptiveInput,
            kind: .sessionDetails
        )
        let remoteViewSettingsPlacement = SettingsPlacementPolicy.shared.resolve(
            input: adaptiveInput,
            kind: .remoteViewSettings
        )
        let previewLayout = FilePreviewPlacementPolicy.shared.resolveLayout(
            previewVisible: model.filePreview != nil,
            largeScreenLayout: wide,
            viewportWidth: width,
            creases: [],
            preferredMasterWidth: geometry.masterPaneWidth
        )
        let previewInPane = model.filePreview != nil &&
            previewLayout.placement != FilePreviewPlacement.compactFullPage
        let previewForSheet = Binding<MobileFilePreview?>(
            get: { previewInPane ? nil : model.filePreview },
            set: { value in
                if value == nil { model.dismissFilePreview() }
            }
        )
        let focusSplit = previewLayout.placement == FilePreviewPlacement.wideFocusSplit
        let triplePane = previewLayout.placement == FilePreviewPlacement.wideTriplePane
        let sidebarVisible = wide && !wideSidebarCollapsed && !focusSplit
        let sidebarWidth = triplePane
            ? CGFloat(previewLayout.masterPaneWidth)
            : CGFloat(geometry.masterPaneWidth)

        ZStack(alignment: .leading) {
            HStack(spacing: 0) {
                if sidebarVisible {
                    SidebarView(
                        model: model,
                        permanent: true,
                        onCollapse: { wideSidebarCollapsed = true },
                        onPermanentActions: { sidebarActionSession = $0 }
                    )
                    .frame(width: sidebarWidth)
                    paneSeparator(width: triplePane ? CGFloat(previewLayout.masterConversationGap) : 0)
                }

                conversationSurface(
                    sidebarAction: sidebarVisible ? nil : {
                        if wide {
                            if focusSplit { model.dismissFilePreview() }
                            wideSidebarCollapsed = false
                        } else {
                            model.drawerOpen = true
                        }
                    },
                    sidebarActionLabel: wide ? "展开侧栏" : "打开侧栏"
                )
                .frame(width: previewInPane ? CGFloat(previewLayout.conversationPaneWidth) : nil)

                if previewInPane, let preview = model.filePreview {
                    paneSeparator(width: CGFloat(previewLayout.conversationPreviewGap))
                    RemoteFilePreviewSheet(model: model, preview: preview, embedded: true)
                        .frame(width: CGFloat(previewLayout.previewPaneWidth))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)

            if !sidebarVisible && model.drawerOpen {
                Color.black.opacity(0.24)
                    .ignoresSafeArea()
                    .onTapGesture { model.drawerOpen = false }
                SidebarView(model: model)
                    .transition(.move(edge: .leading).combined(with: .opacity))
                    .shadow(color: .black.opacity(0.18), radius: 26, x: 10, y: 0)
            }
        }
        .sheet(item: previewForSheet, onDismiss: model.dismissFilePreview) { preview in
            RemoteFilePreviewSheet(model: model, preview: preview)
        }
        .bitFunAdaptiveModal(
            isPresented: $model.settingsOpen,
            placement: settingsPlacement
        ) {
            SettingsView(model: model)
        }
        .bitFunAdaptiveModal(
            isPresented: $model.remoteControlSettingsOpen,
            placement: settingsPlacement
        ) {
            RemoteControlSettingsView(model: model)
        }
        .bitFunAdaptiveModal(
            isPresented: $model.remoteViewSettingsOpen,
            placement: remoteViewSettingsPlacement
        ) {
            RemoteViewSettingsView(model: model)
        }
        .bitFunAdaptiveModal(
            isPresented: $model.pairingSheetOpen,
            placement: connectPlacement,
            onDismiss: model.dismissPairing
        ) {
            PairingSheet(model: model)
        }
        .bitFunAdaptiveModal(
            isPresented: $model.accountSheetOpen,
            placement: settingsPlacement
        ) {
            AccountSettingsView(model: model)
        }
        .bitFunAdaptiveModal(
            isPresented: Binding(
                get: { model.sessionDetails != nil },
                set: { if !$0 { model.dismissSessionDetails() } }
            ),
            placement: sessionDetailsPlacement
        ) {
            if let session = model.sessionDetails {
                SessionDetailsView(
                    model: model,
                    session: session,
                    onClose: model.dismissSessionDetails
                )
            }
        }
        .onChange(of: wide) { isWide in
            if !isWide { wideSidebarCollapsed = false }
        }
    }

    @ViewBuilder
    private func conversationSurface(
        sidebarAction: (() -> Void)?,
        sidebarActionLabel: String
    ) -> some View {
        Group {
            if model.remoteCreateOpen {
                RemoteCreateSessionView(
                    model: model,
                    onBack: { model.remoteCreateOpen = false }
                )
            } else {
                conversationContent(
                    sidebarAction: sidebarAction,
                    sidebarActionLabel: sidebarActionLabel
                )
            }
        }
        .background(BitFunTheme.page)
        .ignoresSafeArea(.keyboard, edges: .bottom)
    }

    private func conversationContent(
        sidebarAction: (() -> Void)?,
        sidebarActionLabel: String
    ) -> some View {
        VStack(spacing: 0) {
            ConversationHeader(
                model: model,
                actionsOpen: $sessionActionsOpen,
                sidebarAction: sidebarAction,
                sidebarActionLabel: sidebarActionLabel
            )
            if model.connectionPhase != .connected {
                ConnectionStatusBar(
                    phase: model.connectionPhase,
                    detail: model.coreErrorMessage,
                    onRetry: model.verifyRemoteConnection
                )
            }
            if model.surface == .remote && !model.remoteConnected {
                RemoteHomeView(model: model)
                ComposerBar(model: model)
            } else if model.surface == .remote && !model.remoteSessionSelected {
                RemoteConnectedHomeView(model: model)
                ComposerBar(model: model)
            } else if model.surface == .local && !model.localSessionSelected {
                LocalHomeView(model: model)
                ComposerBar(model: model)
            } else {
                ChatTimelineView(model: model)
                ComposerBar(model: model)
            }
        }
    }

    @ViewBuilder
    private func paneSeparator(width: CGFloat) -> some View {
        if width > 0 {
            Rectangle().fill(BitFunTheme.line).frame(width: width)
        }
    }
}

private struct RemoteCreateSessionView: View {
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

            if horizontalSizeClass == .regular, !model.accountDevices.isEmpty {
                contextButton(
                    kind: .device,
                    icon: "desktopcomputer",
                    label: model.accountDeviceName ?? model.localized("选择桌面设备")
                )
            }
            contextButton(
                kind: .workspace,
                icon: selectedWorkspacePath.isEmpty ? "message" : "folder",
                label: selectedWorkspaceName
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
                        Color.clear
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

    private var selectedWorkspaceName: String {
        guard !selectedWorkspacePath.isEmpty else { return model.localized("对话") }
        return model.remoteWorkspaces.first(where: { $0.path == selectedWorkspacePath })?.name
            ?? selectedWorkspacePath
    }

    private var selectedModel: ComposerModelOption? {
        model.modelOptions.first(where: { $0.id == selectedModelID }) ?? model.modelOptions.first
    }

    private func contextButton(kind: RemoteCreateSelectionKind, icon: String, label: String) -> some View {
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
        .disabled(model.busy)
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
                    .foregroundColor(speech.isListening ? BitFunTheme.green : BitFunTheme.muted),
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
                }
                Spacer(minLength: 0)
                Button(action: primaryAction) {
                    Image(systemName: instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? (speech.isListening ? "stop.fill" : "mic.fill")
                        : "arrow.up")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(canSubmit ? Color.white : BitFunTheme.ink)
                        .frame(
                            width: MobileDesignGeometry.composerActionSize,
                            height: MobileDesignGeometry.composerActionSize
                        )
                        .background(canSubmit ? BitFunTheme.accent : BitFunTheme.soft)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .disabled(model.busy || !model.remoteConnected)
            }
            .frame(height: MobileDesignGeometry.composerExpandedActionRowHeight)
        }
        .padding(.horizontal, 8)
        .padding(.top, 4)
        .padding(.bottom, 2)
        .frame(minHeight: MobileDesignGeometry.composerExpandedHeight)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.composerExpandedRadius))
        .shadow(color: .black.opacity(0.05), radius: 10, y: 2)
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .padding(.top, 8)
        .padding(.bottom, 14)
    }

    private var canSubmit: Bool {
        !instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            model.remoteConnected && !model.busy
    }

    private func primaryAction() {
        let value = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        if !value.isEmpty {
            guard canSubmit else { return }
            model.createRemoteSession(
                agentType: selectedWorkspacePath.isEmpty ? "Claw" : "code",
                title: "",
                instruction: value,
                modelID: selectedModelID
            )
            instruction = ""
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
                BitFunSelectionHeader(title: kind.title, onClose: { pickerKind = nil })
            }
            ScrollView(showsIndicators: false) {
                VStack(spacing: 0) {
                    switch kind {
                    case .device:
                        ForEach(model.accountDevices) { device in
                            selectionRow(
                                icon: "desktopcomputer",
                                title: device.name.isEmpty ? device.id : device.name,
                                subtitle: model.localized(device.online ? "在线" : "离线"),
                                selected: device.selected,
                                enabled: device.online || device.selected
                            ) {
                                pickerKind = nil
                                model.selectRemoteDevice(device)
                            }
                        }
                    case .workspace:
                        selectionRow(
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
                        } else {
                            ForEach(model.modelOptions) { option in
                                selectionRow(
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
                    .foregroundStyle(selected ? BitFunTheme.ink : Color.clear)
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

private enum RemoteCreateSelectionKind: String, Identifiable, Hashable {
    case device
    case workspace
    case model

    var id: String { rawValue }
    var title: String {
        switch self {
        case .device: return "桌面设备"
        case .workspace: return "工作区"
        case .model: return "选择模型"
        }
    }
}

private struct RemoteCreateSelectionAnchorKey: PreferenceKey {
    static var defaultValue: [RemoteCreateSelectionKind: Anchor<CGRect>] = [:]

    static func reduce(
        value: inout [RemoteCreateSelectionKind: Anchor<CGRect>],
        nextValue: () -> [RemoteCreateSelectionKind: Anchor<CGRect>]
    ) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}

private struct MobileDownloadDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.data] }
    let data: Data

    init(data: Data) {
        self.data = data
    }

    init(configuration: ReadConfiguration) throws {
        data = configuration.file.regularFileContents ?? Data()
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}

private struct RemoteFilePreviewSheet: View {
    @ObservedObject var model: MobileAppModel
    let preview: MobileFilePreview
    var embedded = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: preview.imageData == nil ? "doc.text" : "photo")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(MobileDesignColors.fileLink)
                    .frame(width: 34, height: 34)
                    .background(MobileDesignColors.fileLink.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: 9))
                Text(preview.name)
                    .font(MobileDesignTypography.titleSmall.font)
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Spacer()
                Button {
                    model.downloadRemoteFile(
                        reference: "computer://\(preview.id)",
                        label: preview.name
                    )
                } label: {
                    Image(systemName: "arrow.down.circle")
                        .font(.system(size: 18, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 36, height: 36)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localizedFormat("下载 %@", preview.name)))
                Button {
                    model.dismissFilePreview()
                    if !embedded { dismiss() }
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 36, height: 36)
                        .background(BitFunTheme.soft)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("关闭文件预览")))
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)

            Rectangle().fill(BitFunTheme.line).frame(height: 1)

            Group {
                if model.filePreviewLoading {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text(model.localized("正在加载文件"))
                            .font(MobileDesignTypography.bodySmall.font)
                            .foregroundStyle(BitFunTheme.muted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let failure = preview.failure {
                    VStack(spacing: 10) {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: 28, weight: .medium))
                        Text(model.localized("无法预览"))
                            .font(MobileDesignTypography.titleSmall.font)
                        Text(failure)
                            .font(MobileDesignTypography.bodySmall.font)
                            .multilineTextAlignment(.center)
                    }
                    .foregroundStyle(BitFunTheme.muted)
                    .padding(24)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let data = preview.imageData, let image = UIImage(data: data) {
                    ScrollView([.horizontal, .vertical], showsIndicators: false) {
                        Image(uiImage: image)
                            .resizable()
                            .scaledToFit()
                            .padding(18)
                    }
                } else {
                    ScrollView(showsIndicators: false) {
                        if preview.mimeType.contains("markdown") || preview.name.lowercased().hasSuffix(".md") {
                            MarkdownMessageView(text: preview.content, model: model)
                                .padding(18)
                        } else {
                            Text(preview.content)
                                .font(.system(size: 13, design: .monospaced))
                                .foregroundStyle(BitFunTheme.ink)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(18)
                                .textSelection(.enabled)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            if preview.truncated {
                Text(model.localized("文件较大，当前仅显示部分内容"))
                    .font(MobileDesignTypography.labelSmall.font)
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(BitFunTheme.soft)
            }
        }
        .background(BitFunTheme.page)
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
    }
}

private struct PairingSheet: View {
    private enum Step { case intro, scan }

    @ObservedObject var model: MobileAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var step: Step = .intro
    @State private var pairingURL = ProcessInfo.processInfo.arguments.contains("--pairing-account")
        ? "https://relay.example.com/#/pair?room=preview-room&pk=preview-key&auth=account&user=preview"
        : ""
    @State private var pairingUserID = ""
    // Intentionally transient: pairing passwords must never enter saved scene state.
    @State private var pairingPassword = ""
    @State private var scannerOpen = false
    @State private var manualOpen = false
    @FocusState private var focused: Bool

    var body: some View {
        return ZStack {
            if step == .intro { introPage } else { scanPage }
            if manualOpen { manualPairingOverlay }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.card)
        .onAppear {
            if model.pairingScanRequested {
                step = .scan
                scannerOpen = true
                model.consumePairingScanRequest()
            } else if ProcessInfo.processInfo.arguments.contains("--pairing-manual") ||
                ProcessInfo.processInfo.arguments.contains("--pairing-account") {
                step = .scan
                manualOpen = true
                focused = !ProcessInfo.processInfo.arguments.contains("--pairing-account")
            }
        }
        .fullScreenCover(isPresented: $scannerOpen) {
            QRCodeScannerView { code in
                pairingURL = code
                scannerOpen = false
                if PairingLinkHintsKt.inspectPairingLink(url: code).requiresAccount {
                    manualOpen = true
                    focused = true
                } else {
                    model.submitPairing(url: code)
                }
            }
            .ignoresSafeArea()
        }
    }

    private var introPage: some View {
        VStack(spacing: 0) {
            hero(height: 250)
            VStack(spacing: 15) {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 54, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 88, height: 88)
                    .background(BitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 28))
                    .shadow(color: BitFunTheme.line, radius: 18, y: 7)
                Text(model.localized("选择连接方式"))
                    .font(.system(size: 24, weight: .bold))
                    .foregroundStyle(BitFunTheme.ink)
            }
            .padding(.horizontal, 28)
            .offset(y: -10)
            Spacer(minLength: 12)
            SignedOutConnectionActions(
                scanTitle: model.localized("扫码连接"),
                accountTitle: model.localized("登录 BitFun 账号"),
                onScan: {
                    step = .scan
                    scannerOpen = true
                },
                onOpenAccount: model.openAccountFromPairing,
                enabled: !model.pairingBusy,
                buttonHeight: 58,
                spacing: 12,
                fontSize: 20
            )
            .padding(.horizontal, 44)
            .padding(.bottom, 34)
        }
    }

    private var scanPage: some View {
        VStack(spacing: 0) {
            hero(height: 252)
            VStack(spacing: 22) {
                Button { scannerOpen = true } label: {
                    Image(systemName: "qrcode.viewfinder")
                        .font(.system(size: 72, weight: .regular))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 176, height: 176)
                        .background(MobileDesignColors.connectHeroSurface)
                        .overlay(RoundedRectangle(cornerRadius: 34).stroke(BitFunTheme.line, lineWidth: 1.5))
                        .clipShape(RoundedRectangle(cornerRadius: 34))
                }
                .buttonStyle(.plain)
                Text(model.localized("扫描二维码"))
                    .font(.system(size: 24, weight: .bold)).foregroundStyle(BitFunTheme.ink)
                if let error = model.pairingError {
                    Text(error).font(.system(size: 13)).foregroundStyle(BitFunTheme.red)
                        .multilineTextAlignment(.center)
                }
            }
            .offset(y: -50)
            Spacer(minLength: 12)
            Button { manualOpen = true; focused = true } label: {
                Text(model.localized("手动输入配对码"))
                    .font(.system(size: 20, weight: .bold))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(maxWidth: .infinity, minHeight: 58)
                    .background(BitFunTheme.card)
                    .overlay(Capsule().stroke(BitFunTheme.line, lineWidth: 1.5))
                    .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 44)
            .padding(.bottom, 34)
        }
    }

    private func hero(height: CGFloat) -> some View {
        ZStack(alignment: .topLeading) {
            LinearGradient(
                colors: [MobileDesignColors.connectHeroBg, MobileDesignColors.connectHeroSurface],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Button {
                if step == .scan { step = .intro } else { dismiss() }
            } label: {
                Image(systemName: "chevron.left")
                    .font(.system(size: 20, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 44, height: 44)
                    .background(BitFunTheme.card)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .padding(.top, 18).padding(.leading, 18)
        }
        .frame(height: height)
    }

    private var manualPairingOverlay: some View {
        let hints = PairingLinkHintsKt.inspectPairingLink(url: pairingURL)
        let effectiveUserID = pairingUserID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? hints.suggestedUserId
            : pairingUserID.trimmingCharacters(in: .whitespacesAndNewlines)
        let canSubmit = !model.pairingBusy &&
            !pairingURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            (!hints.requiresAccount || (!effectiveUserID.isEmpty && !pairingPassword.isEmpty))

        return ZStack {
            MobileDesignColors.modalScrim
                .ignoresSafeArea()
                .onTapGesture {
                    if !model.pairingBusy {
                        pairingPassword = ""
                        manualOpen = false
                    }
                }
            VStack(alignment: .leading, spacing: 20) {
                Text(model.localized(hints.requiresAccount ? "账号认证配对" : "手动输入配对码"))
                    .font(.system(size: 24, weight: .bold)).foregroundStyle(BitFunTheme.ink)
                Text(model.localized(
                    hints.requiresAccount
                        ? "此桌面要求使用 BitFun 账号验证身份。"
                        : "输入桌面端显示的配对链接或代码。"
                ))
                    .font(.system(size: 17)).foregroundStyle(BitFunTheme.muted).lineSpacing(5)
                TextField(model.localized("配对码或连接链接"), text: $pairingURL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .lineLimit(1)
                    .font(.system(size: 20)).foregroundStyle(BitFunTheme.ink)
                    .padding(.horizontal, 20).frame(minHeight: 62)
                    .background(BitFunTheme.soft).clipShape(Capsule())
                    .focused($focused)
                if hints.requiresAccount {
                    TextField(
                        hints.suggestedUserId.isEmpty
                            ? model.localized("BitFun 用户名")
                            : hints.suggestedUserId,
                        text: $pairingUserID
                    )
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textContentType(.username)
                    .font(.system(size: 18)).foregroundStyle(BitFunTheme.ink)
                    .padding(.horizontal, 20).frame(minHeight: 56)
                    .background(BitFunTheme.soft).clipShape(Capsule())

                    SecureField(model.localized("BitFun 密码"), text: $pairingPassword)
                        .textContentType(.password)
                        .font(.system(size: 18)).foregroundStyle(BitFunTheme.ink)
                        .padding(.horizontal, 20).frame(minHeight: 56)
                        .background(BitFunTheme.soft).clipShape(Capsule())

                    Text(model.localized("账号凭据只用于本次加密配对，不会保存。"))
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.muted)
                        .lineSpacing(3)
                }
                if let error = model.pairingError {
                    Text(error).font(.system(size: 13)).foregroundStyle(BitFunTheme.red)
                }
                HStack(spacing: 12) {
                    pairingButton("取消", primary: false) {
                        pairingPassword = ""
                        manualOpen = false
                        focused = false
                    }
                    pairingButton(model.pairingBusy ? "正在连接" : "配对", primary: true) {
                        if hints.requiresAccount {
                            model.submitPairing(
                                url: pairingURL,
                                userID: effectiveUserID,
                                password: pairingPassword
                            )
                            pairingPassword = ""
                        } else {
                            model.submitPairing(url: pairingURL)
                        }
                        focused = false
                    }
                    .disabled(!canSubmit)
                }
            }
            .padding(.horizontal, 28).padding(.top, 30).padding(.bottom, 28)
            .frame(maxWidth: 520)
            .background(BitFunTheme.card)
            .clipShape(RoundedRectangle(cornerRadius: 34))
            .overlay(RoundedRectangle(cornerRadius: 34).stroke(BitFunTheme.line, lineWidth: 1))
            .padding(.horizontal, 34)
        }
    }

    private func pairingButton(_ title: String, primary: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(model.localized(title))
                .font(.system(size: 19, weight: .bold))
                .foregroundStyle(primary ? Color.white : BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 58)
                .background(primary ? BitFunTheme.accent : BitFunTheme.soft)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct QRCodeScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> QRScannerController {
        let controller = QRScannerController()
        controller.onCode = onCode
        return controller
    }

    func updateUIViewController(_ uiViewController: QRScannerController, context: Context) {}
}

private final class QRScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let session = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?
    var onCode: ((String) -> Void)?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.tintColor = .white
        close.backgroundColor = UIColor.black.withAlphaComponent(0.55)
        close.layer.cornerRadius = 22
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        close.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(close)
        NSLayoutConstraint.activate([
            close.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            close.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -20),
            close.widthAnchor.constraint(equalToConstant: 44),
            close.heightAnchor.constraint(equalToConstant: 44),
        ])

        guard AVCaptureDevice.authorizationStatus(for: .video) != .denied else { return }
        AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
            guard granted else { return }
            DispatchQueue.main.async { self?.configureCapture() }
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    private func configureCapture() {
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else { return }
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addInput(input)
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        view.layer.insertSublayer(layer, at: 0)
        previewLayer = layer
        session.startRunning()
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection,
    ) {
        guard let value = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue,
              !value.isEmpty else { return }
        session.stopRunning()
        onCode?(value)
        dismiss(animated: true)
    }
}

private struct LocalHomeView: View {
    @ObservedObject var model: MobileAppModel

    private let prompts: [(String, String)] = [
        ("Aa", "帮我写点内容"),
        ("≡", "梳理一个问题"),
        ("✓", "制定行动计划")
    ]

    var body: some View {
        VStack(spacing: 0) {
            Spacer(minLength: 0)
            VStack(spacing: 12) {
                ForEach(prompts, id: \.1) { icon, title in
                    let promptText = model.localized(title)
                    Button {
                        model.draft = promptText
                        model.send()
                    } label: {
                        HStack(spacing: 20) {
                            Text(icon)
                                .font(.system(size: 29, weight: .regular))
                                .foregroundStyle(BitFunTheme.muted)
                                .frame(width: 32)
                                .fixedSize()
                            Text(promptText)
                                .font(.system(size: 20, weight: .medium))
                                .foregroundStyle(BitFunTheme.muted)
                            Spacer(minLength: 0)
                        }
                        .frame(height: 48)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 12)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.page)
    }
}

private struct RemoteHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ZStack(alignment: .topTrailing) {
            VStack(spacing: 12) {
            Spacer()
            ZStack {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 42, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
            }
            .frame(width: 74, height: 74)
            .background(BitFunTheme.card)
            .overlay(RoundedRectangle(cornerRadius: 24).stroke(BitFunTheme.line, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 24))
            Text(model.localized("连接桌面端"))
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(BitFunTheme.ink)
            Text(model.localized("扫描桌面端显示的二维码，开始远程处理任务。"))
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .multilineTextAlignment(.center)
                .lineSpacing(7)
                .padding(.horizontal, 20)
            Button(model.localized("连接")) { model.connectRemote() }
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(.white)
                .frame(width: 136, height: 44)
                .background(BitFunTheme.accent)
                .clipShape(Capsule())
            Spacer()
            }
            remoteSettingsButton
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 20)
        .padding(.bottom, 48)
        .background(BitFunTheme.page)
    }

    private var remoteSettingsButton: some View {
        Button { model.remoteControlSettingsOpen = true } label: {
            Image(systemName: "gearshape")
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(BitFunTheme.ink)
                .frame(width: 44, height: 44)
                .background(BitFunTheme.card)
                .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                .clipShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(model.localized("远程控制设置"))
        .padding(.top, 16).padding(.trailing, 16)
    }
}

private struct RemoteConnectedHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ZStack(alignment: .topTrailing) {
            VStack(spacing: 14) {
            Spacer()
            Image(systemName: "desktopcomputer.and.macbook")
                .font(.system(size: 34, weight: .medium)).foregroundStyle(BitFunTheme.muted)
            Text(model.localized("桌面端已连接"))
                .font(MobileDesignTypography.titleMedium.font).foregroundStyle(BitFunTheme.ink)
            Text(model.localized("选择已有会话，或在当前工作区创建一个新会话。"))
                .font(MobileDesignTypography.bodySmall.font).foregroundStyle(BitFunTheme.muted)
                .multilineTextAlignment(.center)
            Button { model.remoteCreateOpen = true } label: {
                Label(model.localized("新建远程会话"), systemImage: "plus")
                    .font(MobileDesignTypography.labelMedium.font).foregroundStyle(.white)
                    .frame(minWidth: 176, minHeight: 44).background(BitFunTheme.accent).clipShape(Capsule())
            }
            .buttonStyle(.plain)
            Spacer()
            }
            Button { model.remoteControlSettingsOpen = true } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 44, height: 44)
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("远程控制设置"))
            .padding(.top, 16).padding(.trailing, 16)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.page)
    }
}

private struct ConnectionStatusBar: View {
    let phase: ConnectionPhase
    var detail: String?
    let onRetry: () -> Void
    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(phase == .reconnecting ? BitFunTheme.muted : BitFunTheme.red).frame(width: 8, height: 8)
            Text(MobileLocalization.text(phase == .reconnecting ? "正在恢复连接" : "连接不可用"))
                .font(.system(size: 13, weight: .medium))
            Text(
                detail ?? MobileLocalization.text(
                    phase == .reconnecting ? "正在重新连接桌面端" : "请重新连接"
                )
            )
                .font(.system(size: 12))
                .foregroundStyle(BitFunTheme.muted)
            Spacer()
            if phase == .disconnected {
                Button(MobileLocalization.text("重试"), action: onRetry)
                    .font(.system(size: 13, weight: .semibold))
                    .buttonStyle(.plain)
                    .foregroundStyle(BitFunTheme.accent)
            }
        }
        .foregroundStyle(BitFunTheme.ink)
        .padding(.horizontal, 18)
        .frame(height: 48)
        .background(BitFunTheme.soft)
    }
}

private struct SettingsView: View {
    @ObservedObject var model: MobileAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var accountOpen = false

    private var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0.0"
    }

    private var selectedModelName: String {
        model.modelOptions.first(where: \.selected)?.primaryLabel
            ?? model.modelOptions.first?.primaryLabel
            ?? model.localized("未配置")
    }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(model.localized("设置"))
                        .font(.system(size: 28, weight: .bold))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.bottom, 30)

                    Button { accountOpen = true } label: {
                        SettingsCard {
                            SettingsProfileRow(
                                subtitle: model.accountUser ?? model.localized("未登录")
                            )
                        }
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 24)

                    SettingsGroup(title: "通用") {
                        VStack(spacing: 0) {
                            Button { model.languagePickerOpen = true } label: {
                                SettingsValueRow(
                                    icon: "textformat",
                                    title: "语言",
                                    value: model.appLanguage.nativeName,
                                    showsChevron: true
                                )
                            }
                            .buttonStyle(.plain)
                            Divider().overlay(BitFunTheme.line).padding(.horizontal, 26)
                            Button { model.generalConfigOpen = true } label: {
                                SettingsValueRow(
                                    icon: "square.grid.2x2",
                                    title: "模型",
                                    value: selectedModelName,
                                    showsChevron: true
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    SettingsGroup(title: "关于") {
                        VStack(spacing: 0) {
                            SettingsValueRow(
                                icon: nil,
                                title: "产品",
                                value: "BitFun iOS版"
                            )
                            Divider().overlay(BitFunTheme.line).padding(.horizontal, 26)
                            SettingsValueRow(icon: nil, title: "版本", value: appVersion)
                        }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.top, 64)
                .padding(.bottom, 34)
            }

            Button { dismiss() } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 18, weight: .regular))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 40, height: 40)
                    .background(BitFunTheme.card)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("关闭"))
            .padding(.top, 22)
            .padding(.trailing, 18)

            if model.languagePickerOpen {
                LanguagePickerSheet(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            } else if model.generalConfigOpen {
                GeneralChatConfigSheet(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            } else if accountOpen {
                AccountSettingsView(model: model, onClose: { accountOpen = false })
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .background(BitFunTheme.page)
        .animation(.easeInOut(duration: 0.2), value: model.languagePickerOpen)
        .animation(.easeInOut(duration: 0.2), value: model.generalConfigOpen)
        .animation(.easeInOut(duration: 0.2), value: accountOpen)
    }
}

private struct LanguagePickerSheet: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            BitFunSelectionHeader(title: "选择语言", onClose: { model.languagePickerOpen = false })
            Divider().overlay(BitFunTheme.line)

            VStack(spacing: 0) {
                ForEach(MobileLanguage.allCases) { language in
                    Button {
                        model.setLanguage(language)
                        model.languagePickerOpen = false
                    } label: {
                        HStack {
                            Text(language.nativeName)
                                .font(.system(size: 16, weight: .medium))
                                .foregroundStyle(BitFunTheme.ink)
                            Spacer()
                            if model.appLanguage == language {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 18, weight: .medium))
                                    .foregroundStyle(BitFunTheme.ink)
                            }
                        }
                        .padding(.horizontal, 16)
                        .frame(height: MobileDesignGeometry.selectionRowHeight)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.top, 8)
            .padding(.bottom, 28)

            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.selectionTopRadius))
    }
}

private struct RemoteViewSettingsView: View {
    @ObservedObject var model: MobileAppModel

    private var statuses: [String] {
        model.sessionListStatusOptions
    }

    private var workspaces: [MobileSessionWorkspaceOption] {
        model.sessionListWorkspaceOptions
    }

    private var agentGroups: [String] {
        model.sessionListAgentGroups
    }

    var body: some View {
        VStack(spacing: 0) {
            BitFunModalHeader(
                title: "视图设置",
                subtitle: "调整会话列表的分组和信息密度",
                onClose: { model.remoteViewSettingsOpen = false }
            )
            .padding(.horizontal, 20)
            Divider().overlay(BitFunTheme.line)

            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 8) {
                    sectionTitle("分组方式")
                    SettingsCard {
                        choiceRow("按项目", value: "PROJECT", selected: model.remoteGroupMode)
                        settingsDivider
                        choiceRow("按时间倒序排列", value: "TIME", selected: model.remoteGroupMode)
                        settingsDivider
                        choiceRow("聊天优先", value: "CHAT", selected: model.remoteGroupMode)
                    }

                    sectionTitle("筛选")
                    filterLabel("工作区")
                    SettingsCard {
                        filterRow(
                            "所有工作区",
                            selected: model.remoteWorkspaceFilter.isEmpty,
                            action: { model.remoteWorkspaceFilter = "" }
                        )
                        ForEach(workspaces) { workspace in
                            settingsDivider
                            filterRow(
                                workspace.name,
                                selected: normalizedPath(model.remoteWorkspaceFilter) == normalizedPath(workspace.path),
                                action: { model.remoteWorkspaceFilter = workspace.path }
                            )
                        }
                    }

                    filterLabel("Agent 类型")
                    SettingsCard {
                        filterRow(
                            "所有 Agent 类型",
                            selected: model.remoteViewAgentFilter.isEmpty,
                            action: { model.remoteViewAgentFilter = "" }
                        )
                        ForEach(agentGroups, id: \.self) { group in
                            settingsDivider
                            filterRow(
                                agentLabel(group),
                                selected: model.remoteViewAgentFilter == group,
                                action: { model.remoteViewAgentFilter = group }
                            )
                        }
                    }

                    filterLabel("状态")
                    SettingsCard {
                        filterRow(
                            "所有状态",
                            selected: model.remoteStatusFilter.isEmpty,
                            action: { model.remoteStatusFilter = "" }
                        )
                        ForEach(statuses, id: \.self) { status in
                            settingsDivider
                            filterRow(
                                statusLabel(status),
                                selected: model.remoteStatusFilter == status,
                                action: { model.remoteStatusFilter = status }
                            )
                        }
                    }

                    sectionTitle("显示信息")
                    SettingsCard {
                        metadataToggle("工作区", isOn: $model.remoteShowWorkspaceMetadata)
                        settingsDivider
                        metadataToggle("更新时间", isOn: $model.remoteShowUpdatedMetadata)
                        settingsDivider
                        metadataToggle("状态", isOn: $model.remoteShowStatusMetadata)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 8)
                .padding(.bottom, 34)
            }
        }
        .background(BitFunTheme.page)
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(model.localized(title))
            .font(MobileDesignTypography.labelLarge.font)
            .foregroundStyle(BitFunTheme.muted)
            .padding(.top, 8)
            .padding(.leading, 4)
    }

    private func filterLabel(_ title: String) -> some View {
        Text(model.localized(title))
            .font(MobileDesignTypography.labelSmall.font)
            .foregroundStyle(BitFunTheme.muted)
            .padding(.top, 2)
            .padding(.leading, 10)
    }

    private var settingsDivider: some View {
        Divider().overlay(BitFunTheme.line).padding(.horizontal, 20)
    }

    private func choiceRow(_ title: String, value: String, selected: String) -> some View {
        filterRow(title, selected: value == selected) { model.remoteGroupMode = value }
    }

    private func filterRow(_ title: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Text(model.localized(title))
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if selected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(BitFunTheme.accent)
                }
            }
            .padding(.horizontal, 20)
            .frame(minHeight: 52)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func metadataToggle(_ title: String, isOn: Binding<Bool>) -> some View {
        Toggle(isOn: isOn) {
            Text(model.localized(title))
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(BitFunTheme.ink)
        }
        .tint(BitFunTheme.accent)
        .padding(.horizontal, 20)
        .frame(minHeight: 56)
    }

    private func agentLabel(_ group: String) -> String {
        switch group {
        case "CHAT": return "聊天"
        case "COWORK": return "Cowork"
        default: return "Code"
        }
    }

    private func statusLabel(_ status: String) -> String {
        switch status {
        case "active", "running": return "运行中"
        case "ready", "idle": return "就绪"
        case "archived": return "已归档"
        default: return status
        }
    }

    private func normalizedPath(_ path: String) -> String {
        var result = path.trimmingCharacters(in: .whitespacesAndNewlines)
        while result.count > 1 && (result.hasSuffix("/") || result.hasSuffix("\\")) {
            result.removeLast()
        }
        return result
    }
}

/// The desktop-wide control page mirrors HarmonyOS' RemoteControlSettingsSheet.
/// Account navigation and full-access confirmation stay inside this adaptive
/// modal so a settings action never creates a second sheet or scrim.
private struct RemoteControlSettingsView: View {
    private enum Page { case control, account }

    @ObservedObject var model: MobileAppModel
    @State private var page: Page = .control
    @State private var confirmingFullAccess = false

    var body: some View {
        Group {
            if page == .account {
                AccountSettingsView(model: model, onClose: { page = .control })
            } else {
                controlPage
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.page)
        .animation(.easeInOut(duration: 0.2), value: page)
        .onAppear {
            if model.remoteConnected { model.refreshRemotePermissionMode() }
        }
    }

    private var controlPage: some View {
        ZStack(alignment: .topTrailing) {
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(model.localized("远程控制"))
                        .font(.system(size: 20, weight: .bold))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(maxWidth: .infinity, minHeight: 56, alignment: .center)
                        .padding(.bottom, 30)

                    Button { page = .account } label: {
                        SettingsCard {
                            HStack(spacing: 12) {
                                Image(systemName: "person.crop.circle")
                                    .font(.system(size: 28, weight: .regular))
                                    .foregroundStyle(BitFunTheme.muted)
                                    .frame(width: 34, height: 34)
                                Text(model.localized(model.accountUser == nil ? "登录 BitFun 账号" : "个人资料"))
                                    .font(.system(size: 18, weight: .medium))
                                    .foregroundStyle(BitFunTheme.ink)
                                Spacer()
                                Image(systemName: "chevron.right")
                                    .font(.system(size: 14, weight: .medium))
                                    .foregroundStyle(BitFunTheme.muted.opacity(0.72))
                            }
                            .padding(.horizontal, 18)
                            .frame(height: 64)
                        }
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 28)

                    remoteSectionTitle("当前远程控制")
                    currentControlCard

                    remoteSectionTitle("其他连接方式")
                        .padding(.top, 16)
                    Button {
                        model.remoteControlSettingsOpen = false
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.22) {
                            model.connectRemote()
                        }
                    } label: {
                        SettingsCard {
                            HStack(spacing: 12) {
                                Image(systemName: "link")
                                    .font(.system(size: 20, weight: .regular))
                                    .foregroundStyle(BitFunTheme.muted)
                                    .frame(width: 24, height: 24)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(model.localized("扫描二维码连接"))
                                        .font(.system(size: 16, weight: .medium))
                                        .foregroundStyle(BitFunTheme.ink)
                                    Text(model.localized("适用于临时配对或未登录账号的桌面端。"))
                                        .font(.system(size: 13))
                                        .foregroundStyle(BitFunTheme.muted)
                                        .lineLimit(2)
                                }
                                Spacer(minLength: 8)
                                Image(systemName: "chevron.right")
                                    .font(.system(size: 14, weight: .medium))
                                    .foregroundStyle(BitFunTheme.muted.opacity(0.72))
                            }
                            .padding(.horizontal, 18)
                            .frame(minHeight: 78)
                        }
                    }
                    .buttonStyle(.plain)

                    permissionSection
                        .padding(.top, 20)
                }
                .padding(.horizontal, 18)
                .padding(.top, 20)
                .padding(.bottom, 42)
            }

            Button { model.remoteControlSettingsOpen = false } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 17, weight: .regular))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 40, height: 40)
                    .background(BitFunTheme.card)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("关闭"))
            .padding(.top, 16).padding(.trailing, 16)
        }
    }

    private var currentControlCard: some View {
        SettingsCard {
            HStack(spacing: 14) {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 23, weight: .regular))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(width: 40, height: 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.localized("BitFun 桌面版"))
                        .font(.system(size: 14)).foregroundStyle(BitFunTheme.muted)
                    Text(model.accountDeviceName ?? model.localized("尚未连接桌面端"))
                        .font(.system(size: 18, weight: .medium)).foregroundStyle(BitFunTheme.ink)
                        .lineLimit(1)
                    Text(connectionStatus)
                        .font(.system(size: 14)).foregroundStyle(BitFunTheme.muted)
                }
                Spacer(minLength: 6)
                if model.remoteConnected {
                    remoteChip("断开", action: model.disconnectRemote)
                } else if model.connectionPhase == .disconnected {
                    remoteChip("重新连接", action: model.verifyRemoteConnection)
                }
            }
            .padding(.horizontal, 18)
            .frame(minHeight: 92)

            Divider().overlay(BitFunTheme.line).padding(.horizontal, 18)

            HStack(spacing: 10) {
                Image(systemName: "link")
                    .font(.system(size: 18)).foregroundStyle(BitFunTheme.muted)
                    .frame(width: 20, height: 20)
                Text(model.localized("连接来源"))
                    .font(.system(size: 14)).foregroundStyle(BitFunTheme.muted)
                Spacer()
                Text(connectionSource)
                    .font(.system(size: 13)).foregroundStyle(BitFunTheme.ink)
                    .padding(.horizontal, 10).padding(.vertical, 5)
                    .background(BitFunTheme.soft).clipShape(Capsule())
            }
            .padding(.horizontal, 18)
            .frame(height: 52)
        }
    }

    private var permissionSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                remoteSectionTitle("远程权限")
                Spacer()
                if model.remoteConnected {
                    Button(model.localized("刷新")) { model.refreshRemotePermissionMode() }
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .buttonStyle(.plain)
                        .disabled(model.busy)
                }
            }
            SettingsCard {
                Text(model.localized("控制桌面端执行工具时采用的确认方式。"))
                    .font(.system(size: 13)).foregroundStyle(BitFunTheme.muted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 18).padding(.top, 16).padding(.bottom, 4)
                permissionRow("ASK", title: "每次询问", detail: "执行需要授权的操作前先询问。")
                Divider().overlay(BitFunTheme.line).padding(.horizontal, 18)
                permissionRow("AUTO", title: "自动允许", detail: "自动允许常规操作，高风险操作仍会询问。")
                Divider().overlay(BitFunTheme.line).padding(.horizontal, 18)
                permissionRow("FULL_ACCESS", title: "完全访问", detail: "不再询问，允许桌面端执行所有操作。")

                if let failure = model.remotePermissionFailure, !failure.isEmpty {
                    Text(failure)
                        .font(.system(size: 12)).foregroundStyle(BitFunTheme.red)
                        .padding(.horizontal, 18).padding(.bottom, 10)
                }

                if confirmingFullAccess {
                    fullAccessConfirmation
                }
            }
        }
    }

    private var fullAccessConfirmation: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(model.localized("确认完全访问"))
                .font(.system(size: 15, weight: .bold)).foregroundStyle(BitFunTheme.red)
            Text(model.localized("完全访问会取消所有操作确认。仅在你信任当前桌面端时启用。"))
                .font(.system(size: 13)).foregroundStyle(BitFunTheme.ink).lineSpacing(4)
            HStack(spacing: 10) {
                confirmationButton("取消", destructive: false) { confirmingFullAccess = false }
                confirmationButton("启用完全访问", destructive: true) {
                    model.setRemotePermissionMode("FULL_ACCESS")
                    confirmingFullAccess = false
                }
            }
        }
        .padding(16)
        .overlay(RoundedRectangle(cornerRadius: 18).stroke(BitFunTheme.red, lineWidth: 1))
        .padding(.horizontal, 12).padding(.bottom, 14)
    }

    private func permissionRow(_ mode: String, title: String, detail: String) -> some View {
        Button {
            if mode == "FULL_ACCESS" { confirmingFullAccess = true }
            else {
                confirmingFullAccess = false
                model.setRemotePermissionMode(mode)
            }
        } label: {
            HStack(spacing: 12) {
                ZStack {
                    if model.remotePermissionMode == mode {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 20)).foregroundStyle(BitFunTheme.ink)
                    }
                }
                .frame(width: 22, height: 24)
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.localized(title))
                        .font(.system(size: 16, weight: .medium)).foregroundStyle(BitFunTheme.ink)
                    Text(model.localized(detail))
                        .font(.system(size: 12)).foregroundStyle(BitFunTheme.muted)
                        .lineLimit(2)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 18)
            .frame(minHeight: 72)
            .contentShape(Rectangle())
            .opacity(model.remoteConnected && !model.busy ? 1 : 0.54)
        }
        .buttonStyle(.plain)
        .disabled(!model.remoteConnected || model.busy)
    }

    private func remoteSectionTitle(_ title: String) -> some View {
        Text(model.localized(title))
            .font(.system(size: 18, weight: .bold))
            .foregroundStyle(BitFunTheme.muted)
            .frame(maxWidth: .infinity, minHeight: 42, alignment: .leading)
            .padding(.horizontal, 18)
    }

    private func remoteChip(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(model.localized(title))
                .font(.system(size: 14)).foregroundStyle(BitFunTheme.ink)
                .padding(.horizontal, 10).padding(.vertical, 7)
                .background(BitFunTheme.soft).clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    private func confirmationButton(
        _ title: String,
        destructive: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(model.localized(title))
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(destructive ? Color.white : BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 42)
                .background(destructive ? BitFunTheme.red : BitFunTheme.soft)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    private var connectionStatus: String {
        switch model.connectionPhase {
        case .connected: model.localized(model.remoteConnected ? "已连接" : "未连接")
        case .reconnecting: model.localized("正在重新连接")
        case .disconnected: model.localized("连接已断开")
        }
    }

    private var connectionSource: String {
        if model.accountSelectedDeviceID != nil { return model.localized("账号设备") }
        if model.remoteConnected { return model.localized("扫码配对") }
        return model.localized("未连接")
    }
}

private struct AccountSettingsView: View {
    @ObservedObject var model: MobileAppModel
    var onClose: (() -> Void)? = nil
    @State private var relayURL = AccountDefaults.shared.CLOUD_RELAY_URL
    @State private var username = ""
    @State private var password = ""

    var body: some View {
        Group {
            if model.accountUser == nil {
                loginPage
            } else {
                profilePage
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.page)
    }

    private var loginPage: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                Button { close() } label: {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 19, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 44, height: 44)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.localized("返回"))

                Text(model.localized("登录 BitFun 账号"))
                    .font(.system(size: 32, weight: .bold))
                    .foregroundStyle(BitFunTheme.ink)
                Text(model.localized("登录后可查看并连接账号下的桌面设备。"))
                    .font(.system(size: 15))
                    .foregroundStyle(BitFunTheme.muted)
                    .lineSpacing(4)
                    .padding(.top, 12)
                    .padding(.bottom, 42)

                accountField(model.localized("用户名"), text: $username, secure: false, height: 58)
                accountField(model.localized("密码"), text: $password, secure: true, height: 58)
                    .padding(.top, 14)

                Text(model.localized("登录服务器"))
                    .font(.system(size: 13))
                    .foregroundStyle(BitFunTheme.muted)
                    .padding(.leading, 4)
                    .padding(.top, 26)
                    .padding(.bottom, 8)
                accountField(model.localized("Relay 地址"), text: $relayURL, secure: false, height: 52)

                if let error = model.coreErrorMessage, !error.isEmpty {
                    Text(error)
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.red)
                        .padding(.top, 12)
                }

                Button {
                    model.loginAccount(relayURL: relayURL, username: username, password: password)
                    password = ""
                } label: {
                    HStack(spacing: 8) {
                        if model.accountBusy { ProgressView().tint(.white) }
                        Text(model.localized(model.accountBusy ? "正在登录" : "登录"))
                    }
                    .font(.system(size: 17, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity, minHeight: 56)
                    .background(canLogin ? BitFunTheme.accent : BitFunTheme.muted.opacity(0.35))
                    .clipShape(RoundedRectangle(cornerRadius: 18))
                }
                .buttonStyle(.plain)
                .disabled(!canLogin)
                .padding(.top, model.coreErrorMessage == nil ? 30 : 22)
            }
            .padding(.horizontal, 28)
            .padding(.top, 22)
            .padding(.bottom, 44)
        }
    }

    private var profilePage: some View {
        VStack(alignment: .leading, spacing: 0) {
            BitFunModalHeader(title: "个人资料", onClose: close)
                .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                .padding(.top, 8)
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(spacing: 10) {
                        ZStack {
                            Circle().fill(BitFunTheme.soft)
                            Image(systemName: "person.fill")
                                .font(.system(size: 34, weight: .medium))
                                .foregroundStyle(BitFunTheme.ink)
                        }
                        .frame(width: 70, height: 70)
                        Text(model.accountUser ?? "")
                            .font(.system(size: 22, weight: .bold))
                            .foregroundStyle(BitFunTheme.ink)
                            .lineLimit(1)
                        Text(profileIdentifier)
                            .font(.system(size: 14))
                            .foregroundStyle(BitFunTheme.muted)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                    .background(BitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 28))
                        .padding(.bottom, 24)

                    VStack(alignment: .leading, spacing: 10) {
                        HStack {
                            Text(model.localized("BitFun 账号"))
                                .font(.system(size: 17, weight: .bold))
                                .foregroundStyle(BitFunTheme.ink)
                            Spacer()
                            Text(model.localized("已登录"))
                                .font(.system(size: 14))
                                .foregroundStyle(BitFunTheme.green)
                        }
                        Text(model.localizedFormat("当前以 %@ 登录。", model.accountUser ?? ""))
                            .font(.system(size: 14))
                            .foregroundStyle(BitFunTheme.muted)
                            .lineSpacing(3)
                    }
                    .padding(.horizontal, 18)
                    .padding(.vertical, 16)
                    .background(BitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 24))
                    .padding(.bottom, 24)

                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(model.localized("设备管理"))
                                .font(.system(size: 17, weight: .bold))
                                .foregroundStyle(BitFunTheme.ink)
                            Spacer()
                            Button { model.refreshRemoteDevices() } label: {
                                Text(model.localized(model.accountRefreshing ? "正在刷新" : "刷新"))
                                    .font(.system(size: 13))
                                    .foregroundStyle(model.accountRefreshing ? BitFunTheme.muted : BitFunTheme.ink)
                            }
                            .buttonStyle(.plain)
                            .disabled(model.accountRefreshing)
                        }
                        VStack(spacing: 0) {
                            ForEach(Array(model.accountDevices.enumerated()), id: \.offset) { index, device in
                                Button { model.selectRemoteDevice(device) } label: {
                                    SettingsDeviceRow(device: device)
                                }
                                .buttonStyle(.plain)
                                .disabled(!device.online && !device.selected)
                                if index < model.accountDevices.count - 1 {
                                    Divider().overlay(BitFunTheme.line).padding(.horizontal, 20)
                                }
                            }
                            if model.accountDevices.isEmpty {
                                Text(model.localized("暂无可连接的桌面设备"))
                                    .font(.system(size: 13))
                                    .foregroundStyle(BitFunTheme.muted)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(.vertical, 12)
                            }
                        }
                    }
                    .padding(.horizontal, 18)
                    .padding(.vertical, 16)
                    .background(BitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 24))
                    .padding(.bottom, 24)

                    Text(model.localized("个人资料详情"))
                        .font(.system(size: 18, weight: .bold))
                        .foregroundStyle(BitFunTheme.muted)
                        .padding(.leading, 18)
                        .padding(.bottom, 8)

                    VStack(spacing: 0) {
                        profileDetailRow(label: model.localized("用户 ID"), value: profileIdentifier)
                        Divider().overlay(BitFunTheme.line).padding(.horizontal, 18)
                        profileDetailRow(
                            label: model.localized("设备 ID"),
                            value: model.localDeviceID.isEmpty ? "-" : model.localDeviceID
                        )
                    }
                    .background(BitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 28))

                    Button(role: .destructive) {
                        model.logoutAccount()
                    } label: {
                        Text(model.localized("退出账号"))
                            .font(.system(size: 16, weight: .medium))
                            .foregroundStyle(BitFunTheme.red)
                            .frame(maxWidth: .infinity, minHeight: 54)
                            .background(BitFunTheme.card)
                            .clipShape(RoundedRectangle(cornerRadius: 16))
                    }
                    .buttonStyle(.plain)
                    .padding(.top, 18)
                }
                .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                .padding(.top, 20)
                .padding(.bottom, 34)
            }
        }
    }

    private var profileIdentifier: String {
        model.accountUserID?.isEmpty == false ? model.accountUserID! : (model.accountUser ?? "-")
    }

    private func profileDetailRow(label: String, value: String) -> some View {
        HStack(spacing: 12) {
            Text(label)
                .font(.system(size: 16))
                .foregroundStyle(BitFunTheme.ink)
            Spacer(minLength: 8)
            Text(value)
                .font(.system(size: 16))
                .foregroundStyle(BitFunTheme.muted)
                .lineLimit(1)
                .truncationMode(.middle)
                .multilineTextAlignment(.trailing)
        }
        .frame(minHeight: 56)
        .padding(.horizontal, 18)
    }

    private var canLogin: Bool {
        !model.accountBusy &&
            !relayURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            !username.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            !password.isEmpty
    }

    private func close() {
        if let onClose { onClose() } else { model.accountSheetOpen = false }
    }

    @ViewBuilder
    private func accountField(
        _ placeholder: String,
        text: Binding<String>,
        secure: Bool,
        height: CGFloat
    ) -> some View {
        Group {
            if secure { SecureField(placeholder, text: text) }
            else { TextField(placeholder, text: text) }
        }
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .font(.system(size: height == 58 ? 17 : 14))
        .foregroundStyle(BitFunTheme.ink)
        .padding(.horizontal, 20)
        .frame(height: height)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: height == 58 ? 18 : 16))
    }
}

private struct GeneralChatConfigSheet: View {
    private enum Page { case overview, account, local }

    @ObservedObject var model: MobileAppModel
    @State private var page: Page = .overview
    @State private var baseURL = ""
    @State private var modelName = ""
    @State private var apiKey = ""
    @State private var clearAPIKey = false

    private var selectedModel: ComposerModelOption? {
        model.modelOptions.first(where: \.selected)
    }

    private var accountModels: [ComposerModelOption] {
        model.modelOptions.filter { $0.source == "ACCOUNT" }
    }

    private var localModel: ComposerModelOption? {
        model.modelOptions.first { $0.source == "LOCAL" }
    }

    private var localComplete: Bool {
        !model.generalConfigBaseURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            !model.generalConfigModel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            model.generalConfigHasAPIKey
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            modelHeader
            Divider().overlay(BitFunTheme.line)
            switch page {
            case .overview: overview
            case .account: accountSelection
            case .local: localEditor
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(BitFunTheme.card)
        .onAppear {
            baseURL = model.generalConfigBaseURL
            modelName = model.generalConfigModel
        }
    }

    private var modelHeader: some View {
        HStack(spacing: 8) {
            if page != .overview {
                Button { page = .overview } label: {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 18, weight: .medium))
                        .frame(width: 42, height: 42)
                }
                .buttonStyle(.plain)
                .foregroundStyle(BitFunTheme.ink)
                .accessibilityLabel(model.localized("返回"))
            }
            Text(model.localized(headerTitle))
                .font(MobileDesignTypography.headlineSmall.font)
                .foregroundStyle(BitFunTheme.ink)
                .lineLimit(1)
            Spacer(minLength: 8)
            Button { model.generalConfigOpen = false } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 18, weight: .regular))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(
                        width: MobileDesignGeometry.selectionCloseSize,
                        height: MobileDesignGeometry.selectionCloseSize
                    )
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("关闭"))
        }
        .padding(.horizontal, 16)
        .frame(height: MobileDesignGeometry.sheetHeaderHeight)
    }

    private var headerTitle: String {
        switch page {
        case .overview: "普通对话模型"
        case .account: "选择账号模型"
        case .local: "本机自定义模型"
        }
    }

    private var overview: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: MobileDesignGeometry.modelSectionGap) {
                VStack(alignment: .leading, spacing: 8) {
                    sectionTitle("当前使用")
                    modelOverviewRow(
                        icon: "checkmark.circle.fill",
                        title: selectedModel?.primaryLabel ?? model.localized("未配置"),
                        subtitle: selectedModel.map { sourceLabel($0.source) } ?? "",
                        height: MobileDesignGeometry.modelCurrentRowHeight
                    )
                }
                VStack(alignment: .leading, spacing: 8) {
                    sectionTitle("模型来源")
                    VStack(spacing: 0) {
                        Button { page = .account } label: {
                            sourceRow(
                                icon: "cloud",
                                title: "云端账号模型",
                                subtitle: accountModels.isEmpty
                                    ? model.localized("暂无可用的账号模型")
                                    : model.localizedFormat("已同步 %d 个", accountModels.count),
                                chevronAction: nil
                            )
                        }
                        .buttonStyle(.plain)
                        Divider().overlay(BitFunTheme.line).padding(.leading, 56)
                        HStack(spacing: 0) {
                            Button {
                                if localComplete, let localModel { model.selectModel(localModel.id) }
                                else { page = .local }
                            } label: {
                                sourceRow(
                                    icon: "wrench.and.screwdriver",
                                    title: localComplete ? model.generalConfigModel : model.localized("未配置"),
                                    subtitle: localComplete ? model.localized("本机") : "",
                                    chevronAction: nil
                                )
                            }
                            .buttonStyle(.plain)
                            Button { page = .local } label: {
                                Image(systemName: "chevron.right")
                                    .font(.system(size: 14, weight: .medium))
                                    .foregroundStyle(BitFunTheme.muted)
                                    .frame(width: 44, height: MobileDesignGeometry.modelSourceRowHeight)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .background(BitFunTheme.soft)
                    .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCompactCardRadius))
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, MobileDesignGeometry.modelOverviewTopPadding)
            .padding(.bottom, MobileDesignGeometry.modelOverviewBottomPadding)
        }
    }

    private var accountSelection: some View {
        Group {
            if accountModels.isEmpty {
                Text(model.localized("暂无可用的账号模型"))
                    .font(MobileDesignTypography.bodyMedium.font)
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(maxWidth: .infinity, minHeight: MobileDesignGeometry.modelEmptyAccountHeight, alignment: .leading)
                    .padding(.horizontal, 16)
            } else {
                ScrollView(showsIndicators: true) {
                    LazyVStack(spacing: MobileDesignGeometry.modelAccountRowGap) {
                        ForEach(accountModels) { option in
                            Button {
                                model.selectModel(option.id)
                                page = .overview
                            } label: {
                                HStack(spacing: 10) {
                                    Image(systemName: option.selected ? "checkmark.circle" : "circle")
                                        .foregroundStyle(option.selected ? BitFunTheme.ink : Color.clear)
                                        .frame(width: 20, height: 20)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(option.primaryLabel)
                                            .font(MobileDesignTypography.titleSmall.font)
                                            .foregroundStyle(BitFunTheme.ink)
                                            .lineLimit(1)
                                        Text(model.localized("云端账号"))
                                            .font(MobileDesignTypography.labelSmall.font)
                                            .foregroundStyle(BitFunTheme.muted)
                                    }
                                    Spacer()
                                }
                                .padding(.horizontal, 10)
                                .frame(height: MobileDesignGeometry.modelAccountRowHeight)
                                .background(option.selected ? BitFunTheme.soft : Color.clear)
                                .clipShape(RoundedRectangle(cornerRadius: 9))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.top, MobileDesignGeometry.modelListTopPadding)
                    .padding(.bottom, MobileDesignGeometry.modelListBottomPadding)
                }
            }
        }
    }

    private var localEditor: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 20) {
                labeledField("API URL", placeholder: "https://api.example.com", text: $baseURL, secure: false)
                labeledField(
                    "API Key",
                    placeholder: model.generalConfigHasAPIKey ? "API Key（留空则保留）" : "请输入 API Key",
                    text: $apiKey,
                    secure: true
                )
                if model.generalConfigHasAPIKey {
                    Button {
                        clearAPIKey.toggle()
                        apiKey = ""
                    } label: {
                        Text(model.localized(clearAPIKey ? "保留已保存的 Key" : "清除已保存的 API Key"))
                            .font(MobileDesignTypography.bodySmall.font)
                            .foregroundStyle(clearAPIKey ? BitFunTheme.ink : BitFunTheme.red)
                    }
                    .buttonStyle(.plain)
                }
                labeledField("模型名称", placeholder: "例如 chat-model", text: $modelName, secure: false)
                HStack(spacing: 12) {
                    editorAction(title: model.generalConnectionTestRunning ? "测试中…" : "测试连接", primary: false) {
                        model.testGeneralConnection(
                            baseURL: baseURL, model: modelName, apiKey: apiKey, clearAPIKey: clearAPIKey
                        )
                    }
                    .disabled(model.generalConnectionTestRunning || (apiKey.isEmpty && (!model.generalConfigHasAPIKey || clearAPIKey)))
                    editorAction(title: "保存", primary: true) {
                        model.saveGeneralConfig(
                            baseURL: baseURL, model: modelName, apiKey: apiKey, clearAPIKey: clearAPIKey
                        )
                    }
                }
                if apiKey.isEmpty && (!model.generalConfigHasAPIKey || clearAPIKey) {
                    Text(model.localized("保留或输入 API Key 后可测试连接。"))
                        .font(MobileDesignTypography.labelSmall.font)
                        .foregroundStyle(MobileDesignColors.subtle)
                }
                if let failure = model.generalConfigFailure {
                    Text(configFailureText(failure))
                        .font(MobileDesignTypography.bodySmall.font).foregroundStyle(BitFunTheme.red)
                }
                if let message = model.generalConnectionTestMessage {
                    Text(message).font(MobileDesignTypography.bodySmall.font)
                        .foregroundStyle(message == model.localized("连接成功") ? BitFunTheme.green : BitFunTheme.red)
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 18)
            .padding(.bottom, 30)
        }
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(model.localized(title))
            .font(MobileDesignTypography.labelMedium.font)
            .foregroundStyle(BitFunTheme.muted)
    }

    private func modelOverviewRow(icon: String, title: String, subtitle: String, height: CGFloat) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon).font(.system(size: 23)).frame(width: 28, height: 28)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(MobileDesignTypography.bodyLarge.font.weight(.medium)).lineLimit(1)
                if !subtitle.isEmpty {
                    Text(subtitle).font(MobileDesignTypography.labelSmall.font).foregroundStyle(BitFunTheme.muted)
                }
            }
            Spacer()
        }
        .foregroundStyle(BitFunTheme.ink)
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: height)
        .background(BitFunTheme.soft)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCompactCardRadius))
    }

    private func sourceRow(icon: String, title: String, subtitle: String, chevronAction: (() -> Void)?) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon).font(.system(size: 21)).foregroundStyle(BitFunTheme.muted).frame(width: 28, height: 28)
            VStack(alignment: .leading, spacing: 3) {
                Text(model.localized(title)).font(MobileDesignTypography.titleSmall.font).foregroundStyle(BitFunTheme.ink).lineLimit(1)
                if !subtitle.isEmpty {
                    Text(subtitle).font(MobileDesignTypography.labelSmall.font).foregroundStyle(BitFunTheme.muted).lineLimit(1)
                }
            }
            Spacer()
            if chevronAction != nil {
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .medium)).foregroundStyle(BitFunTheme.muted)
            }
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, minHeight: MobileDesignGeometry.modelSourceRowHeight)
    }

    private func sourceLabel(_ source: String) -> String {
        model.localized(source == "LOCAL" ? "本机" : "云端账号")
    }

    @ViewBuilder
    private func labeledField(_ label: String, placeholder: String, text: Binding<String>, secure: Bool) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(model.localized(label))
                .font(MobileDesignTypography.labelMedium.font)
                .foregroundStyle(BitFunTheme.ink)
            Group {
                if secure { SecureField(model.localized(placeholder), text: text) }
                else { TextField(model.localized(placeholder), text: text) }
            }
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .font(MobileDesignTypography.bodyMedium.font)
            .padding(.horizontal, 14)
            .frame(height: 52)
            .background(BitFunTheme.soft)
            .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCompactCardRadius))
        }
    }

    private func editorAction(title: String, primary: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(model.localized(title))
                .font(MobileDesignTypography.bodyLarge.font.weight(.medium))
                .foregroundStyle(primary ? Color.white : BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 50)
                .background(primary ? BitFunTheme.accent : BitFunTheme.soft)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    private func configFailureText(_ failure: String) -> String {
        switch failure {
        case "INVALID_URL": model.localized("请输入有效的服务地址")
        case "MODEL_REQUIRED": model.localized("请输入模型名称")
        case "API_KEY_REQUIRED": model.localized("请输入 API Key")
        default: model.localized("配置无法保存，请稍后重试")
        }
    }
}

private struct PermissionModeRow: View {
    @ObservedObject var model: MobileAppModel
    let mode: String
    let title: String
    let detail: String

    var body: some View {
        Button { model.setRemotePermissionMode(mode) } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.localized(title)).font(MobileDesignTypography.bodyMedium.font).foregroundStyle(BitFunTheme.ink)
                    Text(model.localized(detail)).font(MobileDesignTypography.labelSmall.font).foregroundStyle(BitFunTheme.muted)
                }
                Spacer()
                if model.remotePermissionMode == mode {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(BitFunTheme.green)
                }
            }
            .padding(.horizontal, 20).frame(minHeight: 62)
        }
        .buttonStyle(.plain).disabled(model.busy)
    }
}

private struct SettingsGroup<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(MobileLocalization.text(title))
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.leading, 12)
            SettingsCard(content: content)
        }
        .padding(.bottom, 24)
    }
}

private struct SettingsCard<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        BitFunModalCard(
            radius: MobileDesignGeometry.settingsCompactCardRadius,
            bordered: false,
            content: content
        )
    }
}

private struct SettingsProfileRow: View {
    let subtitle: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: 24, weight: .regular))
                .foregroundStyle(BitFunTheme.muted)
                .frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 2) {
                Text(MobileLocalization.text("个人资料"))
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                Text(MobileLocalization.text(subtitle))
                    .font(.system(size: 13))
                    .foregroundStyle(BitFunTheme.muted)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            Image(systemName: "chevron.right")
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(BitFunTheme.muted.opacity(0.72))
        }
        .padding(.horizontal, 18)
        .frame(height: 64)
    }
}

private struct SettingsValueRow: View {
    let icon: String?
    let title: String
    let value: String
    var showsChevron: Bool = false

    var body: some View {
        HStack(spacing: 14) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 20, weight: .regular))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(width: 23, height: 23)
            }
            Text(MobileLocalization.text(title))
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(BitFunTheme.ink)
            Spacer(minLength: 12)
            Text(MobileLocalization.text(value))
                .font(.system(size: 15))
                .foregroundStyle(BitFunTheme.muted)
                .lineLimit(1)
            if showsChevron {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted.opacity(0.72))
            }
        }
        .padding(.horizontal, 18)
        .frame(height: 52)
    }
}

private struct SettingsDeviceRow: View {
    let device: MobileAccountDevice

    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: "desktopcomputer")
                .font(.system(size: 21, weight: .regular))
                .foregroundStyle(BitFunTheme.muted)
                .frame(width: 42, height: 42)
            VStack(alignment: .leading, spacing: 3) {
                Text(device.name)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Text(MobileLocalization.text(device.online ? "在线" : "离线"))
                    .font(.system(size: 12))
                    .foregroundStyle(device.online ? BitFunTheme.green : BitFunTheme.muted)
            }
            Spacer(minLength: 12)
            if device.selected {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 18))
                    .foregroundStyle(BitFunTheme.green)
            } else {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
            }
        }
        .padding(.horizontal, 20)
        .frame(minHeight: 76)
    }
}
