import SwiftUI

struct ComposerBar: View {
    @ObservedObject var model: MobileAppModel
    @FocusState private var focused: Bool

    var body: some View {
        let placeholder = model.surface == .remote
            ? "向 BitFun 提问"
            : (model.localSessionSelected ? "输入消息" : "问问 BitFun")
        HStack(spacing: 5) {
            Button { } label: {
                ReferenceGlyph(assetName: "ComposerPlusGlyph", width: 18, height: 18)
                    .frame(
                        width: MobileDesignGeometry.composerActionSize,
                        height: MobileDesignGeometry.composerActionSize
                    )
            }
            .buttonStyle(.plain)
            TextField(
                "",
                text: $model.draft,
                prompt: Text(placeholder).foregroundColor(BitFunTheme.muted),
                axis: .vertical
            )
                .font(MobileDesignTypography.bodyLarge.font)
                .foregroundStyle(BitFunTheme.ink)
                .lineLimit(1...4)
                .focused($focused)
                .submitLabel(.send)
                .onSubmit { model.send() }
                .onChange(of: model.draft) { _ in model.syncDraftToCore() }
            Button { model.send() } label: {
                if model.isSending {
                    Image(systemName: "stop.fill")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(BitFunTheme.accent)
                        .frame(
                            width: MobileDesignGeometry.composerActionSize,
                            height: MobileDesignGeometry.composerActionSize
                        )
                } else if model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    ReferenceGlyph(assetName: "ComposerMicGlyph", width: 16, height: 19)
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(
                            width: MobileDesignGeometry.composerActionSize,
                            height: MobileDesignGeometry.composerActionSize
                        )
                } else {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(BitFunTheme.accent)
                        .frame(
                            width: MobileDesignGeometry.composerActionSize,
                            height: MobileDesignGeometry.composerActionSize
                        )
                }
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .frame(minHeight: MobileDesignGeometry.composerCollapsedHeight)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.composerCollapsedRadius))
        .shadow(color: .black.opacity(0.05), radius: 10, y: 2)
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .padding(.top, 8)
        .padding(.bottom, 14)
        .background(BitFunTheme.page)
    }
}
