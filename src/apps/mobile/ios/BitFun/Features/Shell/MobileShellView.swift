import AVFoundation
import SwiftUI

struct MobileShellView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ZStack(alignment: .leading) {
            VStack(spacing: 0) {
                ConversationHeader(model: model)
                if model.connectionPhase != .connected {
                    ConnectionStatusBar(phase: model.connectionPhase, detail: model.coreErrorMessage)
                }
                if model.surface == .remote && !model.remoteConnected {
                    RemoteHomeView(model: model)
                    ComposerBar(model: model)
                } else if model.surface == .remote && !model.remoteSessionSelected {
                    RemoteConnectedHomeView()
                    ComposerBar(model: model)
                } else if model.surface == .local && !model.localSessionSelected {
                    LocalHomeView(model: model)
                    ComposerBar(model: model)
                } else {
                    ChatTimelineView(model: model)
                    ComposerBar(model: model)
                }
            }
            .background(BitFunTheme.page)
            .ignoresSafeArea(.keyboard, edges: .bottom)

            if model.drawerOpen {
                Color.black.opacity(0.24)
                    .ignoresSafeArea()
                    .onTapGesture { model.drawerOpen = false }
                SidebarView(model: model)
                    .transition(.move(edge: .leading).combined(with: .opacity))
                    .shadow(color: .black.opacity(0.18), radius: 26, x: 10, y: 0)
            }
        }
        .animation(.easeOut(duration: 0.24), value: model.drawerOpen)
        .sheet(isPresented: $model.settingsOpen) { SettingsView(model: model) }
        .sheet(isPresented: $model.pairingSheetOpen) { PairingSheet(model: model) }
    }
}

private struct PairingSheet: View {
    @ObservedObject var model: MobileAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var pairingURL = ""
    @State private var scannerOpen = false
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("连接桌面端")
                    .font(.system(size: 24, weight: .bold))
                    .foregroundStyle(BitFunTheme.ink)
                Spacer()
                Button { dismiss() } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 36, height: 36)
                        .background(BitFunTheme.soft)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
            }
            .padding(.bottom, 24)

            Text("扫描桌面端显示的二维码，或粘贴连接链接。")
                .font(.system(size: 15))
                .foregroundStyle(BitFunTheme.muted)
                .lineSpacing(4)
                .padding(.bottom, 18)

            Button {
                scannerOpen = true
            } label: {
                Label("扫描二维码", systemImage: "qrcode.viewfinder")
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(BitFunTheme.accent)
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .background(BitFunTheme.soft)
                    .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            .padding(.bottom, 12)

            HStack(spacing: 8) {
                TextField("粘贴桌面端连接链接", text: $pairingURL, axis: .vertical)
                    .font(.system(size: 14))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(2...4)
                    .focused($focused)
                Button {
                    pairingURL = UIPasteboard.general.string ?? ""
                    focused = false
                } label: {
                    Image(systemName: "doc.on.clipboard")
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(BitFunTheme.accent)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("从剪贴板粘贴")
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(BitFunTheme.card)
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(BitFunTheme.line, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 12))

            if let error = model.pairingError {
                Text(error)
                    .font(.system(size: 13))
                    .foregroundStyle(BitFunTheme.red)
                    .lineSpacing(3)
                    .padding(.top, 12)
            }

            Button {
                model.submitPairing(url: pairingURL)
                focused = false
            } label: {
                HStack(spacing: 8) {
                    if model.pairingBusy { ProgressView().tint(.white) }
                    Text(model.pairingBusy ? "正在连接" : "连接")
                        .font(.system(size: 16, weight: .semibold))
                }
                .foregroundStyle(.white)
                .frame(maxWidth: .infinity, minHeight: 48)
                .background(pairingURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? BitFunTheme.muted : BitFunTheme.accent)
                .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            .disabled(model.pairingBusy || pairingURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .padding(.top, 22)

            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.top, 22)
        .background(BitFunTheme.page)
        .presentationDetents([.medium])
        .presentationDragIndicator(.visible)
        .fullScreenCover(isPresented: $scannerOpen) {
            QRCodeScannerView { code in
                pairingURL = code
                scannerOpen = false
            }
            .ignoresSafeArea()
        }
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
                    Button {
                        model.draft = title
                        model.send()
                    } label: {
                        HStack(spacing: 20) {
                            Text(icon)
                                .font(.system(size: 29, weight: .regular))
                                .foregroundStyle(BitFunTheme.muted)
                                .frame(width: 32)
                                .fixedSize()
                            Text(title)
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
            Text("连接桌面端")
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(BitFunTheme.ink)
            Text("扫描桌面端显示的二维码，开始远程处理任务。")
                .font(.system(size: 13))
                .foregroundStyle(BitFunTheme.muted)
                .multilineTextAlignment(.center)
                .lineSpacing(7)
                .padding(.horizontal, 20)
            Button("连接") { model.connectRemote() }
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(.white)
                .frame(width: 136, height: 44)
                .background(BitFunTheme.accent)
                .clipShape(Capsule())
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 20)
        .padding(.bottom, 48)
        .background(BitFunTheme.page)
    }
}

private struct RemoteConnectedHomeView: View {
    var body: some View {
        VStack {
            Spacer()
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(BitFunTheme.page)
    }
}

private struct ConnectionStatusBar: View {
    let phase: ConnectionPhase
    var detail: String?
    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(phase == .reconnecting ? BitFunTheme.muted : BitFunTheme.red).frame(width: 8, height: 8)
            Text(phase == .reconnecting ? "正在恢复连接" : "连接不可用")
                .font(.system(size: 13, weight: .medium))
            Text(detail ?? (phase == .reconnecting ? "正在重新连接桌面端" : "请重新连接"))
                .font(.system(size: 12))
                .foregroundStyle(BitFunTheme.muted)
            Spacer()
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

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Spacer()
                Button { dismiss() } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 17, weight: .regular))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(width: 48, height: 48)
                        .background(BitFunTheme.card)
                        .clipShape(Circle())
                        .shadow(color: .black.opacity(0.07), radius: 10, y: 4)
                }
                .buttonStyle(.plain)
            }
            .padding(.top, 8)

            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    Text("设置")
                        .font(.system(size: 32, weight: .bold))
                        .foregroundStyle(BitFunTheme.ink)
                        .padding(.top, 72)
                        .padding(.bottom, 34)

                    SettingsCard {
                        SettingsValueRow(icon: "person", title: "个人资料", value: "6a7c7282-b185-4ae7-…")
                    }
                    SettingsGroup(title: "语言") {
                        SettingsValueRow(icon: "textformat", title: "语言", value: "简体中文")
                    }
                    SettingsGroup(title: "普通对话") {
                        SettingsValueRow(icon: "square.grid.2x2", title: "模型", value: "deepseek-v4-pro")
                    }
                    SettingsGroup(title: "关于") {
                        VStack(spacing: 0) {
                            SettingsValueRow(icon: nil, title: "产品", value: "BitFun HarmonyOS")
                            Divider().overlay(BitFunTheme.line).padding(.horizontal, 20)
                            SettingsValueRow(icon: nil, title: "版本", value: "1.0.0")
                        }
                    }
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 28)
            }
        }
        .background(BitFunTheme.page)
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.hidden)
    }
}

private struct SettingsGroup<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.system(size: 20, weight: .bold))
                .foregroundStyle(BitFunTheme.muted)
                .padding(.leading, 18)
                .padding(.top, 34)
            SettingsCard(content: content)
        }
    }
}

private struct SettingsCard<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(spacing: 0, content: content)
            .background(BitFunTheme.card)
            .clipShape(RoundedRectangle(cornerRadius: 14))
    }
}

private struct SettingsValueRow: View {
    let icon: String?
    let title: String
    let value: String

    var body: some View {
        HStack(spacing: 14) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 23, weight: .regular))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(width: 42, height: 42)
            }
            Text(title)
                .font(.system(size: 17, weight: .medium))
                .foregroundStyle(BitFunTheme.ink)
            Spacer(minLength: 12)
            Text(value)
                .font(.system(size: 16))
                .foregroundStyle(BitFunTheme.muted)
                .lineLimit(1)
            Image(systemName: "chevron.right")
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(BitFunTheme.muted)
        }
        .padding(.horizontal, 20)
        .frame(minHeight: 76)
    }
}
