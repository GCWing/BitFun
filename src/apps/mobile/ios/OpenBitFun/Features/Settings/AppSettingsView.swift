import SwiftUI

struct SettingsView: View {
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
            VStack(spacing: 0) {
                OpenBitFunModalHeader(title: "设置", onClose: { dismiss() })
                    .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                Divider().overlay(OpenBitFunTheme.line)

                ScrollView(showsIndicators: false) {
                    VStack(alignment: .leading, spacing: 0) {
                        SettingsGroup(title: "账号") {
                            Button { accountOpen = true } label: {
                                SettingsProfileRow(
                                    subtitle: model.accountUser ?? model.localized("未登录"),
                                    authenticated: model.accountUser != nil
                                )
                            }
                            .buttonStyle(.plain)
                        }

                        if showsCurrentConnection {
                            currentConnectionSection
                        }

                        SettingsGroup(title: "通用") {
                            Button { model.languagePickerOpen = true } label: {
                                SettingsValueRow(
                                    icon: "textformat",
                                    title: "语言",
                                    value: model.appLanguage.nativeName,
                                    showsChevron: true
                                )
                            }
                            .buttonStyle(.plain)
                        }
                        SettingsGroup(title: "通知") {
                            Button {
                                Task { await TaskCompletionNotifier.manageNotifications() }
                            } label: {
                                SettingsValueRow(
                                    icon: "bell",
                                    title: "任务完成通知",
                                    value: "",
                                    showsChevron: true
                                )
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("settings.notifications")
                        }
                        accountDevicesSection
                        SettingsGroup(title: "关于") {
                            VStack(spacing: 0) {
                                SettingsValueRow(
                                    icon: nil,
                                    title: "产品",
                                    value: "OpenBitFun iOS版"
                                )
                                Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 26)
                                SettingsValueRow(icon: nil, title: "版本", value: appVersion)
                            }
                        }
                        if model.accountUser != nil {
                            Button(role: .destructive) {
                                model.logoutAccount()
                            } label: {
                                HStack(spacing: 14) {
                                    Image(systemName: "rectangle.portrait.and.arrow.right")
                                        .font(.system(size: 20, weight: .regular))
                                        .frame(width: 24, height: 24)
                                    Text(model.localized("退出账号"))
                                        .font(MobileDesignTypography.bodyLarge.font.weight(.medium))
                                    Spacer(minLength: 0)
                                }
                                .foregroundStyle(OpenBitFunTheme.statusDanger)
                                .padding(.horizontal, 20)
                                .frame(minHeight: 62)
                            }
                            .buttonStyle(.plain)
                            .overlay(alignment: .top) {
                                Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 8)
                            }
                        }
                    }
                    .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                    .padding(.top, 22)
                    .padding(.bottom, 34)
                }
            }

            if model.languagePickerOpen {
                LanguagePickerSheet(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            } else if accountOpen {
                AccountSettingsView(model: model, onClose: { accountOpen = false })
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .background(OpenBitFunTheme.page)
        .animation(.easeInOut(duration: 0.2), value: model.languagePickerOpen)
        .animation(.easeInOut(duration: 0.2), value: accountOpen)
    }

    private var showsCurrentConnection: Bool {
        model.remoteConnected || model.accountDeviceName != nil
    }

    private var currentConnectionSection: some View {
        SettingsGroup(title: "当前远程控制") {
            VStack(spacing: 0) {
                HStack(spacing: 14) {
                    Image(systemName: "desktopcomputer")
                        .font(.system(size: 20, weight: .regular))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .frame(width: 28, height: 28)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(
                            model.accountDeviceName

                                ?? model.localized("尚未连接桌面端")
                        )
                        .font(MobileDesignTypography.bodyLarge.font.weight(.medium))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .lineLimit(1)
                        Text(connectionDetail)
                            .font(MobileDesignTypography.bodySmall.font)
                            .foregroundStyle(OpenBitFunTheme.muted)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 18)
                .frame(minHeight: 68)

                Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 18)

                HStack(spacing: 8) {
                    Text(model.localized(model.accountDeviceName == nil ? "扫码连接" : "账号设备"))
                        .font(MobileDesignTypography.bodySmall.font)
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(OpenBitFunTheme.soft)
                        .clipShape(Capsule())
                    Spacer(minLength: 0)
                    if model.connectionPhase == .disconnected {
                        Button(model.localized("重新连接"), action: model.verifyRemoteConnection)
                            .font(MobileDesignTypography.bodyMedium.font.weight(.medium))
                            .foregroundStyle(OpenBitFunTheme.ink)
                            .buttonStyle(.plain)
                    }
                    Button(model.localized("断开"), action: model.disconnectRemote)
                        .font(MobileDesignTypography.bodyMedium.font.weight(.medium))
                        .foregroundStyle(OpenBitFunTheme.statusDanger)
                        .buttonStyle(.plain)
                }
                .padding(.horizontal, 18)
                .frame(minHeight: 54)
            }
        }
    }

    private var connectionDetail: String {
        switch model.connectionPhase {
        case .connected: model.localized("已连接")
        case .reconnecting: model.localized("正在重连")
        case .disconnected: model.localized("连接已断开")
        }
    }

    private var accountDevicesSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(model.localized("设备"))
                    .font(MobileDesignTypography.bodySmall.font.weight(.medium))
                    .foregroundStyle(OpenBitFunTheme.muted)
                Spacer(minLength: 0)
                Button(action: model.refreshRemoteDevices) {
                    Group {
                        if model.accountRefreshing {
                            ProgressView().controlSize(.small)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 17, weight: .regular))
                        }
                    }
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .frame(width: 44, height: 44)
                    .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .disabled(model.accountUser == nil || model.accountRefreshing)
                .opacity(model.accountUser == nil || model.accountRefreshing ? 0.55 : 1)
                .accessibilityLabel(model.localized("刷新"))
            }
            .padding(.leading, 8)
            .padding(.trailing, 4)
            .frame(minHeight: 48)

            VStack(spacing: 4) {
                if model.accountUser == nil {
                    emptyDeviceMessage("登录云账号后可查看该账号下的所有设备。")
                } else if model.accountRefreshing && model.accountDevices.isEmpty {
                    emptyDeviceMessage("正在加载设备…")
                } else if model.accountDevices.isEmpty {
                    emptyDeviceMessage("账号下还没有其他已注册设备。")
                } else {
                    ForEach(model.accountDevices) { device in
                        Button {
                            guard device.online,
                                  !(device.selected && model.connectionPhase == .connected) else { return }
                            model.selectRemoteDevice(device)
                        } label: {
                            EmbeddedSettingsDeviceRow(
                                device: device,
                                connected: device.selected && model.connectionPhase == .connected
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(8)
            .background(OpenBitFunTheme.card)
            .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCardRadius))
        }
        .padding(.bottom, 24)
    }

    private func emptyDeviceMessage(_ text: String) -> some View {
        Text(model.localized(text))
            .font(MobileDesignTypography.bodySmall.font)
            .foregroundStyle(OpenBitFunTheme.muted)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
    }
}

private struct LanguagePickerSheet: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            OpenBitFunSelectionHeader(title: "选择语言", onClose: { model.languagePickerOpen = false })
            Divider().overlay(OpenBitFunTheme.line)

            VStack(spacing: 0) {
                ForEach(MobileLanguage.allCases) { language in
                    Button {
                        model.setLanguage(language)
                        model.languagePickerOpen = false
                    } label: {
                        HStack {
                            Text(language.nativeName)
                                .font(.system(size: 16, weight: .medium))
                                .foregroundStyle(OpenBitFunTheme.ink)
                            Spacer()
                            if model.appLanguage == language {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 18, weight: .medium))
                                    .foregroundStyle(OpenBitFunTheme.ink)
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
        .background(OpenBitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.selectionTopRadius))
    }
}
private struct SettingsGroup<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(MobileLocalization.text(title))
                .font(MobileDesignTypography.bodySmall.font.weight(.medium))
                .foregroundStyle(OpenBitFunTheme.muted)
                .padding(.leading, 8)
                .padding(.bottom, 2)
            SettingsCard(content: content)
        }
        .padding(.bottom, 24)
    }
}

struct SettingsCard<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        OpenBitFunModalCard(
            radius: MobileDesignGeometry.settingsCardRadius,
            bordered: false,
            content: content
        )
    }
}

private struct SettingsProfileRow: View {
    let subtitle: String
    let authenticated: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: 24, weight: .regular))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 2) {
                Text(MobileLocalization.text(authenticated ? "当前账号" : "当前身份"))
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
                Text(MobileLocalization.text(subtitle))
                    .font(.system(size: 13))
                    .foregroundStyle(OpenBitFunTheme.muted)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            if authenticated {
                Text(MobileLocalization.text("已登录"))
                    .font(MobileDesignTypography.bodySmall.font.weight(.medium))
                    .foregroundStyle(OpenBitFunTheme.statusSuccess)
            } else {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.muted.opacity(0.72))
            }
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
                if icon == "textformat" {
                    Text("Aa")
                        .font(.system(size: 18, weight: .regular))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .frame(width: 23, height: 23)
                } else {
                    Image(systemName: icon)
                        .font(.system(size: 20, weight: .regular))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .frame(width: 23, height: 23)
                }
            }
            Text(MobileLocalization.text(title))
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.ink)
            Spacer(minLength: 12)
            Text(MobileLocalization.text(value))
                .font(.system(size: 15))
                .foregroundStyle(OpenBitFunTheme.muted)
                .lineLimit(1)
            if showsChevron {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.muted.opacity(0.72))
            }
        }
        .padding(.horizontal, 18)
        .frame(minHeight: 56)
    }
}

private struct EmbeddedSettingsDeviceRow: View {
    let device: MobileAccountDevice
    let connected: Bool

    private var status: String {
        if connected {
            return "\(MobileLocalization.text("当前控制")) · \(MobileLocalization.text("在线"))"
        }
        return MobileLocalization.text(device.online ? "在线" : "离线")
    }

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "desktopcomputer")
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(connected ? OpenBitFunTheme.ink : OpenBitFunTheme.muted)
                .frame(width: 28, height: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(device.name.isEmpty ? device.id : device.name)
                    .font(MobileDesignTypography.titleSmall.font.weight(.medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .lineLimit(1)
                Text(status)
                    .font(MobileDesignTypography.bodySmall.font)
                    .foregroundStyle(device.online ? OpenBitFunTheme.statusSuccess : OpenBitFunTheme.muted)
            }
            Spacer(minLength: 8)
            if device.online && !connected {
                Text(MobileLocalization.text("连接"))
                    .font(MobileDesignTypography.bodyMedium.font)
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(OpenBitFunTheme.soft)
                    .clipShape(Capsule())
            }
        }
        .padding(.horizontal, 10)
        .frame(minHeight: 62)
        .background(connected ? OpenBitFunTheme.soft : OpenBitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCompactCardRadius))
        .opacity(device.online ? 1 : 0.48)
    }
}
