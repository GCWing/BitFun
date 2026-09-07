import OpenBitFunMobileCore
import SwiftUI

struct AccountSettingsView: View {
    @ObservedObject var model: MobileAppModel
    var onClose: (() -> Void)? = nil
    @State private var relayURL = AccountDefaults.shared.CLOUD_RELAY_URL
    @State private var username = ""
    @State private var password = ""
    @State private var advancedOpen = false

    var body: some View {
        Group {
            if model.accountFailureStage == "DEVICE_LIST", model.accountFailureCanRetry {
                deviceListRetryPage
            } else if model.accountUser == nil {
                loginPage
            } else {
                profilePage
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(OpenBitFunTheme.page)
    }

    private var loginPage: some View {
        VStack(spacing: 0) {
            OpenBitFunModalHeader(title: "", onClose: close)
                .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)

            ScrollView(showsIndicators: false) {
                VStack(spacing: 0) {
                    Text(model.localized("登录 OpenBitFun 账号"))
                        .font(MobileDesignTypography.displayMedium.font)
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity)
                    Text(model.localized("登录后可查看并连接账号下的桌面设备。"))
                        .font(MobileDesignTypography.bodyMedium.font)
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity)
                        .padding(.top, 8)
                        .padding(.bottom, 24)

                    VStack(spacing: 0) {
                        accountCredentialRow(
                            icon: "person",
                            placeholder: model.localized("用户名"),
                            text: $username,
                            secure: false
                        )
                        Divider()
                            .overlay(OpenBitFunTheme.line)
                            .padding(.leading, 54)
                            .padding(.trailing, 16)
                        accountCredentialRow(
                            icon: "lock",
                            placeholder: model.localized("密码"),
                            text: $password,
                            secure: true
                        )
                    }
                    .background(OpenBitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 24))
                    .overlay(RoundedRectangle(cornerRadius: 24).stroke(OpenBitFunTheme.line, lineWidth: 1))
                    .shadow(color: MobileDesignColors.shadowFaint, radius: 16, y: 5)

                    VStack(spacing: 0) {
                        Button { advancedOpen.toggle() } label: {
                            HStack(spacing: 14) {
                                Image(systemName: "gearshape")
                                    .font(.system(size: 21, weight: .regular))
                                    .foregroundStyle(OpenBitFunTheme.muted)
                                    .frame(width: 24, height: 24)
                                Text(model.localized("高级选项"))
                                    .font(MobileDesignTypography.titleSmall.font)
                                    .foregroundStyle(OpenBitFunTheme.ink)
                                Spacer(minLength: 8)
                                Image(systemName: advancedOpen ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 13, weight: .medium))
                                    .foregroundStyle(OpenBitFunTheme.muted)
                            }
                            .padding(.horizontal, 18)
                            .frame(height: 58)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)

                        if advancedOpen {
                            Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 18)
                            VStack(alignment: .leading, spacing: 8) {
                                Text(model.localized("登录服务器"))
                                    .font(MobileDesignTypography.labelSmall.font)
                                    .foregroundStyle(OpenBitFunTheme.muted)
                                accountRelayField
                            }
                            .padding(.horizontal, 18)
                            .padding(.top, 14)
                            .padding(.bottom, 18)
                        }
                    }
                    .background(OpenBitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: 20))
                    .overlay(RoundedRectangle(cornerRadius: 20).stroke(OpenBitFunTheme.line, lineWidth: 1))
                    .padding(.top, 14)

                    if let error = model.coreErrorMessage, !error.isEmpty {
                        Text(error)
                            .font(MobileDesignTypography.bodySmall.font)
                            .foregroundStyle(OpenBitFunTheme.statusDanger)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 4)
                            .padding(.top, 12)
                    }
                }
                .frame(maxWidth: 520)
                .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                .padding(.bottom, 34)
                .frame(maxWidth: .infinity)
            }

            Button {
                model.loginAccount(relayURL: relayURL, username: username, password: password)
                password = ""
            } label: {
                HStack(spacing: 8) {
                    if model.accountBusy { ProgressView().tint(OpenBitFunTheme.contentOnAction) }
                    Text(model.localized(model.accountBusy ? "正在登录" : "登录"))
                }
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.contentOnAction)
                .frame(maxWidth: 520, minHeight: MobileDesignGeometry.sheetActionHeight)
                .background(canLogin ? OpenBitFunTheme.accent : OpenBitFunTheme.muted.opacity(0.35))
                .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            .disabled(!canLogin)
            .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
            .padding(.top, 10)
            .padding(.bottom, 24)
        }
    }

    private var accountRelayField: some View {
        TextField(model.localized("Relay 地址"), text: $relayURL)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.URL)
            .font(MobileDesignTypography.bodyMedium.font)
            .foregroundStyle(OpenBitFunTheme.ink)
            .padding(.horizontal, 14)
            .frame(height: 48)
            .background(OpenBitFunTheme.soft)
            .clipShape(RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(OpenBitFunTheme.line, lineWidth: 1))
    }

    @ViewBuilder
    private func accountCredentialRow(
        icon: String,
        placeholder: String,
        text: Binding<String>,
        secure: Bool
    ) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(width: 24, height: 24)
            Group {
                if secure {
                    SecureField(placeholder, text: text)
                        .textContentType(.password)
                } else {
                    TextField(placeholder, text: text)
                        .textContentType(.username)
                }
            }
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .font(MobileDesignTypography.bodyLarge.font)
            .foregroundStyle(OpenBitFunTheme.ink)
        }
        .padding(.leading, 18)
        .padding(.trailing, 12)
        .frame(height: 60)
    }

    private var deviceListRetryPage: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button { close() } label: {
                Image(systemName: "chevron.left")
                    .font(.system(size: 19, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("返回"))

            Spacer()
            Image(systemName: "desktopcomputer.trianglebadge.exclamationmark")
                .font(.system(size: 48, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(maxWidth: .infinity)
            Text(model.localized("无法加载设备列表"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.ink)
                .frame(maxWidth: .infinity)
                .padding(.top, 20)
            Text(model.coreErrorMessage ?? model.localized("登录已完成，但设备列表加载失败。请重试。"))
                .font(.system(size: 15))
                .foregroundStyle(OpenBitFunTheme.muted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.top, 10)

            Button { model.retryAccountFailure() } label: {
                HStack(spacing: 8) {
                    if model.accountBusy { ProgressView().tint(OpenBitFunTheme.contentOnAction) }
                    Text(model.localized(model.accountBusy ? "正在重试" : "重试加载设备"))
                }
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.contentOnAction)
                .frame(maxWidth: .infinity, minHeight: 56)
                .background(OpenBitFunTheme.accent)
                .clipShape(RoundedRectangle(cornerRadius: 18))
            }
            .buttonStyle(.plain)
            .disabled(model.accountBusy)
            .padding(.top, 30)

            Button(model.localized("使用其他账号重新登录")) {
                model.logoutAccount()
            }
            .font(.system(size: 15, weight: .medium))
            .foregroundStyle(OpenBitFunTheme.ink)
            .frame(maxWidth: .infinity, minHeight: 48)
            .buttonStyle(.plain)
            .disabled(model.accountBusy)
            .padding(.top, 8)
            Spacer()
        }
        .padding(.horizontal, 28)
        .padding(.top, 22)
        .padding(.bottom, 44)
    }

    private var profilePage: some View {
        VStack(alignment: .leading, spacing: 0) {
            OpenBitFunModalHeader(title: "个人资料", onClose: close)
                .padding(.horizontal, MobileDesignGeometry.sheetHorizontalPadding)
                .padding(.top, 8)
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(spacing: 10) {
                        ZStack {
                            Circle().fill(OpenBitFunTheme.soft)
                            Image(systemName: "person.fill")
                                .font(.system(size: 34, weight: .medium))
                                .foregroundStyle(OpenBitFunTheme.ink)
                        }
                        .frame(width: 70, height: 70)
                        Text(model.accountUser ?? "")
                            .font(.system(size: 22, weight: .bold))
                            .foregroundStyle(OpenBitFunTheme.ink)
                            .lineLimit(1)
                        Text(profileIdentifier)
                            .font(.system(size: 14))
                            .foregroundStyle(OpenBitFunTheme.muted)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                    .background(OpenBitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsProminentCardRadius))
                    .padding(.bottom, 24)

                    VStack(alignment: .leading, spacing: 10) {
                        HStack {
                            Text(model.localized("OpenBitFun 账号"))
                                .font(.system(size: 17, weight: .bold))
                                .foregroundStyle(OpenBitFunTheme.ink)
                            Spacer()
                            Text(model.localized("已登录"))
                                .font(.system(size: 14))
                                .foregroundStyle(OpenBitFunTheme.statusSuccess)
                        }
                        Text(model.localizedFormat("当前以 %@ 登录。", model.accountUser ?? ""))
                            .font(.system(size: 14))
                            .foregroundStyle(OpenBitFunTheme.muted)
                            .lineSpacing(3)
                    }
                    .padding(.horizontal, 18)
                    .padding(.vertical, 16)
                    .background(OpenBitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCardRadius))
                    .padding(.bottom, 24)

                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(model.localized("设备管理"))
                                .font(MobileDesignTypography.titleSmall.font)
                                .foregroundStyle(OpenBitFunTheme.muted)
                            Spacer()
                            Button(action: model.refreshRemoteDevices) {
                                Group {
                                    if model.accountRefreshing {
                                        ProgressView().controlSize(.small)
                                    } else {
                                        Image(systemName: "arrow.clockwise")
                                            .font(.system(size: 18, weight: .regular))
                                    }
                                }
                                .foregroundStyle(model.accountRefreshing ? OpenBitFunTheme.muted : OpenBitFunTheme.ink)
                                .frame(width: MobileDesignGeometry.controlTouchSize, height: MobileDesignGeometry.controlTouchSize)
                            }
                            .buttonStyle(.plain)
                            .disabled(model.accountRefreshing)
                            .accessibilityLabel(model.localized("刷新"))
                        }
                        VStack(spacing: 4) {
                            ForEach(model.accountDevices) { device in
                                Button { model.selectRemoteDevice(device) } label: {
                                    SettingsDeviceRow(
                                        device: device,
                                        connected: device.selected && model.connectionPhase == .connected
                                    )
                                }
                                .buttonStyle(.plain)
                                .disabled(!device.online && !device.selected)
                            }
                            if model.accountDevices.isEmpty {
                                Text(model.localized("暂无可连接的桌面设备"))
                                    .font(.system(size: 13))
                                    .foregroundStyle(OpenBitFunTheme.muted)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(.vertical, 12)
                            }
                        }
                        .padding(8)
                        .background(OpenBitFunTheme.card)
                        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCardRadius))
                    }
                    .padding(.bottom, 24)

                    Text(model.localized("个人资料详情"))
                        .font(MobileDesignTypography.titleSmall.font.weight(.bold))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .padding(.leading, 18)
                        .padding(.bottom, 8)

                    VStack(spacing: 0) {
                        profileDetailRow(label: model.localized("用户 ID"), value: profileIdentifier)
                        Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 18)
                        profileDetailRow(
                            label: model.localized("设备 ID"),
                            value: model.localDeviceID.isEmpty ? "-" : model.localDeviceID
                        )
                    }
                    .background(OpenBitFunTheme.card)
                    .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCardRadius))

                    Button(role: .destructive) {
                        model.logoutAccount()
                    } label: {
                        HStack(spacing: 14) {
                            Image(systemName: "rectangle.portrait.and.arrow.right")
                                .font(.system(size: 22, weight: .regular))
                                .frame(width: 24, height: 24)
                            Text(model.localized("退出账号"))
                                .font(MobileDesignTypography.titleMedium.font)
                            Spacer(minLength: 0)
                        }
                        .foregroundStyle(OpenBitFunTheme.statusDanger)
                        .padding(.horizontal, 20)
                        .frame(maxWidth: .infinity, minHeight: 62)
                        .background(OpenBitFunTheme.card)
                        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCardRadius))
                    }
                    .buttonStyle(.plain)
                    .padding(.top, 32)
                    .padding(.bottom, 8)
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
        HStack(spacing: 16) {
            Text(label)
                .font(MobileDesignTypography.bodyLarge.font)
                .foregroundStyle(OpenBitFunTheme.ink)
            Spacer(minLength: 8)
            Text(value)
                .font(MobileDesignTypography.bodyMedium.font)
                .foregroundStyle(OpenBitFunTheme.muted)
                .lineLimit(1)
                .truncationMode(.middle)
                .multilineTextAlignment(.trailing)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
        .frame(minHeight: 58)
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

}

struct SettingsDeviceRow: View {
    let device: MobileAccountDevice
    let connected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "desktopcomputer")
                .font(.system(size: 20, weight: .regular))
                .foregroundStyle(connected ? OpenBitFunTheme.ink : OpenBitFunTheme.muted)
                .frame(width: 28, height: 28)
            VStack(alignment: .leading, spacing: 3) {
                Text(device.name)
                    .font(MobileDesignTypography.titleSmall.font)
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .lineLimit(1)
                Text(deviceStatus)
                    .font(MobileDesignTypography.bodySmall.font)
                    .foregroundStyle(device.online ? OpenBitFunTheme.statusSuccess : OpenBitFunTheme.muted)
            }
            Spacer(minLength: 0)
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
        .padding(.vertical, 10)
        .frame(minHeight: 62)
        .background(connected ? OpenBitFunTheme.soft : OpenBitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.settingsCompactCardRadius))
        .opacity(device.online ? 1 : 0.48)
    }

    private var deviceStatus: String {
        let presence = MobileLocalization.text(device.online ? "在线" : "离线")
        return connected ? "\(MobileLocalization.text("当前控制")) · \(presence)" : presence
    }
}
