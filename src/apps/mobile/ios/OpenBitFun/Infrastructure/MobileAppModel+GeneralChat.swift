import Foundation
import OpenBitFunMobileCore

extension MobileAppModel {
    func send() { sendRemote() }

    func select(_ session: ChatSession) {
        pendingDirectoryRemoteDraft = nil
        selectedSessionID = session.id
        remoteSessionSelected = true
        coreAdapter?.openRemoteSession(sessionID: session.id)
        drawerOpen = false
    }

    func addComposerImage(data: Data, mimeType: String) {
        guard composerImages.count < 4, data.count <= 10 * 1024 * 1024 else {
            showToast(localized("最多添加 4 张且每张不超过 10 MB 的图片"))
            return
        }
        composerImages.append(ComposerAttachment(id: UUID().uuidString, data: data, mimeType: mimeType))
    }

    func removeComposerImage(id: String) {
        composerImages.removeAll { $0.id == id }
    }

    func selectModel(_ modelID: String) {
        guard selectedSession != nil else { return }
        coreAdapter?.selectRemoteModel(sessionID: selectedSessionID, modelID: modelID)
    }

    static func simpleTimelineRow(_ message: ChatMessage) -> MobileConversationRow {
        simpleTimelineRow(message, images: [])
    }

    static func simpleTimelineRow(
        _ message: ChatMessage,
        images: [ComposerAttachment]
    ) -> MobileConversationRow {
        MobileConversationRow(
            id: message.id.uuidString,
            kind: message.role == .user ? "USER" : "ASSISTANT",
            text: message.text,
            thinking: nil,
            images: images.map {
                MobileTimelineImage(name: "image", dataURL: $0.dataURL)
            },
            tools: [],
            blocks: [],
            streaming: false,
            typing: false,
            pending: false,
            showRetry: false,
            error: nil
        )
    }

    static func mapConversationRow(_ row: ConversationRow) -> MobileConversationRow {
        MobileConversationRow(
            id: row.id,
            kind: row.kind.name,
            text: row.text,
            thinking: row.thinking,
            images: row.images.map {
                MobileTimelineImage(name: $0.name, dataURL: $0.dataUrl)
            },
            tools: row.tools.map(mapTool),
            blocks: row.blocks.map(mapBlock),
            streaming: row.streaming,
            typing: row.typing,
            pending: row.pending,
            showRetry: row.showRetry,
            error: row.error
        )
    }

    static func mapTool(_ tool: ToolCard) -> MobileTimelineTool {
        MobileTimelineTool(
            id: tool.id,
            name: tool.name,
            phase: tool.phase.name,
            kind: tool.kind.name,
            operation: tool.operation.name,
            target: tool.target,
            filePath: tool.filePath,
            fileLabel: tool.fileLabel,
            input: tool.input,
            output: tool.output,
            question: tool.question,
            questions: tool.questions.map { question in
                MobileTimelineQuestion(
                    index: Int(question.index),
                    header: question.header,
                    question: question.question,
                    options: question.options.map {
                        MobileTimelineOption(label: $0.label, description: $0.description_)
                    },
                    multiSelect: question.multiSelect
                )
            },
            actions: Set(tool.actions.map(\.name))
        )
    }

    static func mapBlock(_ block: MessageBlock) -> MobileTimelineBlock {
        if let text = block as? MessageBlockText {
            return .text(id: text.id, text: text.text, streaming: text.streaming)
        }
        if let thinking = block as? MessageBlockThinking {
            return .thinking(id: thinking.id, text: thinking.text, streaming: thinking.streaming)
        }
        if let tools = block as? MessageBlockTools {
            return .tools(id: tools.id, tools: tools.tools.map(mapTool))
        }
        if let subagent = block as? MessageBlockSubagent {
            return .subagent(
                id: subagent.id,
                title: subagent.title,
                running: subagent.running,
                text: subagent.text,
                children: subagent.children.map(mapBlock)
            )
        }
        return .text(id: block.id, text: "", streaming: false)
    }
}
