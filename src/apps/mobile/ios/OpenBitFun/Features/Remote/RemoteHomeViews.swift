import OpenBitFunMobileCore
import OpenBitFunMobileCore
import SwiftUI

struct RemoteHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(spacing: 12) {
            Spacer()
            ZStack {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 42, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
            }
            .frame(width: 74, height: 74)
            .background(OpenBitFunTheme.card)
            .overlay(RoundedRectangle(cornerRadius: 24).stroke(OpenBitFunTheme.line, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 24))
            Text(model.localized("连接桌面端"))
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.ink)
            Text(model.localized("扫描桌面端显示的二维码，开始远程处理任务。"))
                .font(.system(size: 13))
                .foregroundStyle(OpenBitFunTheme.muted)
                .multilineTextAlignment(.center)
                .lineSpacing(7)
                .padding(.horizontal, 20)
            Button(model.localized("连接")) { model.connectRemote() }
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.contentOnAction)
                .frame(width: 136, height: 44)
                .background(OpenBitFunTheme.accent)
                .clipShape(Capsule())
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 20)
        .padding(.bottom, 48)
        .background(OpenBitFunTheme.page)
    }
}

struct RemoteConnectedHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(spacing: 10) {
            Spacer()
            Text(model.localized(model.remoteSessions.isEmpty ? "还没有远程对话" : "选择一个会话"))
                .font(MobileDesignTypography.headlineMedium.font)
                .foregroundStyle(OpenBitFunTheme.ink)
            Text(model.localized(
                model.remoteSessions.isEmpty
                    ? "新建聊天后，可以从手机继续处理桌面端任务。"
                    : "从侧边栏打开会话，或新建一个。"
            ))
                .font(MobileDesignTypography.bodyMedium.font)
                .foregroundStyle(OpenBitFunTheme.muted)
                .multilineTextAlignment(.center)
                .lineLimit(2)
                .frame(maxWidth: 280)
            Group {
                if model.selectedRemoteWorkspaceKind != "assistant",
                   HarnessProfilePolicy.shared.supported(capabilities: model.remoteHostCapabilities) {
                    Menu {
                        ForEach([HarnessProfile.minimal, .standard, .ultimate], id: \.name) { profile in
                            Button { model.createRemoteSessionFromHome(agentType: profile.agentType) } label: {
                                HarnessProfileLabel(model: model, profile: profile)
                            }
                        }
                    } label: { createLabel }
                } else {
                    Button { model.createRemoteSessionFromHome() } label: { createLabel }
                }
            }
            .buttonStyle(.plain)
            .disabled(model.remoteCreateSubmitting || !model.remoteCreateInteraction.canSubmit)

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 24)
        .padding(.bottom, 56)
        .background(OpenBitFunTheme.page)
    }
    private var createLabel: some View {
                HStack(spacing: 8) {
                    if model.remoteCreateSubmitting {
                        ProgressView().controlSize(.small).tint(OpenBitFunTheme.contentOnAction)
                    }
                    Text(model.localized(model.remoteCreateSubmitting ? "正在加载" : "新建会话"))
                }
                .font(MobileDesignTypography.titleSmall.font)
                .foregroundStyle(OpenBitFunTheme.contentOnAction)
                .frame(width: 148, height: 46)
                .background(OpenBitFunTheme.accent)
                .clipShape(Capsule())
    }
}

struct ConnectionStatusBar: View {
    let phase: ConnectionPhase
    var detail: String?
    let onRetry: () -> Void
    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(phase == .reconnecting ? OpenBitFunTheme.muted : OpenBitFunTheme.statusDanger).frame(width: 8, height: 8)
            Text(MobileLocalization.text(phase == .reconnecting ? "正在恢复连接" : "连接不可用"))
                .font(.system(size: 13, weight: .medium))
            Text(
                detail ?? MobileLocalization.text(
                    phase == .reconnecting ? "正在重新连接桌面端" : "请重新连接"
                )
            )
                .font(.system(size: 12))
                .foregroundStyle(OpenBitFunTheme.muted)
            Spacer()
            if phase == .disconnected {
                Button(MobileLocalization.text("重试"), action: onRetry)
                    .font(.system(size: 13, weight: .semibold))
                    .buttonStyle(.plain)
                    .foregroundStyle(OpenBitFunTheme.accent)
            }
        }
        .foregroundStyle(OpenBitFunTheme.ink)
        .padding(.horizontal, 18)
        .frame(height: 48)
        .background(OpenBitFunTheme.soft)
    }


}
