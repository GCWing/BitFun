import SwiftUI

struct ConversationHeader: View {
    @ObservedObject var model: MobileAppModel
    var contextTitle: String? = nil
    @State private var menuOpen = false

    private var resolvedSubtitle: String? {
        if let contextTitle, !contextTitle.isEmpty { return contextTitle }
        if model.surface == .local && model.localSessionSelected { return "本地会话" }
        if model.remoteConnected && model.remoteSessionSelected { return "DESKTOP-KM3L4UI" }
        return nil
    }

    var body: some View {
        HStack(spacing: 8) {
            Button { model.drawerOpen = true } label: {
                ReferenceGlyph(assetName: "MenuGlyph", width: 23, height: 18)
                    .frame(
                        width: MobileDesignGeometry.controlTouchSize,
                        height: MobileDesignGeometry.controlTouchSize
                    )
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.07), radius: 8, y: 3)
            }
            .buttonStyle(.plain)
            VStack(spacing: 3) {
                Text(model.selectedSession?.title ?? "BitFun")
                    .font(
                        (resolvedSubtitle == nil
                            ? MobileDesignTypography.titleMedium
                            : MobileDesignTypography.conversationHeaderTitle).font
                    )
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                if let resolvedSubtitle {
                    Text(resolvedSubtitle)
                        .font(MobileDesignTypography.labelMedium.font)
                        .foregroundStyle(BitFunTheme.muted)
                }
            }
            .frame(maxWidth: .infinity)
            .contentShape(Rectangle())
            .onTapGesture { }

            Menu {
                Button("置顶会话") { }
                Button("导出会话") { }
                Button("归档会话") { }
            } label: {
                ReferenceGlyph(assetName: "MoreGlyph", width: 23, height: 7)
                    .frame(
                        width: MobileDesignGeometry.controlTouchSize,
                        height: MobileDesignGeometry.controlTouchSize
                    )
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.07), radius: 8, y: 3)
            }
        }
        .frame(
            height: resolvedSubtitle == nil
                ? MobileDesignGeometry.conversationHeaderCompactHeight
                : MobileDesignGeometry.conversationHeaderHeight
        )
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .background(BitFunTheme.page)
    }
}
