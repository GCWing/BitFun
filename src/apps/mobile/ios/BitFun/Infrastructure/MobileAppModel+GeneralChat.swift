import Foundation
import BitFunMobileCore

extension MobileAppModel {
    func send() {
        if surface == .remote {
            sendRemote()
            return
        }
        let value = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty || !composerImages.isEmpty else { return }
        guard !isSending && !busy else { return }
        if surface == .local {
            localSessionSelected = true
            if selectedSession == nil, let first = sessions.first {
                selectedSessionID = first.id
            }
        }
        let optimisticMessage = ChatMessage(id: UUID(), role: .user, text: value)
        messages.append(optimisticMessage)
        timelineRows.append(Self.simpleTimelineRow(optimisticMessage, images: composerImages))
        draft = ""
        isSending = true
        busy = true
        coreAdapter?.updateDraft(value)
        coreAdapter?.setGeneralChatImages(composerImages)
        composerImages = []
        coreAdapter?.send()
    }

    func select(_ session: ChatSession) {
        pendingDirectoryRemoteDraft = nil
        selectedSessionID = session.id
        if surface == .remote {
            remoteSessionSelected = true
            coreAdapter?.openRemoteSession(sessionID: session.id)
        } else {
            localSessionSelected = true
            coreAdapter?.selectGeneralSession(sessionID: session.id)
        }
        drawerOpen = false
    }

    func newLocalChat() {
        pendingDirectoryRemoteDraft = nil
        surface = .local
        drawerOpen = false
        localSessionSelected = false
        selectedSessionID = ""
        messages = []
        timelineRows = []
        draft = ""
        composerImages = []
        coreAdapter?.newGeneralSession()
    }

    func archiveLocalSession(_ session: ChatSession) {
        coreAdapter?.archiveGeneralSession(
            sessionID: session.id,
            archived: session.status.lowercased() != "archived"
        )
    }

    func deleteLocalSession(_ session: ChatSession) {
        coreAdapter?.deleteGeneralSession(sessionID: session.id)
        if selectedSessionID == session.id {
            localSessionSelected = false
        }
    }

    func saveGeneralConfig(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        coreAdapter?.saveGeneralConfig(
            baseURL: baseURL, model: model, apiKey: apiKey, clearAPIKey: clearAPIKey
        )
    }

    func testGeneralConnection(baseURL: String, model: String, apiKey: String, clearAPIKey: Bool) {
        coreAdapter?.testGeneralConnection(
            baseURL: baseURL, model: model, apiKey: apiKey, clearAPIKey: clearAPIKey
        )
    }

    func exportSelectedSession() {
        guard surface == .local, let session = selectedSession else { return }
        coreAdapter?.exportGeneralSession(sessionID: session.id)
    }

    func exportLocalSession(_ session: ChatSession) {
        coreAdapter?.exportGeneralSession(sessionID: session.id)
    }

    func finishGeneralExport() {
        generalExportOpen = false
        generalExportData = Data()
        coreAdapter?.clearGeneralExport()
    }

    func syncDraftToCore() {
        if surface == .local {
            coreAdapter?.updateDraft(draft)
        }
    }

    func addComposerImage(data: Data, mimeType: String) {
        guard composerImages.count < 4, data.count <= 10 * 1024 * 1024 else {
            showToast(localized("最多添加 4 张且每张不超过 10 MB 的图片"))
            return
        }
        composerImages.append(
            ComposerAttachment(id: UUID().uuidString, data: data, mimeType: mimeType)
        )
        if surface == .local {
            coreAdapter?.setGeneralChatImages(composerImages)
        }
    }

    func removeComposerImage(id: String) {
        composerImages.removeAll { $0.id == id }
        if surface == .local {
            coreAdapter?.setGeneralChatImages(composerImages)
        }
    }

    func selectModel(_ modelID: String) {
        guard selectedSession != nil else { return }
        if surface == .remote {
            coreAdapter?.selectRemoteModel(sessionID: selectedSessionID, modelID: modelID)
        } else {
            coreAdapter?.selectGeneralModel(modelID: modelID)
        }
    }

    func apply(coreState state: GeneralChatUiState) {
        generalConfigured = state.configured
        generalConfigBaseURL = state.config.baseUrl
        generalConfigModel = state.config.model
        generalConfigHasAPIKey = state.config.hasApiKey
        generalConfigFailure = state.configFailure?.name
        generalConnectionTestRunning = state.connectionTest.running
        if state.connectionTest.passed {
            generalConnectionTestMessage = localized("连接成功")
        } else if let failure = state.connectionTest.failure {
            generalConnectionTestMessage = localizedFormat("连接失败：%@", failure.name)
        } else {
            generalConnectionTestMessage = nil
        }
        if let exported = state.export {
            let safeTitle = exported.title
                .replacingOccurrences(of: "/", with: "-")
                .replacingOccurrences(of: "\\", with: "-")
                .replacingOccurrences(of: ":", with: "-")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            generalExportName = safeTitle.isEmpty ? "conversation.md" : "\(safeTitle).md"
            generalExportData = Data(exported.markdown.utf8)
            generalExportOpen = true
        }
        if !state.sessions.isEmpty {
            sessions = state.sessions.map { session in
                ChatSession(
                    id: session.id,
                    title: session.title.isEmpty ? localized("未命名会话") : session.title,
                    updatedLabel: session.updatedAt,
                    pinned: session.pinned,
                    status: session.status,
                )
            }
        }
        if !state.messages.isEmpty {
            messages = state.messages.map { message in
                let text = message.blocks.map(\.text).joined(separator: "\n")
                return ChatMessage(
                    id: UUID(uuidString: message.id) ?? UUID(),
                    role: message.role.lowercased() == "user" ? .user : .assistant,
                    text: text,
                )
            }
            timelineRows = messages.map(Self.simpleTimelineRow)
        }
        if !composerModelPickerPreview, draft != state.draft { draft = state.draft }
        isSending = state.busy
        busy = state.busy
        if !composerModelPickerPreview {
            modelOptions = state.models.map { model in
                ComposerModelOption(
                    id: model.id,
                    primaryLabel: model.label,
                    secondaryLabel: model.source.name,
                    source: model.source.name,
                    selected: model.id == state.activeModelId
                )
            }
        }
        if !accountLoginPreview {
            if let failure = state.failure {
                coreErrorMessage = failure.name
            } else {
                coreErrorMessage = nil
            }
        }
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
            showRetry: false
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
            showRetry: row.showRetry
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
