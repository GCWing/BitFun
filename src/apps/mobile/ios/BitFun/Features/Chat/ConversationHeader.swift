import SwiftUI

struct ConversationHeader: View {
    @ObservedObject var model: MobileAppModel
    @State private var menuOpen = false

    var body: some View {
        HStack(spacing: 8) {
            Button { model.drawerOpen = true } label: {
                ReferenceGlyph(assetName: "MenuGlyph", width: 23, height: 18)
                    .frame(width: 44, height: 44)
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.07), radius: 8, y: 3)
            }
            .buttonStyle(.plain)
            VStack(spacing: 3) {
                Text(model.selectedSession?.title ?? "BitFun")
                    .font(.system(size: 17, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                if model.surface == .local && model.localSessionSelected {
                    Text("本地会话")
                        .font(.system(size: 14))
                        .foregroundStyle(BitFunTheme.muted)
                } else if model.remoteConnected && model.remoteSessionSelected {
                    Text("DESKTOP-KM3L4UI")
                        .font(.system(size: 14))
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
                    .frame(width: 44, height: 44)
                    .background(BitFunTheme.card)
                    .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                    .clipShape(Circle())
                    .shadow(color: .black.opacity(0.07), radius: 8, y: 3)
            }
        }
        .frame(height: 76)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(BitFunTheme.page)
    }
}
