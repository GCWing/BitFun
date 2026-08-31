import BitFunMobileCore
import SwiftUI

struct AccountSettingsView: View {
    @ObservedObject var model: MobileAppModel
    var onClose: (() -> Void)? = nil
    @State private var relayURL = AccountDefaults.shared.CLOUD_RELAY_URL
    @State private var username = ""
    @State private var password = ""

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
                        .foregroundStyle(BitFunTheme.statusDanger)
                        .padding(.top, 12)
                }

                Button {
                    model.loginAccount(relayURL: relayURL, username: username, password: password)
                    password = ""
                } label: {
                    HStack(spacing: 8) {
                        if model.accountBusy { ProgressView().tint(BitFunTheme.contentOnAction) }
                        Text(model.localized(model.accountBusy ? "正在登录" : "登录"))
                    }
                    .font(.system(size: 17, weight: .bold))
                    .foregroundStyle(BitFunTheme.contentOnAction)
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

    private var deviceListRetryPage: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button { close() } label: {
                Image(systemName: "chevron.left")
                    .font(.system(size: 19, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("返回"))

            Spacer()
            Image(systemName: "desktopcomputer.trianglebadge.exclamationmark")
                .font(.system(size: 48, weight: .medium))
                .foregroundStyle(BitFunTheme.muted)
                .frame(maxWidth: .infinity)
            Text(model.localized("无法加载设备列表"))
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(BitFunTheme.ink)
                .frame(maxWidth: .infinity)
                .padding(.top, 20)
            Text(model.coreErrorMessage ?? model.localized("登录已完成，但设备列表加载失败。请重试。"))
                .font(.system(size: 15))
                .foregroundStyle(BitFunTheme.muted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .padding(.top, 10)

            Button { model.retryAccountFailure() } label: {
                HStack(spacing: 8) {
                    if model.accountBusy { ProgressView().tint(BitFunTheme.contentOnAction) }
                    Text(model.localized(model.accountBusy ? "正在重试" : "重试加载设备"))
                }
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(BitFunTheme.contentOnAction)
                .frame(maxWidth: .infinity, minHeight: 56)
                .background(BitFunTheme.accent)
                .clipShape(RoundedRectangle(cornerRadius: 18))
            }
            .buttonStyle(.plain)
            .disabled(model.accountBusy)
            .padding(.top, 30)

            Button(model.localized("使用其他账号重新登录")) {
                model.logoutAccount()
            }
            .font(.system(size: 15, weight: .medium))
            .foregroundStyle(BitFunTheme.ink)
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
                                .foregroundStyle(BitFunTheme.statusSuccess)
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
                            .foregroundStyle(BitFunTheme.statusDanger)
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

struct SettingsDeviceRow: View {
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
                    .foregroundStyle(device.online ? BitFunTheme.statusSuccess : BitFunTheme.muted)
            }
            Spacer(minLength: 12)
            if device.selected {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 18))
                    .foregroundStyle(BitFunTheme.statusSuccess)
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
