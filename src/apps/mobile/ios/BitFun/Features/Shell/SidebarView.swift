import SwiftUI

struct SidebarView: View {
    @ObservedObject var model: MobileAppModel
    @State private var search = ""
    @State private var searchVisible = false

    private var devices: [SidebarDevice] {
        if let accountDeviceName = model.accountDeviceName, !accountDeviceName.isEmpty {
            return [SidebarDevice(name: accountDeviceName, online: true)]
        }
        return [
            SidebarDevice(name: "Mac-userdeMacBook-Pro.local", online: true),
            SidebarDevice(name: "DESKTOP-KM3L4UI", online: true),
        ]
    }

    private let workspaces = [
        SidebarWorkspace(name: "arkanaly...", sessions: ["本项目是啥", "Remote Code ..."]),
        SidebarWorkspace(name: "BitFun", sessions: ["你在哪个分支", "你去拉一下Deep...", "你去看看issue里..."])
    ]

    private var recentSessions: [ChatSession] {
        let source = model.sessions
        guard !search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return source }
        return source.filter { $0.title.localizedCaseInsensitiveContains(search) }
    }

    var body: some View {
        GeometryReader { proxy in
            VStack(alignment: .leading, spacing: 0) {
                authenticatedHeader
                if searchVisible {
                    searchField
                }
                ScrollView(showsIndicators: false) {
                    VStack(alignment: .leading, spacing: 0) {
                        recentSection
                        workspaceSection
                    }
                    .padding(.bottom, 84)
                }
                footer
            }
            .padding(.horizontal, 20)
            .padding(.top, 4)
            .padding(.bottom, 16)
            .frame(width: min(320, proxy.size.width * 0.68), height: proxy.size.height, alignment: .topLeading)
            .background(BitFunTheme.page)
        }
    }

    private var authenticatedHeader: some View {
        HStack(spacing: 6) {
            Text("BitFun")
                .font(.system(size: 20, weight: .bold))
                .foregroundStyle(BitFunTheme.ink)
            Spacer(minLength: 0)
            Button {
                withAnimation(.easeOut(duration: 0.18)) { searchVisible.toggle() }
                if !searchVisible { search = "" }
            } label: {
                ReferenceImage(assetName: "SidebarSearchGlyph", width: 22, height: 22)
                    .frame(width: 38, height: 38)
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.08), radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("搜索")
        }
        .frame(height: 50)
    }

    private var searchField: some View {
        TextField("搜索对话", text: $search)
            .font(.system(size: 14))
            .foregroundStyle(BitFunTheme.ink)
            .padding(.horizontal, 14)
            .frame(height: 42)
            .background(BitFunTheme.soft)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .padding(.top, 12)
    }

    private var recentSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("最近对话")
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.top, 16)
                .padding(.bottom, 6)
            ForEach(recentSessions) { session in
                SidebarRecentRow(session: session, selected: session.id == model.selectedSessionID) {
                    model.surface = .local
                    model.select(session)
                }
            }
            if recentSessions.count > 6 {
                HStack(spacing: 8) {
                    Text("···")
                    Text("还有 \(recentSessions.count - 6) 个会话")
                }
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .frame(height: 40, alignment: .leading)
                .padding(.leading, 12)
            }
        }
    }

    private var workspaceSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("设备")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(BitFunTheme.muted)
                Spacer()
                Button { model.connectRemote() } label: {
                    ReferenceImage(assetName: "SidebarPlusGlyph", width: 17, height: 20)
                        .frame(width: 32, height: 32)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("添加连接")
            }
            .frame(height: 38)
            .padding(.top, 18)

            ForEach(devices) { device in
                Button { model.connectRemote() } label: {
                    HStack(spacing: 10) {
                        ReferenceImage(assetName: "SidebarDeviceGlyph", width: 22, height: 18)
                        Text(device.name)
                            .font(.system(size: 15))
                            .foregroundStyle(BitFunTheme.ink)
                            .lineLimit(1)
                        Spacer(minLength: 0)
                        ReferenceImage(
                            assetName: device.online ? "SidebarChevronGlyph" : "SidebarDownGlyph",
                            width: 14,
                            height: 14
                        )
                    }
                    .padding(.horizontal, 10)
                    .frame(height: 46)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }

            ForEach(workspaces) { workspace in
                SidebarWorkspaceRow(workspace: workspace)
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 0) {
            Button { model.surface = .local; model.drawerOpen = false } label: {
                HStack(spacing: 9) {
                    ReferenceImage(assetName: "SidebarEditGlyph", width: 24, height: 24)
                    Text("聊天")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                }
                .frame(width: 116, height: 46)
                .background(BitFunTheme.card)
                .overlay(RoundedRectangle(cornerRadius: 23).stroke(BitFunTheme.line, lineWidth: 1))
                .clipShape(Capsule())
                .shadow(color: .black.opacity(0.08), radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            Spacer(minLength: 0)
            Button { model.settingsOpen = true; model.drawerOpen = false } label: {
                ReferenceImage(assetName: "SidebarSettingsGlyph", width: 24, height: 24)
                    .frame(width: 46, height: 46)
                    .background(BitFunTheme.card)
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.08), radius: 10, y: 4)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("设置")
        }
        .frame(height: 56)
    }
}

private struct SidebarRecentRow: View {
    let session: ChatSession
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Text(session.title)
                    .font(.system(size: 15))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                HStack(spacing: 3) {
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                    Circle().fill(BitFunTheme.muted).frame(width: 3.5, height: 3.5)
                }
                .frame(width: 34, height: 40)
                .opacity(0.62)
            }
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(selected ? BitFunTheme.soft : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
    }
}

private struct SidebarWorkspaceRow: View {
    let workspace: SidebarWorkspace

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                ReferenceImage(assetName: "SidebarFolderGlyph", width: 24, height: 20)
                Text(workspace.name)
                    .font(.system(size: 15))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                ReferenceImage(assetName: "SidebarEditGlyph", width: 22, height: 22)
                    .opacity(0.62)
                ReferenceImage(assetName: "SidebarDownGlyph", width: 14, height: 14)
                    .opacity(0.62)
            }
            .padding(.horizontal, 10)
            .frame(height: 46)
            ForEach(Array(workspace.sessions.enumerated()), id: \.offset) { _, title in
                HStack(spacing: 10) {
                    Image(systemName: "doc")
                        .font(.system(size: 21, weight: .regular))
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 22)
                    Text(title)
                        .font(.system(size: 15))
                        .foregroundStyle(BitFunTheme.ink)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                }
                .padding(.leading, 32)
                .frame(height: 44)
            }
        }
    }
}

private struct SidebarDevice: Identifiable {
    let id = UUID()
    let name: String
    let online: Bool
}

private struct SidebarWorkspace: Identifiable {
    let id = UUID()
    let name: String
    let sessions: [String]
}
