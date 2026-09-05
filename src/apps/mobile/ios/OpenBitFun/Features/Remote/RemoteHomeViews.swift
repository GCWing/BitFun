import OpenBitFunMobileCore
import SwiftUI

struct RemoteHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ZStack(alignment: .topTrailing) {
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
            remoteSettingsButton
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 20)
        .padding(.bottom, 48)
        .background(OpenBitFunTheme.page)
    }

    private var remoteSettingsButton: some View {
        Button { model.remoteControlSettingsOpen = true } label: {
            Image(systemName: "gearshape")
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.ink)
                .frame(width: 44, height: 44)
                .background(OpenBitFunTheme.card)
                .overlay(Circle().stroke(OpenBitFunTheme.line, lineWidth: 1))
                .clipShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(model.localized("远程控制设置"))
        .padding(.top, 16).padding(.trailing, 16)
    }
}

struct RemoteConnectedHomeView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ZStack(alignment: .topTrailing) {
            VStack(spacing: 14) {
            Spacer()
            Image(systemName: "desktopcomputer.and.macbook")
                .font(.system(size: 34, weight: .medium)).foregroundStyle(OpenBitFunTheme.muted)
            Text(model.localized("桌面端已连接"))
                .font(MobileDesignTypography.titleMedium.font).foregroundStyle(OpenBitFunTheme.ink)
            Text(model.localized("选择已有会话，或在当前工作区创建一个新会话。"))
                .font(MobileDesignTypography.bodySmall.font).foregroundStyle(OpenBitFunTheme.muted)
                .multilineTextAlignment(.center)
            Button { model.remoteCreateOpen = true } label: {
                Label(model.localized("新建远程会话"), systemImage: "plus")
                    .font(MobileDesignTypography.labelMedium.font).foregroundStyle(OpenBitFunTheme.contentOnAction)
                    .frame(minWidth: 176, minHeight: 44).background(OpenBitFunTheme.accent).clipShape(Capsule())
            }
            .buttonStyle(.plain)
            Spacer()
            }
            Button { model.remoteControlSettingsOpen = true } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .frame(width: 44, height: 44)
                    .background(OpenBitFunTheme.card)
                    .overlay(Circle().stroke(OpenBitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("远程控制设置"))
            .padding(.top, 16).padding(.trailing, 16)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(OpenBitFunTheme.page)
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
