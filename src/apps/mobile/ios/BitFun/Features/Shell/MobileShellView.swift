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
                .accessibilityElement(children: .contain)
                .accessibilityIdentifier("conversation.draft")
            } else {
                conversationContent(
                    sidebarAction: sidebarAction,
                    sidebarActionLabel: sidebarActionLabel
                )
                .ignoresSafeArea(.keyboard, edges: .bottom)
            }
        }
        .background(BitFunTheme.page)
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
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(conversationAccessibilityIdentifier)
    }

    private var conversationAccessibilityIdentifier: String {
        guard model.surface == .remote else { return "conversation.local" }
        let sessionID = model.selectedSessionID
        guard model.remoteSessionSelected,
              !sessionID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return "conversation.draft"
        }
        return "conversation.session.\(sessionID)"
    }

    @ViewBuilder
    private func paneSeparator(width: CGFloat) -> some View {
        if width > 0 {
            Rectangle().fill(BitFunTheme.line).frame(width: width)
        }
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

struct SettingsCard<Content: View>: View {
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
