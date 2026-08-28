import AVFoundation
import BitFunMobileCore
import SwiftUI

struct PairingSheet: View {
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

struct QRCodeScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> QRScannerController {
        let controller = QRScannerController()
        controller.onCode = onCode
        return controller
    }

    func updateUIViewController(_ uiViewController: QRScannerController, context: Context) {}
}

final class QRScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
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
                    if model.accountBusy { ProgressView().tint(.white) }
                    Text(model.localized(model.accountBusy ? "正在重试" : "重试加载设备"))
                }
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(.white)
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
