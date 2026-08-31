import SwiftUI

struct RemoteViewSettingsView: View {
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
struct RemoteControlSettingsView: View {
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
                        .font(.system(size: 12)).foregroundStyle(BitFunTheme.statusDanger)
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
                .font(.system(size: 15, weight: .bold)).foregroundStyle(BitFunTheme.statusDanger)
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
        .overlay(RoundedRectangle(cornerRadius: 18).stroke(BitFunTheme.statusDanger, lineWidth: 1))
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
                .foregroundStyle(destructive ? BitFunTheme.contentOnAction : BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 42)
                .background(destructive ? BitFunTheme.statusDanger : BitFunTheme.soft)
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

struct GeneralChatConfigSheet: View {
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
                                        .foregroundStyle(option.selected ? BitFunTheme.ink : BitFunTheme.transparent)
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
                                .background(option.selected ? BitFunTheme.soft : BitFunTheme.transparent)
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
                            .foregroundStyle(clearAPIKey ? BitFunTheme.ink : BitFunTheme.statusDanger)
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
                        .font(MobileDesignTypography.bodySmall.font).foregroundStyle(BitFunTheme.statusDanger)
                }
                if let message = model.generalConnectionTestMessage {
                    Text(message).font(MobileDesignTypography.bodySmall.font)
                        .foregroundStyle(message == model.localized("连接成功") ? BitFunTheme.statusSuccess : BitFunTheme.statusDanger)
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
                .foregroundStyle(primary ? BitFunTheme.contentOnAction : BitFunTheme.ink)
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
