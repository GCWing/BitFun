import SwiftUI

struct ChatTimelineView: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: false) {
                LazyVStack(spacing: 0) {
                    ForEach(model.messages) { message in
                        ChatMessageBubble(message: message)
                            .id(message.id)
                    }
                    if model.isSending {
                        HStack(spacing: 5) {
                            Circle().fill(BitFunTheme.muted).frame(width: 5, height: 5)
                            Circle().fill(BitFunTheme.muted).frame(width: 5, height: 5)
                            Circle().fill(BitFunTheme.muted).frame(width: 5, height: 5)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 20)
                        .padding(.vertical, 15)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.top, 8)
                .padding(.bottom, 14)
            }
            .onChange(of: model.messages.count) { _ in
                if let id = model.messages.last?.id {
                    withAnimation(.easeOut(duration: 0.18)) { proxy.scrollTo(id, anchor: .bottom) }
                }
            }
        }
        .background(BitFunTheme.page)
    }
}

private struct ChatMessageBubble: View {
    let message: ChatMessage

    var body: some View {
        VStack(alignment: message.role == .user ? .trailing : .leading, spacing: 0) {
            Text(message.text)
                .font(.system(size: 15, weight: .regular))
                .foregroundStyle(BitFunTheme.ink)
                .lineSpacing(4)
                .padding(.horizontal, 14)
                .padding(.vertical, 11)
                .background(message.role == .user ? BitFunTheme.soft : BitFunTheme.card)
                .clipShape(RoundedRectangle(cornerRadius: 17))
                .overlay(RoundedRectangle(cornerRadius: 17).stroke(BitFunTheme.line, lineWidth: 1))
                .frame(maxWidth: 320, alignment: message.role == .user ? .trailing : .leading)
        }
        .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
        .padding(.top, message.role == .user ? 8 : 2)
        .padding(.bottom, message.role == .user ? 12 : 10)
    }
}
