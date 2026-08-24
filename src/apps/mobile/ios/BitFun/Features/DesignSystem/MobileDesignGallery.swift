import SwiftUI

struct MobileDesignGallery: View {
    let scenario: MobilePreviewScenario
    @StateObject private var model: MobileAppModel

    init(scenario: MobilePreviewScenario) {
        self.scenario = scenario
        let session = ChatSession(id: UUID(), title: scenario.headerTitle, updatedLabel: "刚刚")
        let previewModel = MobileAppModel(
            sessions: [session],
            selectedSessionID: session.id,
            messages: scenario.messages.map { message in
                ChatMessage(
                    id: UUID(),
                    role: message.role == "user" ? .user : .assistant,
                    text: message.text
                )
            }
        )
        previewModel.surface = .remote
        previewModel.remoteConnected = true
        previewModel.remoteSessionSelected = true
        previewModel.remoteSessions = [session]
        previewModel.draft = scenario.composerDraft
        previewModel.isSending = scenario.streaming
        _model = StateObject(wrappedValue: previewModel)
    }

    var body: some View {
        VStack(spacing: 0) {
            platformLabel
            ConversationHeader(model: model, contextTitle: scenario.headerSubtitle)
            ChatTimelineView(model: model)
            ComposerBar(model: model)
        }
        .background(BitFunTheme.page)
    }

    private var platformLabel: some View {
        HStack(spacing: 8) {
            Text("iOS")
                .font(MobileDesignTypography.labelMedium.font)
                .fontWeight(.medium)
            Text("NATIVE")
                .font(MobileDesignTypography.labelSmall.font)
                .foregroundStyle(BitFunTheme.muted)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(BitFunTheme.soft)
                .clipShape(Capsule())
            Spacer()
            Text("\(Int(scenario.viewportWidth)) × \(Int(scenario.viewportHeight))")
                .font(MobileDesignTypography.labelSmall.font)
                .foregroundStyle(BitFunTheme.muted)
        }
        .frame(height: MobileDesignGeometry.connectionStripHeight)
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .overlay(alignment: .bottom) {
            Rectangle().fill(BitFunTheme.line).frame(height: 1)
        }
    }
}

#Preview("BitFun Mobile · Compact") {
    MobileDesignGallery(scenario: MobilePreviewScenarios.connectedConversation)
        .preferredColorScheme(.light)
}

#Preview("BitFun Mobile · Dark") {
    MobileDesignGallery(scenario: MobilePreviewScenarios.streamingDark)
        .preferredColorScheme(.dark)
}
