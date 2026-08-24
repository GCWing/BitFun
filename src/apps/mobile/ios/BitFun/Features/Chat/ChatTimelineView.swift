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
                .padding(.horizontal, MobileDesignGeometry.contentGutter)
                .padding(.top, MobileDesignGeometry.timelineTopPadding)
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
                .font(MobileDesignTypography.bodyMedium.font)
                .foregroundStyle(BitFunTheme.ink)
                .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing)
                .padding(.horizontal, MobileDesignGeometry.messageBubbleHorizontalPadding)
                .padding(.vertical, MobileDesignGeometry.messageBubbleVerticalPadding)
                .background(message.role == .user ? BitFunTheme.soft : BitFunTheme.card)
                .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.messageBubbleRadius))
                .overlay(
                    RoundedRectangle(cornerRadius: MobileDesignGeometry.messageBubbleRadius)
                        .stroke(BitFunTheme.line, lineWidth: 1)
                )
                .frame(
                    maxWidth: MobileDesignGeometry.messageBubbleMaxWidth,
                    alignment: message.role == .user ? .trailing : .leading
                )
        }
        .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
        .padding(.bottom, MobileDesignGeometry.messageSpacing)
    }
}
