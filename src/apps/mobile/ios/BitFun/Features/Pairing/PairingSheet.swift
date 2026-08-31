import BitFunMobileCore
import SwiftUI

struct PairingSheet: View {
    private enum Step { case intro, scan }

    @ObservedObject var model: MobileAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var step: Step = .intro
    @State private var pairingURL = MobileLaunchConfiguration.pairingAccountPreview
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
            } else if MobileLaunchConfiguration.pairingManualPreview ||
                MobileLaunchConfiguration.pairingAccountPreview {
                step = .scan
                manualOpen = true
                focused = !MobileLaunchConfiguration.pairingAccountPreview
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
                    Text(error).font(.system(size: 13)).foregroundStyle(BitFunTheme.statusDanger)
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
            BitFunTheme.scrim
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
                    Text(error).font(.system(size: 13)).foregroundStyle(BitFunTheme.statusDanger)
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
                .foregroundStyle(primary ? BitFunTheme.contentOnAction : BitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 58)
                .background(primary ? BitFunTheme.accent : BitFunTheme.soft)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}
