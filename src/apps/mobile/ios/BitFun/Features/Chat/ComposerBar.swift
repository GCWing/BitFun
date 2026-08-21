import SwiftUI

struct ComposerBar: View {
    @ObservedObject var model: MobileAppModel
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 5) {
            Button { } label: {
                ReferenceGlyph(assetName: "ComposerPlusGlyph", width: 18, height: 18)
                    .frame(width: 40, height: 40)
            }
            .buttonStyle(.plain)
            TextField(
                model.surface == .remote ? "向 BitFun 提问" : (model.localSessionSelected ? "输入消息" : "问问 BitFun"),
                text: $model.draft,
                axis: .vertical
            )
                .font(.system(size: 15))
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
                        .frame(width: 40, height: 40)
                } else if model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    ReferenceGlyph(assetName: "ComposerMicGlyph", width: 16, height: 19)
                        .foregroundStyle(BitFunTheme.muted)
                        .frame(width: 40, height: 40)
                } else {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(BitFunTheme.accent)
                        .frame(width: 40, height: 40)
                }
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .frame(minHeight: 52)
        .background(BitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: 20))
        .overlay(RoundedRectangle(cornerRadius: 20).stroke(BitFunTheme.line, lineWidth: 1))
        .shadow(color: .black.opacity(0.07), radius: 10, y: 2)
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, 14)
        .background(BitFunTheme.page)
    }
}
