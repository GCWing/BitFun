import SwiftUI

private func normalizedDeviceKey(_ key: String?) -> String? {
    guard let key else { return nil }
    if key == "pairing" { return key }
    return key.hasPrefix("account:") ? String(key.dropFirst("account:".count)) : key
}

struct SidebarSessionActionsAnchorKey: PreferenceKey {
    static var defaultValue: [String: Anchor<CGRect>] = [:]

    static func reduce(
        value: inout [String: Anchor<CGRect>],
        nextValue: () -> [String: Anchor<CGRect>]
    ) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}

struct SidebarWorkspaceCreateAnchorKey: PreferenceKey {
    static var defaultValue: [String: Anchor<CGRect>] = [:]

    static func reduce(
        value: inout [String: Anchor<CGRect>],
        nextValue: () -> [String: Anchor<CGRect>]
    ) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}

struct SidebarView: View {
    @ObservedObject var model: MobileAppModel
    var permanent = false
    var onCollapse: (() -> Void)? = nil
    var onPermanentActions: ((ChatSession) -> Void)? = nil
    @State private var search = ""
    @State private var searchVisible = false
    @State private var visibleRecentCount = 6
    @State private var expandedWorkspacePaths: Set<String> = []
    @State private var expandedDeviceWorkspaceLists: Set<String> = []
    @State private var compactActionSession: ChatSession?
    @State private var workspaceCreatePath: String?
    @State private var remoteChatsCollapsed = false

    private var recentSessions: [ChatSession] {
        let source = model.sessions
        guard !search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return source }
        return source.filter { $0.title.localizedCaseInsensitiveContains(search) }
    }

    private var shownRecentSessions: [ChatSession] {
        Array(recentSessions.prefix(search.isEmpty ? visibleRecentCount : recentSessions.count))
    }

    private var hasActiveRemoteViewFilter: Bool {
        !model.remoteWorkspaceFilter.isEmpty ||
            !model.remoteViewAgentFilter.isEmpty ||
            !model.remoteStatusFilter.isEmpty
    }

    private var directoryEntries: [MobileDeviceDirectoryEntry] {
        var entries = model.deviceDirectory
        if let direct = model.directPairingDirectoryEntry,
           !entries.contains(where: { $0.id == direct.id }) {
            entries.insert(direct, at: 0)
        }
        return entries
    }

    var body: some View {
        GeometryReader { proxy in
            VStack(alignment: .leading, spacing: 0) {
                if model.accountUser == nil { signedOutHeader } else { authenticatedHeader }
                if searchVisible {
                    searchField
                }
                ScrollView(showsIndicators: false) {
                    VStack(alignment: .leading, spacing: 0) {
                        recentSection
                        workspaceSection
                    }
                    .padding(.bottom, model.accountUser == nil && !model.remoteConnected ? 142 : 84)
                }
                footer
            }
            .padding(.horizontal, 20)
            .padding(.top, 4)
            .padding(.bottom, 16)
            .frame(
                width: proxy.size.width,
                height: proxy.size.height,
                alignment: .topLeading
            )
            .background(BitFunTheme.page)
        }
        .sheet(item: $compactActionSession) { session in
            let surface = SessionActionSurface(
                model: model,
                session: session,
                presentation: .bottomSheet,
                canViewDetails: true,
                canArchive: model.surface == .local,
                canExport: model.surface == .local,
                canDelete: true,
                onViewDetails: { openDetails(afterClosing: session) },
                onArchive: { if model.surface == .local { model.archiveLocalSession(session) } },
                onExport: { if model.surface == .local { model.exportLocalSession(session) } },
                onDelete: {
                    if model.surface == .remote { model.deleteRemoteSession(session) }
                    else { model.deleteLocalSession(session) }
                },
                onClose: { compactActionSession = nil }
            )
            .presentationDetents([.height(380)])
            .presentationDragIndicator(.hidden)
            if #available(iOS 16.4, *) {
                surface.presentationCornerRadius(MobileDesignGeometry.popoverRadius)
            } else {
                surface
            }
        }
        .overlayPreferenceValue(SidebarWorkspaceCreateAnchorKey.self) { anchors in
            GeometryReader { proxy in
                if let path = workspaceCreatePath,
                   let workspace = model.remoteWorkspaces.first(where: { $0.path == path }),
                   let anchor = anchors[path] {
                    let frame = proxy[anchor]
                    let menuHeight = MobileDesignGeometry.compactPopoverActionHeight * 2 + 16
                    ZStack(alignment: .topLeading) {
                        BitFunTheme.transparent
                            .contentShape(Rectangle())
                            .onTapGesture { workspaceCreatePath = nil }
                        workspaceCreateMenu(workspace)
                            .position(
                                x: min(
                                    max(MobileDesignGeometry.compactPopoverWidth / 2 + 8, frame.midX),
                                    proxy.size.width - MobileDesignGeometry.compactPopoverWidth / 2 - 8
                                ),
                                y: max(menuHeight / 2 + 8, frame.minY - menuHeight / 2 - 6)
                            )
                    }
                }
            }
        }
        .task {
            if ProcessInfo.processInfo.arguments.contains("--project-create-menu"),
               workspaceCreatePath == nil,
               let workspace = model.remoteWorkspaces.first {
                try? await Task.sleep(nanoseconds: 450_000_000)
                workspaceCreatePath = workspace.path
            } else if ProcessInfo.processInfo.arguments.contains("--sidebar-actions"),
                      compactActionSession == nil,
                      let session = shownRecentSessions.first {
                try? await Task.sleep(nanoseconds: 450_000_000)
                if permanent { onPermanentActions?(session) }
                else { compactActionSession = session }
            }
        }
    }

    private var authenticatedHeader: some View {
        HStack(spacing: 6) {
            Text(verbatim: "BitFun")
                .font(.system(size: 20, weight: .bold))
                .foregroundStyle(BitFunTheme.ink)
            Spacer(minLength: 0)
            if let onCollapse {
                Button(action: onCollapse) {
                    Image(systemName: "sidebar.left")
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 38, height: 38)
                        .background(BitFunTheme.card)
                        .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("收起侧栏")))
            }
            if model.remoteConnected {
                Button { model.remoteViewSettingsOpen = true } label: {
                    Image(systemName: "ellipsis")
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 38, height: 38)
                        .background(BitFunTheme.card)
                        .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("视图设置")))
            }
            Button {
                withAnimation(.easeOut(duration: 0.18)) { searchVisible.toggle() }
                if !searchVisible { search = "" }
            } label: {
                ReferenceImage(assetName: "SidebarSearchGlyph", width: 22, height: 22)
                    .frame(width: 38, height: 38)
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: BitFunTheme.shadowMedium, radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text(model.localized("搜索")))
        }
        .frame(height: 50)
    }

    private var signedOutHeader: some View {
        HStack(spacing: 8) {
            Button { model.newLocalChat() } label: {
                HStack(spacing: 8) {
                    Image(systemName: "square.and.pencil")
                        .font(.system(size: 17, weight: .medium))
                    Text(model.localized("聊天"))
                        .font(.system(size: 15, weight: .medium))
                }
                .foregroundStyle(BitFunTheme.ink)
                .frame(height: 42)
            }
            .buttonStyle(.plain)
            Spacer(minLength: 0)
            if let onCollapse {
                Button(action: onCollapse) {
                    Image(systemName: "sidebar.left")
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 38, height: 38)
                        .background(BitFunTheme.card)
                        .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("收起侧栏")))
            }
        }
        .frame(height: 50)
    }

    private var searchField: some View {
        TextField(model.localized("搜索对话"), text: $search)
            .font(.system(size: 14))
            .foregroundStyle(BitFunTheme.ink)
            .padding(.horizontal, 14)
            .frame(height: 42)
            .background(BitFunTheme.soft)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .padding(.top, 12)
            .onChange(of: search) { value in
                if model.surface == .remote { model.searchRemoteSessions(value) }
            }
    }

    private var recentSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(model.localized("最近对话"))
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.top, 16)
                .padding(.bottom, 6)
            if shownRecentSessions.isEmpty {
                Text(model.localized(search.isEmpty ? "暂无最近会话" : "没有匹配的会话"))
                    .font(.system(size: 13))
                    .foregroundStyle(BitFunTheme.muted)
                    .padding(.horizontal, 12)
                    .frame(height: 44, alignment: .leading)
            }
            ForEach(shownRecentSessions) { session in
                SidebarRecentRow(
                    model: model,
                    session: session,
                    selected: model.surface == .local && session.id == model.selectedSessionID,
                    onOpen: {
                        model.surface = .local
                        model.select(session)
                    },
                    onActions: {
                        model.surface = .local
                        if permanent { onPermanentActions?(session) }
                        else { compactActionSession = session }
                    }
                )
            }
            if visibleRecentCount < recentSessions.count && search.isEmpty {
                Button {
                    visibleRecentCount = min(visibleRecentCount + 6, recentSessions.count)
                } label: {
                    HStack(spacing: 8) {
                    Text(verbatim: "···")
                        Text(
                            model.localizedFormat(
                                "还有 %lld 个会话",
                                Int64(recentSessions.count - visibleRecentCount)
                            )
                        )
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .frame(height: 40, alignment: .leading)
                .padding(.leading, 12)
            }
        }
    }

    private func openDetails(afterClosing session: ChatSession) {
        compactActionSession = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.24) {
            model.showSessionDetails(session)
        }
    }

    private var workspaceSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(model.localized("设备"))
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
                Spacer()
                if model.accountUser != nil {
                    Button { model.refreshRemoteDevices() } label: {
                        if model.accountRefreshing {
                            ProgressView().controlSize(.small)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 14, weight: .medium))
                                .foregroundStyle(BitFunTheme.muted)
                        }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text(model.localized("刷新设备")))
                }
                Button { model.scanRemote() } label: {
                    ReferenceImage(assetName: "SidebarPlusGlyph", width: 17, height: 20)
                        .frame(width: 32, height: 32)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("添加连接")))
            }
            .frame(height: 38)
            .padding(.top, 18)

            if directoryEntries.isEmpty {
                Text(model.localized("尚未连接桌面设备"))
                    .font(.system(size: 13))
                    .foregroundStyle(BitFunTheme.muted)
                    .padding(.horizontal, 10)
                    .frame(height: 42, alignment: .leading)
            }

            ForEach(directoryEntries) { device in
                directoryDevice(device)
            }

        }
    }

    @ViewBuilder
    private func directoryDevice(_ device: MobileDeviceDirectoryEntry) -> some View {
        let current = model.accountSelectedDeviceID == device.id || device.id == model.directPairingSidebarDeviceID
        VStack(alignment: .leading, spacing: 0) {
            Button { model.toggleDeviceDirectory(device) } label: {
                HStack(spacing: 10) {
                    ReferenceImage(assetName: "SidebarDeviceGlyph", width: 22, height: 18)
                    Text(device.name).font(.system(size: 15, weight: current ? .medium : .regular))
                        .foregroundStyle(BitFunTheme.ink).lineLimit(1)
                    Spacer(minLength: 0)
                    Circle().fill(device.online ? BitFunTheme.statusSuccess : BitFunTheme.muted).frame(width: 7, height: 7)
                    if current { Text(model.localized("当前控制")).font(.system(size: 11)).foregroundStyle(BitFunTheme.statusSuccess) }
                    if device.status == "LOADING" { ProgressView().controlSize(.small) }
                    Image(systemName: device.expanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 12, weight: .medium)).foregroundStyle(BitFunTheme.muted)
                }
                .padding(.horizontal, 10).frame(minHeight: 46).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("sidebar.device.\(device.id)")
            .accessibilityLabel(Text(device.name))
            .accessibilityValue(Text(device.online ? model.localized("在线") : model.localized("离线")))
            if device.expanded { directoryDeviceBody(device) }
        }
    }

    @ViewBuilder
    private func directoryDeviceBody(_ device: MobileDeviceDirectoryEntry) -> some View {
        if device.status == "LOADING" && device.workspaces.isEmpty && device.sessions.isEmpty {
            HStack(spacing: 8) { ProgressView().controlSize(.small); Text(model.localized("正在加载工作区")).font(.system(size: 13)).foregroundStyle(BitFunTheme.muted) }
                .padding(.horizontal, 18).frame(height: 42)
        } else if device.status == "FAILED" {
            Button { model.retryDeviceDirectory(device) } label: {
                Text(model.localized("工作区加载失败，点按重试")).font(.system(size: 13)).foregroundStyle(BitFunTheme.statusDanger)
                    .frame(maxWidth: .infinity, minHeight: 42, alignment: .leading).padding(.leading, 18)
            }.buttonStyle(.plain)
        } else if device.status == "READY" && device.online && device.workspaces.isEmpty && device.sessions.isEmpty {
            Text(model.localized("这台电脑还没有工作区"))
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.horizontal, 18)
                .frame(maxWidth: .infinity, minHeight: 42, alignment: .leading)
                .accessibilityIdentifier("sidebar.emptyWorkspaces")
        }
        ForEach((expandedDeviceWorkspaceLists.contains(device.id) ? device.workspaces : Array(device.workspaces.prefix(3)))) { workspace in
            let scopedWorkspace = MobileWorkspaceGroup(
                path: workspace.path,
                name: workspace.name,
                selected: workspace.selected,
                sessions: workspace.sessions.map { session in
                    var scopedSession = session
                    scopedSession.deviceKey = device.id
                    return scopedSession
                },
                deviceKey: device.id
            )
            SidebarWorkspaceRow(
                workspace: scopedWorkspace,
                expanded: expandedWorkspacePaths.contains(workspace.id),
                selectedSessionID: model.surface == .remote ? model.selectedSessionID : nil,
                metadata: { _ in nil },
                onToggle: { if expandedWorkspacePaths.contains(workspace.id) { expandedWorkspacePaths.remove(workspace.id) } else { expandedWorkspacePaths.insert(workspace.id) } },
                onToggleCreate: {
                    model.openDirectoryRemoteDraft(device: device, workspace: scopedWorkspace)
                },
                onOpenWorkspace: { model.selectDirectoryWorkspace(scopedWorkspace) },
                onOpenSession: { model.selectDirectorySession($0) }, onActions: { session in
                    if permanent { onPermanentActions?(session) } else { compactActionSession = session }
                },
                sessionLimit: expandedWorkspacePaths.contains(workspace.id) ? workspace.sessions.count : 3,
                selectedDeviceKey: model.accountSelectedDeviceID,
                selectedWorkspacePath: model.workspaceCatalog.first(where: { $0.selected })?.path,
                onShowMore: { expandedWorkspacePaths.insert(workspace.id) }
            )
            .padding(.leading, 20)
        }
        if device.workspaces.count > 3 {
            Button {
                expandedDeviceWorkspaceLists.insert(device.id)
            } label: {
                Text(model.localizedFormat("还有 %lld 个工作区", Int64(device.workspaces.count - 3)))
                    .font(.system(size: 13)).foregroundStyle(BitFunTheme.muted).padding(.leading, 42).frame(height: 36, alignment: .leading)
            }.buttonStyle(.plain)
        }
    }

    @ViewBuilder
    private var activeRemoteDeviceBody: some View {
        if model.workspaceLoading && model.remoteWorkspaces.isEmpty {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(model.localized("正在加载工作区"))
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.muted)
                }
                .padding(.horizontal, 10)
                .frame(height: 42)
        } else if model.workspaceLoadFailed && model.remoteWorkspaces.isEmpty {
                Button { model.retryRemoteWorkspaces() } label: {
                    Text(model.localized("工作区加载失败，点按重试"))
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.statusDanger)
                        .frame(maxWidth: .infinity, minHeight: 42, alignment: .leading)
                        .padding(.horizontal, 10)
                }
                .buttonStyle(.plain)
        }

        if model.remoteConnected {
            remoteGroupedSessionSections(model.sessionListSections)
        }
        if model.remoteHasMore {
            Button { model.loadMoreRemoteSessions() } label: {
                Text(model.localized(model.busy ? "正在加载" : "加载更多会话"))
                    .font(.system(size: 13)).foregroundStyle(BitFunTheme.muted)
                    .frame(maxWidth: .infinity, minHeight: 42)
            }
            .buttonStyle(.plain).disabled(model.busy)
        }
    }

    @ViewBuilder
    private func remoteGroupedSessionSections(
        _ sections: [MobileSessionListSectionProjection]
    ) -> some View {
        let visibleSessions = sections.flatMap(\.sessions)
        let chatSessions = sections.first(where: { $0.kind == .chat })?.sessions ?? []
        if visibleSessions.isEmpty && !model.workspaceLoading {
            Text(model.localized(hasActiveRemoteViewFilter ? "没有匹配的会话" : "暂无远程会话"))
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.horizontal, 10)
                .frame(height: 42, alignment: .leading)
        } else {
            switch model.remoteGroupMode {
            case "TIME":
                remoteTimeSections(sections)
            case "CHAT":
                if !chatSessions.isEmpty { remoteChatSection(chatSessions) }
                remoteProjectSections(sections)
            default:
                remoteProjectSections(sections)
                if !chatSessions.isEmpty { remoteChatSection(chatSessions) }
            }
        }
    }

    private func remoteProjectSections(
        _ sections: [MobileSessionListSectionProjection]
    ) -> some View {
        let workspaces = sections.compactMap { section -> MobileWorkspaceGroup? in
            guard section.kind == .project else { return nil }
            let source = model.remoteWorkspaces.first {
                normalizedWorkspacePath($0.path) == normalizedWorkspacePath(section.path)
            }
            return MobileWorkspaceGroup(
                path: section.path,
                name: section.name,
                selected: source?.selected ?? false,
                sessions: section.sessions,
                deviceKey: normalizedDeviceKey(model.remoteExpectedDeviceKey)
            )
        }
        return ForEach(workspaces) { workspace in
            SidebarWorkspaceRow(
                workspace: workspace,
                expanded: expandedWorkspacePaths.contains(workspace.path) || workspace.selected,
                selectedSessionID: model.surface == .remote ? model.selectedSessionID : nil,
                metadata: remoteSessionMetadata,
                onToggle: {
                    if expandedWorkspacePaths.contains(workspace.path) {
                        expandedWorkspacePaths.remove(workspace.path)
                    } else {
                        expandedWorkspacePaths.insert(workspace.path)
                    }
                },
                onToggleCreate: {
                    workspaceCreatePath = workspaceCreatePath == workspace.path ? nil : workspace.path
                },
                onOpenWorkspace: { model.selectRemoteWorkspace(workspace) },
                onOpenSession: { model.surface = .remote; model.select($0) },
                onActions: { session in
                    model.surface = .remote
                    if permanent { onPermanentActions?(session) }
                    else { compactActionSession = session }
                },
                selectedDeviceKey: normalizedDeviceKey(model.remoteExpectedDeviceKey),
                selectedWorkspacePath: model.workspaceCatalog.first(where: { $0.selected })?.path
            )
        }
    }

    private func remoteTimeSections(
        _ sections: [MobileSessionListSectionProjection]
    ) -> some View {
        let buckets = sections.compactMap { section -> RemoteTimeBucket? in
            switch section.kind {
            case .today: return RemoteTimeBucket(id: section.id, title: "sidebar.time.today", sessions: section.sessions)
            case .yesterday: return RemoteTimeBucket(id: section.id, title: "sidebar.time.yesterday", sessions: section.sessions)
            case .earlier: return RemoteTimeBucket(id: section.id, title: "sidebar.time.older", sessions: section.sessions)
            default: return nil
            }
        }
        return ForEach(buckets) { bucket in
            VStack(alignment: .leading, spacing: 0) {
                Text(model.localized(bucket.title))
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
                    .padding(.top, 12)
                    .padding(.bottom, 4)
                ForEach(bucket.sessions) { session in
                    SidebarRecentRow(
                        model: model,
                        session: session,
                        selected: model.surface == .remote && session.id == model.selectedSessionID,
                        metadata: remoteSessionMetadata(session),
                        onOpen: { model.surface = .remote; model.select(session) },
                        onActions: {
                            model.surface = .remote
                            if permanent { onPermanentActions?(session) }
                            else { compactActionSession = session }
                        }
                    )
                }
            }
        }
    }

    private func remoteChatSection(_ sessions: [ChatSession]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Button {
                    withAnimation(.easeOut(duration: 0.18)) { remoteChatsCollapsed.toggle() }
                } label: {
                    HStack(spacing: 8) {
                        Text(model.localized("聊天"))
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(BitFunTheme.muted)
                        Text(verbatim: "\(sessions.count)")
                            .font(.system(size: 12))
                            .foregroundStyle(BitFunTheme.muted)
                        Image(systemName: remoteChatsCollapsed ? "chevron.right" : "chevron.down")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(BitFunTheme.muted)
                    }
                }
                .buttonStyle(.plain)
                Spacer(minLength: 0)
                Button { model.createRemoteAssistantSession() } label: {
                    Image(systemName: "square.and.pencil")
                        .font(.system(size: 16, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .disabled(model.busy)
                .accessibilityLabel(Text(model.localized("新建远程会话")))
            }
            .frame(height: 44)

            if !remoteChatsCollapsed {
                ForEach(sessions.prefix(4)) { session in
                    SidebarRecentRow(
                        model: model,
                        session: session,
                        selected: model.surface == .remote && session.id == model.selectedSessionID,
                        metadata: remoteSessionMetadata(session),
                        onOpen: { model.surface = .remote; model.select(session) },
                        onActions: {
                            model.surface = .remote
                            if permanent { onPermanentActions?(session) }
                            else { compactActionSession = session }
                        }
                    )
                }
            }
        }
        .padding(.top, 8)
    }

    private func remoteIsAssistant(_ session: ChatSession) -> Bool {
        let agent = session.agentType.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if ["claw", "assistant", "chat"].contains(agent) { return true }
        let path = normalizedWorkspacePath(session.workspacePath)
        return !path.isEmpty && model.remoteAssistants.contains {
            normalizedWorkspacePath($0.path) == path
        }
    }

    private func remoteWorkspacePath(_ session: ChatSession) -> String {
        let own = normalizedWorkspacePath(session.workspacePath)
        if !own.isEmpty { return own }
        if remoteIsAssistant(session) { return "" }
        return normalizedWorkspacePath(model.remoteWorkspaces.first(where: \.selected)?.path)
    }

    private func normalizedWorkspacePath(_ path: String?) -> String {
        var result = (path ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        while result.count > 1 && (result.hasSuffix("/") || result.hasSuffix("\\")) {
            result.removeLast()
        }
        return result
    }

    private func remoteSessionMetadata(_ session: ChatSession) -> String? {
        var parts: [String] = []
        if model.remoteShowWorkspaceMetadata {
            let name = session.workspaceName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let path = remoteWorkspacePath(session)
            if !name.isEmpty { parts.append(name) }
            else if !path.isEmpty { parts.append(path) }
        }
        if model.remoteShowUpdatedMetadata, !session.updatedLabel.isEmpty {
            parts.append(relativeUpdatedLabel(session))
        }
        if model.remoteShowStatusMetadata, !session.status.isEmpty {
            parts.append(remoteStatusLabel(session.status))
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    private func remoteStatusLabel(_ status: String) -> String {
        switch status.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "active", "running": return model.localized("运行中")
        case "ready", "idle": return model.localized("就绪")
        case "archived": return model.localized("已归档")
        default: return status
        }
    }

    private func relativeUpdatedLabel(_ session: ChatSession) -> String {
        let date = remoteSessionDate(session)
        guard date != .distantPast else { return session.updatedLabel }
        if abs(date.timeIntervalSinceNow) < 60 { return model.localized("刚刚") }
        let formatter = RelativeDateTimeFormatter()
        formatter.locale = Locale(identifier: model.appLanguage.rawValue)
        formatter.unitsStyle = .full
        return formatter.localizedString(for: date, relativeTo: Date())
    }

    private func remoteSessionDate(_ session: ChatSession) -> Date {
        parsedRemoteDate(session.updatedLabel) ?? parsedRemoteDate(session.createdAt) ?? .distantPast
    }

    private func parsedRemoteDate(_ value: String) -> Date? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        if let numeric = Double(trimmed) {
            return Date(timeIntervalSince1970: numeric > 10_000_000_000 ? numeric / 1_000 : numeric)
        }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = fractional.date(from: trimmed) { return date }
        return ISO8601DateFormatter().date(from: trimmed)
    }

    private func pairedDeviceRow(name: String) -> some View {
        Button { model.openRemoteSurface() } label: {
            HStack(spacing: 10) {
                ReferenceImage(assetName: "SidebarDeviceGlyph", width: 22, height: 18)
                Text(name)
                    .font(.system(size: 15))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Circle().fill(BitFunTheme.statusSuccess).frame(width: 7, height: 7)
                ReferenceImage(assetName: "SidebarDownGlyph", width: 14, height: 14)
            }
            .padding(.horizontal, 10)
            .frame(height: 46)
        }
        .buttonStyle(.plain)
    }

    private func workspaceCreateMenu(_ workspace: MobileWorkspaceGroup) -> some View {
        VStack(spacing: 0) {
            workspaceCreateMenuRow("Code") {
                workspaceCreatePath = nil
                model.createRemoteSession(in: workspace, agentType: "code")
            }
            workspaceCreateMenuRow("Cowork") {
                workspaceCreatePath = nil
                model.createRemoteSession(in: workspace, agentType: "Cowork")
            }
        }
        .bitFunCompactPopoverSurface()
    }

    private func workspaceCreateMenuRow(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(verbatim: title)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: MobileDesignGeometry.compactPopoverActionHeight, alignment: .leading)
                .padding(.horizontal, 18)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var footer: some View {
        Group {
            if model.accountUser == nil {
                SignedOutConnectionActions(
                    scanTitle: model.localized("扫码连接"),
                    accountTitle: model.localized("登录 BitFun 账号"),
                    onScan: model.scanRemote,
                    onOpenAccount: { model.accountSheetOpen = true; model.drawerOpen = false },
                    showScan: !model.remoteConnected
                )
            } else {
                authenticatedFooter
            }
        }
    }

    private var authenticatedFooter: some View {
        HStack(spacing: 0) {
            Button { model.newLocalChat() } label: {
                HStack(spacing: 9) {
                    ReferenceImage(assetName: "SidebarEditGlyph", width: 24, height: 24)
                    Text(model.localized("聊天"))
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                }
                .frame(width: 116, height: 46)
                .background(BitFunTheme.card)
                .overlay(RoundedRectangle(cornerRadius: 23).stroke(BitFunTheme.line, lineWidth: 1))
                .clipShape(Capsule())
                .shadow(color: BitFunTheme.shadowMedium, radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            Spacer(minLength: 0)
            Button { model.settingsOpen = true; model.drawerOpen = false } label: {
                ReferenceImage(assetName: "SidebarSettingsGlyph", width: 24, height: 24)
                    .frame(width: 46, height: 46)
                    .background(BitFunTheme.card)
                    .clipShape(Circle())
                    .shadow(color: BitFunTheme.shadowMedium, radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text(model.localized("设置")))
        }
        .frame(height: 56)
    }
}

private struct RemoteTimeBucket: Identifiable {
    let id: String
    let title: String
    let sessions: [ChatSession]
}

private struct SidebarRecentRow: View {
    @ObservedObject var model: MobileAppModel
    let session: ChatSession
    let selected: Bool
    var metadata: String? = nil
    let onOpen: () -> Void
    let onActions: () -> Void
    var body: some View {
        HStack(spacing: 0) {
            Button(action: onOpen) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(session.title)
                        .font(.system(size: 15, weight: selected ? .medium : .regular))
                        .foregroundStyle(BitFunTheme.ink)
                        .lineLimit(1)
                    if let metadata, !metadata.isEmpty {
                        Text(metadata)
                            .font(MobileDesignTypography.labelSmall.font)
                            .foregroundStyle(BitFunTheme.muted)
                            .lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("sidebar.recentSession.\(session.id)")

            Button {
                onActions()
            } label: {
                HStack(spacing: 3) {
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                }
                .frame(width: 34, height: 40)
                .opacity(0.62)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text(model.localized("会话操作")))
            .anchorPreference(
                key: SidebarSessionActionsAnchorKey.self,
                value: .bounds,
                transform: { [session.id: $0] }
            )
        }
        .padding(.leading, 12)
        .padding(.trailing, 4)
        .frame(minHeight: metadata == nil ? 44 : 56)
        .background(selected ? BitFunTheme.soft : BitFunTheme.transparent)
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }
}

private struct SidebarWorkspaceRow: View {
    let workspace: MobileWorkspaceGroup
    let expanded: Bool
    let selectedSessionID: String?
    let metadata: (ChatSession) -> String?
    let onToggle: () -> Void
    let onToggleCreate: () -> Void
    let onOpenWorkspace: () -> Void
    let onOpenSession: (ChatSession) -> Void
    let onActions: (ChatSession) -> Void
    var sessionLimit: Int = 3
    var selectedDeviceKey: String? = nil
    var selectedWorkspacePath: String? = nil
    var onShowMore: (() -> Void)? = nil

    private func isSelected(_ session: ChatSession) -> Bool {
        guard selectedSessionID == session.id,
              normalizedDeviceKey(selectedDeviceKey) == normalizedDeviceKey(workspace.deviceKey) else { return false }
        func normalized(_ path: String?) -> String {
            var value = (path ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            while value.count > 1 && value.hasSuffix("/") { value.removeLast() }
            return value
        }
        return normalized(selectedWorkspacePath) == normalized(workspace.path)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Button(action: onOpenWorkspace) {
                    HStack(spacing: 10) {
                        ReferenceImage(assetName: "SidebarFolderGlyph", width: 24, height: 20)
                        Text(workspace.name)
                            .font(.system(size: 15, weight: workspace.selected ? .medium : .regular))
                            .foregroundStyle(BitFunTheme.ink)
                            .lineLimit(1)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("sidebar.workspace.\(workspace.deviceKey ?? "unknown").\(workspace.path)")
                .accessibilityValue(Text(workspace.path))
                Spacer(minLength: 0)
                Button(action: onToggleCreate) {
                    Image(systemName: "square.and.pencil")
                        .font(.system(size: 16, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 32, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(MobileLocalization.text("新建远程会话"))
                .accessibilityIdentifier("sidebar.newSession.\(workspace.deviceKey ?? "unknown").\(workspace.path)")
                .anchorPreference(
                    key: SidebarWorkspaceCreateAnchorKey.self,
                    value: .bounds,
                    transform: { [workspace.path: $0] }
                )
                Button(action: onToggle) {
                    ReferenceImage(
                        assetName: expanded ? "SidebarDownGlyph" : "SidebarChevronGlyph",
                        width: 14,
                        height: 14
                    )
                    .opacity(0.62)
                    .frame(width: 32, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(
                    MobileLocalization.text(expanded ? "收起工作区" : "展开工作区")
                )
            }
            .padding(.horizontal, 10)
            .frame(height: 46)
            .background(workspace.selected ? BitFunTheme.soft.opacity(0.75) : BitFunTheme.transparent)
            .clipShape(RoundedRectangle(cornerRadius: 10))

            if expanded {
                if workspace.sessions.isEmpty {
                    Text(MobileLocalization.text("此工作区暂无会话"))
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.muted)
                        .padding(.leading, 42)
                        .frame(height: 38, alignment: .leading)
                }
                ForEach(workspace.sessions.prefix(sessionLimit)) { session in
                    HStack(spacing: 0) {
                        Button { onOpenSession(session) } label: {
                            HStack(spacing: 10) {
                            if ["running", "active", "in_progress"].contains(session.status.lowercased()) {
                                Circle().fill(BitFunTheme.statusSuccess).frame(width: 7, height: 7)
                            }
                            Image(systemName: "doc")
                                .font(.system(size: 18, weight: .regular))
                                .foregroundStyle(BitFunTheme.muted)
                                .frame(width: 22)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(session.title)
                                    .font(.system(
                                        size: 15,
                                        weight: isSelected(session) ? .medium : .regular
                                    ))
                                    .foregroundStyle(BitFunTheme.ink)
                                    .lineLimit(1)
                                if let detail = metadata(session), !detail.isEmpty {
                                    Text(detail)
                                        .font(MobileDesignTypography.labelSmall.font)
                                        .foregroundStyle(BitFunTheme.muted)
                                        .lineLimit(1)
                                }
                            }
                            Spacer(minLength: 0)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("sidebar.session.\(workspace.deviceKey ?? session.deviceKey ?? "unknown").\(session.id)")
                        .accessibilityAddTraits(isSelected(session) ? .isSelected : [])
                        Button { onActions(session) } label: {
                            Image(systemName: "ellipsis")
                                .font(.system(size: 13, weight: .medium)).foregroundStyle(BitFunTheme.muted)
                                .frame(width: 36, height: 40)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(MobileLocalization.text("会话操作"))
                        .anchorPreference(
                            key: SidebarSessionActionsAnchorKey.self,
                            value: .bounds,
                            transform: { [session.id: $0] }
                        )
                    }
                    .padding(.leading, 32)
                    .padding(.trailing, 4)
                    .frame(minHeight: metadata(session) == nil ? 44 : 56)
                    .background(isSelected(session) ? BitFunTheme.soft : BitFunTheme.transparent)
                    .clipShape(RoundedRectangle(cornerRadius: 9))
                }
                if workspace.sessions.count > sessionLimit {
                    Button(action: { onShowMore?() }) {
                        Text(
                            MobileLocalization.format(
                                "还有 %lld 个会话",
                                language: MobileLocalization.restoredLanguage(),
                                Int64(workspace.sessions.count - sessionLimit)
                            )
                        )
                        .font(.system(size: 13))
                        .foregroundStyle(BitFunTheme.muted)
                        .padding(.leading, 42)
                        .frame(height: 36, alignment: .leading)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}
