import OpenBitFunMobileCore
import SwiftUI
import UIKit

struct ChatTimelineView: View {
    @ObservedObject var model: MobileAppModel
    @State private var userScrolledUp = false

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: false) {
                LazyVStack(spacing: MobileDesignGeometry.messageSpacing) {
                    if model.surface == .remote && model.remoteHasMoreMessages {
                        Button { model.loadOlderRemoteMessages() } label: {
                            HStack(spacing: 7) {
                                if model.busy { ProgressView().controlSize(.small) }
                                Text(model.localized(model.busy ? "正在加载" : "加载更早消息"))
                                    .font(MobileDesignTypography.labelSmall.font)
                            }
                            .foregroundStyle(OpenBitFunTheme.muted)
                            .frame(maxWidth: .infinity, minHeight: 38)
                        }
                        .buttonStyle(.plain)
                        .disabled(model.busy)
                    }
                    ForEach(model.timelineRows) { row in
                        ConversationRowView(row: row, model: model).id(row.id)
                    }
                    if model.timelineRows.isEmpty && model.isSending {
                        TypingIndicator().frame(maxWidth: .infinity, alignment: .leading)
                    }
                    OpenBitFunTheme.transparent.frame(height: 1).id("timeline-bottom")
                }
                .padding(.horizontal, MobileDesignGeometry.contentGutter)
                .padding(.top, MobileDesignGeometry.timelineTopPadding)
                .padding(.bottom, 14)
            }
            .simultaneousGesture(
                DragGesture(minimumDistance: 8).onChanged { value in
                    if value.translation.height < -8 { userScrolledUp = true }
                }
            )
            .onChange(of: model.timelineRows) { _ in
                guard !userScrolledUp else { return }
                withAnimation(.easeOut(duration: 0.18)) {
                    proxy.scrollTo("timeline-bottom", anchor: .bottom)
                }
            }
            .overlay(alignment: .bottomTrailing) {
                if userScrolledUp {
                    Button {
                        userScrolledUp = false
                        withAnimation(.easeOut(duration: 0.18)) {
                            proxy.scrollTo("timeline-bottom", anchor: .bottom)
                        }
                    } label: {
                        Image(systemName: "chevron.down")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(OpenBitFunTheme.ink)
                            .frame(width: 36, height: 36)
                            .background(OpenBitFunTheme.card)
                            .clipShape(Circle())
                            .overlay(Circle().stroke(OpenBitFunTheme.line, lineWidth: 1))
                            .shadow(color: OpenBitFunTheme.shadowMedium, radius: 8, y: 3)
                    }
                    .buttonStyle(.plain)
                    .padding(18)
                    .accessibilityLabel(Text(model.localized("滚动到底部")))
                }
            }
        }
        .background(OpenBitFunTheme.page)
    }
}

private struct ConversationRowView: View {
    let row: MobileConversationRow
    @ObservedObject var model: MobileAppModel

    @ViewBuilder
    var body: some View {
        switch row.kind {
        case "EMPTY":
            EmptyConversationRow()
        case "USER":
            userRow
                .accessibilityElement(children: .contain)
                .accessibilityIdentifier("message.user.\(row.id)")
        default:
            assistantRow
                .accessibilityElement(children: .contain)
                .accessibilityIdentifier("message.assistant.\(row.id)")
        }
    }

    private var userRow: some View {
        VStack(alignment: .trailing, spacing: 7) {
            if !row.images.isEmpty { TimelineImageGrid(images: row.images) }
            if !row.text.isEmpty {
                Text(row.text)
                    .font(MobileDesignTypography.bodyMedium.font)
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing)
                    .textSelection(.enabled)
            }
            if row.pending {
                Text(model.localized("正在发送"))
                    .font(MobileDesignTypography.labelSmall.font)
                    .foregroundStyle(OpenBitFunTheme.muted)
            }
            if row.showRetry {
                Button { model.retryMessage(row.text) } label: {
                    Label(model.localized("重新发送"), systemImage: "arrow.clockwise")
                        .font(MobileDesignTypography.labelSmall.font)
                        .foregroundStyle(OpenBitFunTheme.statusDanger)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, MobileDesignGeometry.messageBubbleHorizontalPadding)
        .padding(.vertical, MobileDesignGeometry.messageBubbleVerticalPadding)
        .frame(maxWidth: MobileDesignGeometry.messageBubbleMaxWidth, alignment: .trailing)
        .background(OpenBitFunTheme.soft)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.messageBubbleRadius))
        .frame(maxWidth: .infinity, alignment: .trailing)
    }

    private var assistantRow: some View {
        VStack(alignment: .leading, spacing: 10) {
            if row.typing {
                TypingIndicator()
            } else if !row.blocks.isEmpty {
                MessageBlockList(blocks: row.blocks, model: model)
            } else {
                if let thinking = row.thinking, !thinking.isEmpty {
                    ThinkingBlock(text: thinking, streaming: row.streaming)
                }
                if !row.text.isEmpty { MarkdownMessageView(text: row.text, model: model) }
                if !row.tools.isEmpty { ToolStatusList(tools: row.tools, model: model) }
            }
            if !row.images.isEmpty { TimelineImageGrid(images: row.images) }
            if row.showRetry {
                Button { model.retryMessage(row.text) } label: {
                    Label(model.localized("重试"), systemImage: "arrow.clockwise")
                        .font(MobileDesignTypography.labelSmall.font)
                        .foregroundStyle(OpenBitFunTheme.statusDanger)
                }
                .buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct EmptyConversationRow: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "sparkles").font(.system(size: 23, weight: .medium))
            Text(MobileLocalization.text("从这里开始新的对话"))
                .font(MobileDesignTypography.bodyMedium.font)
        }
        .foregroundStyle(OpenBitFunTheme.muted)
        .frame(maxWidth: .infinity, minHeight: 180)
    }
}

private struct MessageBlockList: View {
    let blocks: [MobileTimelineBlock]
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(blocks) { block in
                switch block {
                case let .text(_, text, _):
                    if !text.isEmpty { MarkdownMessageView(text: text, model: model) }
                case let .thinking(_, text, streaming):
                    ThinkingBlock(text: text, streaming: streaming)
                case let .tools(_, tools):
                    ToolStatusList(tools: tools, model: model)
                case let .subagent(_, title, running, text, children):
                    SubagentBlock(title: title, running: running, text: text, children: children, model: model)
                }
            }
        }
    }
}

private struct SubagentBlock: View {
    let title: String
    let running: Bool
    let text: String
    let children: [MobileTimelineBlock]
    @ObservedObject var model: MobileAppModel
    @State private var expanded = true

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button { withAnimation(.easeOut(duration: 0.18)) { expanded.toggle() } } label: {
                HStack(spacing: 7) {
                    Image(systemName: "person.2.fill").font(.system(size: 12, weight: .medium))
                    Text(title.isEmpty ? model.localized("子任务") : title)
                        .font(MobileDesignTypography.labelMedium.font).lineLimit(1)
                    Spacer()
                    if running { ProgressView().controlSize(.mini) }
                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 11, weight: .semibold))
                }
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(minHeight: 32)
            }
            .buttonStyle(.plain)
            if expanded {
                if !text.isEmpty { MarkdownMessageView(text: text, model: model) }
                if !children.isEmpty { MessageBlockList(blocks: children, model: model) }
            }
        }
        .padding(.leading, 12)
        .overlay(alignment: .leading) { Rectangle().fill(OpenBitFunTheme.line).frame(width: 2) }
    }
}

private struct ThinkingBlock: View {
    let text: String
    let streaming: Bool
    @State private var expanded: Bool

    init(text: String, streaming: Bool) {
        self.text = text
        self.streaming = streaming
        _expanded = State(initialValue: streaming)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Button { withAnimation(.easeOut(duration: 0.18)) { expanded.toggle() } } label: {
                HStack(spacing: 7) {
                    if streaming { ProgressView().controlSize(.mini) }
                    Image(systemName: "sparkles").font(.system(size: 12, weight: .medium))
                    Text(MobileLocalization.text(streaming ? "正在思考" : "思考过程"))
                        .font(MobileDesignTypography.labelMedium.font)
                    Spacer()
                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 11, weight: .semibold))
                }
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(minHeight: 32)
            }
            .buttonStyle(.plain)
            if expanded {
                Text(text)
                    .font(MobileDesignTypography.bodyMedium.font)
                    .foregroundStyle(OpenBitFunTheme.muted)
                    .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing)
                    .textSelection(.enabled)
            }
        }
    }
}

private struct TypingIndicator: View {
    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.35)) { context in
            let phase = Int(context.date.timeIntervalSinceReferenceDate / 0.35) % 3
            HStack(spacing: 5) {
                ForEach(0..<3) { index in
                    Circle().fill(OpenBitFunTheme.muted).frame(width: 5, height: 5)
                        .opacity(index == phase ? 1 : 0.32)
                }
            }
            .frame(height: 32)
        }
        .accessibilityLabel(Text(MobileLocalization.text("正在回复")))
    }
}

struct MarkdownMessageView: View {
    let text: String
    @ObservedObject var model: MobileAppModel

    private var blocks: [MarkdownBlock] { MarkdownParser.shared.parse(text: text) }
    private var references: [MessageFileReference] {
        MessageFileReferenceProjector.shared.project(source: text)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            ForEach(blocks, id: \.id) { MarkdownBlockView(block: $0) }
            ForEach(references, id: \.id) { FileReferenceCard(reference: $0, model: model) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .environment(\.openURL, OpenURLAction { url in
            if url.scheme?.lowercased() == "computer" {
                model.openRemoteFile(reference: url.absoluteString, label: url.lastPathComponent)
                return .handled
            }
            return .systemAction
        })
    }
}

private struct MarkdownBlockView: View {
    let block: MarkdownBlock

    var body: some View {
        switch block.type {
        case "heading":
            Text(inlineString(block.inlines))
                .font(.system(size: headingSize, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.ink).textSelection(.enabled)
        case "quote":
            Text(inlineString(block.inlines))
                .font(MobileDesignTypography.bodyMedium.font).foregroundStyle(OpenBitFunTheme.muted)
                .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing).padding(.leading, 12)
                .overlay(alignment: .leading) { Rectangle().fill(OpenBitFunTheme.line).frame(width: 2) }
                .textSelection(.enabled)
        case "list":
            VStack(alignment: .leading, spacing: 5) {
                ForEach(block.items, id: \.id) { item in
                    HStack(alignment: .firstTextBaseline, spacing: 7) {
                        Text(item.marker).foregroundStyle(OpenBitFunTheme.muted)
                            .frame(width: 20, alignment: .trailing)
                        Text(inlineString(item.inlines)).foregroundStyle(OpenBitFunTheme.ink)
                            .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing)
                            .textSelection(.enabled)
                    }
                    .font(MobileDesignTypography.bodyMedium.font)
                }
            }
        case "code": CodeBlock(language: block.language, code: block.text)
        case "table":
            ScrollView(.horizontal, showsIndicators: false) {
                Text(block.text).font(.system(size: 12.5, design: .monospaced))
                    .foregroundStyle(OpenBitFunTheme.ink).padding(12).textSelection(.enabled)
            }
            .background(OpenBitFunTheme.soft).clipShape(RoundedRectangle(cornerRadius: 12))
        case "divider": Rectangle().fill(OpenBitFunTheme.line).frame(height: 1).padding(.vertical, 3)
        default:
            Text(inlineString(block.inlines))
                .font(MobileDesignTypography.bodyMedium.font).foregroundStyle(OpenBitFunTheme.ink)
                .lineSpacing(MobileDesignTypography.bodyMedium.lineSpacing).textSelection(.enabled)
        }
    }

    private var headingSize: CGFloat {
        switch block.level { case 1: 18; case 2: 16; default: 15 }
    }

    private func inlineString(_ inlines: [MarkdownInline]) -> AttributedString {
        var result = AttributedString()
        for inline in inlines {
            var part = AttributedString(inline.text)
            switch inline.type {
            case "strong": part.font = .system(size: 14, weight: .semibold)
            case "emphasis": part.font = .system(size: 14).italic()
            case "code":
                part.font = .system(size: 13, design: .monospaced)
                part.backgroundColor = OpenBitFunTheme.soft
            case "link":
                part.foregroundColor = MobileDesignColors.fileLink
                part.underlineStyle = .single
                part.link = URL(string: inline.url)
            default: break
            }
            result.append(part)
        }
        return result.characters.isEmpty ? AttributedString(block.text) : result
    }
}

private struct CodeBlock: View {
    let language: String
    let code: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(language.isEmpty ? MobileLocalization.text("代码") : language)
                    .font(MobileDesignTypography.labelSmall.font).foregroundStyle(OpenBitFunTheme.muted)
                Spacer()
                Button { UIPasteboard.general.string = code } label: {
                    Label(MobileLocalization.text("复制"), systemImage: "doc.on.doc")
                        .font(MobileDesignTypography.labelSmall.font).foregroundStyle(OpenBitFunTheme.muted)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 12).frame(height: 34)
            Rectangle().fill(OpenBitFunTheme.line).frame(height: 1)
            ScrollView(.horizontal, showsIndicators: false) {
                Text(code).font(.system(size: 12.5, design: .monospaced))
                    .foregroundStyle(OpenBitFunTheme.ink).padding(12).textSelection(.enabled)
            }
        }
        .background(OpenBitFunTheme.soft).clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(OpenBitFunTheme.line, lineWidth: 1))
    }
}

private struct FileReferenceCard: View {
    let reference: MessageFileReference
    @ObservedObject var model: MobileAppModel

    var body: some View {
        HStack(spacing: 10) {
            Button { model.openRemoteFile(reference: reference.reference, label: reference.label) } label: {
                HStack(spacing: 10) {
                    Image(systemName: "doc.text")
                        .font(.system(size: 16, weight: .medium)).foregroundStyle(MobileDesignColors.fileLink)
                        .frame(width: 34, height: 34).background(MobileDesignColors.fileLink.opacity(0.1))
                        .clipShape(RoundedRectangle(cornerRadius: 9))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(reference.label).font(MobileDesignTypography.labelMedium.font)
                            .foregroundStyle(OpenBitFunTheme.ink).lineLimit(1)
                        Text(reference.remotePath).font(MobileDesignTypography.labelSmall.font)
                            .foregroundStyle(OpenBitFunTheme.muted).lineLimit(1)
                        if let status = model.downloadStatus(for: reference.remotePath) {
                            Text(status).font(MobileDesignTypography.labelSmall.font)
                                .foregroundStyle(model.downloadPhase == .failed ? OpenBitFunTheme.statusDanger : OpenBitFunTheme.muted)
                                .lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
                }
            }
            .buttonStyle(.plain)
            Button { model.downloadRemoteFile(reference: reference.reference, label: reference.label) } label: {
                Group {
                    if model.downloadStatus(for: reference.remotePath) != nil,
                       [.preparing, .downloading, .saving].contains(model.downloadPhase) {
                        ProgressView().controlSize(.small)
                    } else if model.downloadStatus(for: reference.remotePath) != nil,
                              model.downloadPhase == .saved {
                        Image(systemName: "checkmark.circle")
                    } else {
                        Image(systemName: "arrow.down.circle")
                    }
                }
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.muted).frame(width: 40, height: 40)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localizedFormat("下载 %@", reference.label))
        }
        .padding(.leading, 10).padding(.trailing, 4).padding(.vertical, 7)
        .background(OpenBitFunTheme.card).clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(OpenBitFunTheme.line, lineWidth: 1))
    }
}

private struct TimelineImageGrid: View {
    let images: [MobileTimelineImage]
    @State private var selected: MobileTimelineImage?

    var body: some View {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 7) {
            ForEach(images) { image in
                Button { selected = image } label: {
                    if let uiImage = image.uiImage {
                        Image(uiImage: uiImage).resizable().scaledToFill()
                            .frame(height: images.count == 1 ? 180 : 112).frame(maxWidth: .infinity)
                            .clipped().clipShape(RoundedRectangle(cornerRadius: 14))
                    } else {
                        Image(systemName: "photo").foregroundStyle(OpenBitFunTheme.muted)
                            .frame(maxWidth: .infinity, minHeight: 112).background(OpenBitFunTheme.soft)
                            .clipShape(RoundedRectangle(cornerRadius: 14))
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .fullScreenCover(item: $selected) { FullScreenTimelineImage(image: $0) }
    }
}

private struct FullScreenTimelineImage: View {
    let image: MobileTimelineImage
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ZStack(alignment: .topTrailing) {
            OpenBitFunTheme.mediaBackground.ignoresSafeArea()
            if let uiImage = image.uiImage { Image(uiImage: uiImage).resizable().scaledToFit().ignoresSafeArea() }
            Button { dismiss() } label: {
                Image(systemName: "xmark").font(.system(size: 15, weight: .semibold)).foregroundStyle(OpenBitFunTheme.contentOnAction)
                    .frame(width: 44, height: 44).background(OpenBitFunTheme.mediaControlBackground).clipShape(Circle())
            }
            .buttonStyle(.plain).padding(20)
        }
    }
}

private extension MobileTimelineImage {
    var uiImage: UIImage? {
        guard let marker = dataURL.range(of: "base64,") else { return nil }
        return Data(base64Encoded: String(dataURL[marker.upperBound...])).flatMap(UIImage.init(data:))
    }
}

private enum ToolDisplayRow: Identifiable {
    case tool(MobileTimelineTool)
    case collapsed(id: String, tools: [MobileTimelineTool])
    var id: String {
        switch self { case let .tool(tool): tool.id; case let .collapsed(id, _): id }
    }
}

private struct ToolStatusList: View {
    let tools: [MobileTimelineTool]
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(displayRows) { row in
                switch row {
                case let .tool(tool): ToolStatusRow(tool: tool, model: model)
                case let .collapsed(_, tools): CollapsedToolsRow(tools: tools, model: model)
                }
            }
        }
    }

    private var displayRows: [ToolDisplayRow] {
        var result: [ToolDisplayRow] = []
        var pending: [MobileTimelineTool] = []
        func flush() {
            if pending.count < 2 { result.append(contentsOf: pending.map(ToolDisplayRow.tool)) }
            else if let first = pending.first {
                result.append(.collapsed(id: "collapsed-\(first.id)-\(pending.count)", tools: pending))
            }
            pending.removeAll()
        }
        for tool in tools {
            let collapsible = tool.actions.isEmpty
                && ["COMPLETED", "CANCELLED"].contains(tool.phase)
                && ["DOCUMENT", "FOLDER", "SEARCH"].contains(tool.kind)
            if collapsible { pending.append(tool) } else { flush(); result.append(.tool(tool)) }
        }
        flush()
        return result
    }
}

private struct CollapsedToolsRow: View {
    let tools: [MobileTimelineTool]
    @ObservedObject var model: MobileAppModel
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Button { withAnimation(.easeOut(duration: 0.18)) { expanded.toggle() } } label: {
                HStack(spacing: 8) {
                    Image(systemName: "doc.on.doc").font(.system(size: 12, weight: .medium))
                        .frame(width: 20, height: 20).background(OpenBitFunTheme.soft)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                    Text(model.localizedFormat("已完成 %lld 项读取与搜索", Int64(tools.count)))
                        .font(MobileDesignTypography.bodySmall.font)
                    Spacer()
                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 10, weight: .semibold))
                }
                .foregroundStyle(OpenBitFunTheme.muted).frame(minHeight: 32)
            }
            .buttonStyle(.plain)
            if expanded { ForEach(tools) { ToolStatusRow(tool: $0, model: model) } }
        }
    }
}

private struct ToolStatusRow: View {
    let tool: MobileTimelineTool
    @ObservedObject var model: MobileAppModel
    @State private var expanded = false
    @State private var answer = ""
    @State private var selectedOptions: [Int: Set<String>] = [:]
    @State private var otherAnswers: [Int: String] = [:]

    private var emphasized: Bool { !tool.actions.isEmpty || expanded || tool.phase == "FAILED" }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                if !tool.input.isEmpty || !tool.output.isEmpty || !tool.filePath.isEmpty {
                    withAnimation(.easeOut(duration: 0.18)) { expanded.toggle() }
                }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: toolSymbol).font(.system(size: 12, weight: .medium))
                        .foregroundStyle(statusColor).frame(width: 20, height: 20)
                        .background(statusColor.opacity(0.09)).clipShape(RoundedRectangle(cornerRadius: 6))
                    Text(statusLabel).font(MobileDesignTypography.bodySmall.font)
                        .foregroundStyle(OpenBitFunTheme.ink).lineLimit(1)
                    Spacer(minLength: 4)
                    if tool.phase == "RUNNING" { ProgressView().controlSize(.mini) }
                    else { Text(statusMark).font(.system(size: 11, weight: .semibold)).foregroundStyle(statusColor) }
                }
                .frame(minHeight: 32)
            }
            .buttonStyle(.plain)

            if expanded {
                if !tool.filePath.isEmpty {
                    Button { model.openRemoteFile(reference: tool.filePath, label: tool.fileLabel) } label: {
                        Label(tool.fileLabel.isEmpty ? tool.filePath : tool.fileLabel, systemImage: "doc.text")
                            .font(MobileDesignTypography.labelSmall.font).foregroundStyle(MobileDesignColors.fileLink)
                    }
                    .buttonStyle(.plain)
                }
                if !tool.input.isEmpty { detailText(model.localized("输入"), tool.input) }
                if !tool.output.isEmpty { detailText(model.localized("输出"), tool.output) }
            }

            if tool.actions.contains("ANSWER") {
                if tool.questions.isEmpty {
                    legacyAnswerPanel
                } else {
                    structuredAnswerPanel
                }
            } else if tool.actions.contains("APPROVE") || tool.actions.contains("REJECT") {
                HStack(spacing: 8) {
                    if tool.actions.contains("REJECT") { toolAction(model.localized("拒绝"), primary: false) { model.rejectTool(tool.id) } }
                    if tool.actions.contains("APPROVE") { toolAction(model.localized("允许"), primary: true) { model.approveTool(tool.id) } }
                }
            }
            if tool.actions.contains("CANCEL") {
                Button { model.cancelTool(tool.id) } label: {
                    Text(model.localized("停止执行"))
                        .font(MobileDesignTypography.labelMedium.font).foregroundStyle(OpenBitFunTheme.statusDanger)
                        .frame(maxWidth: .infinity, minHeight: 40).background(OpenBitFunTheme.card).clipShape(Capsule())
                        .overlay(Capsule().stroke(OpenBitFunTheme.statusDanger.opacity(0.5), lineWidth: 1))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(emphasized ? 10 : 0).background(emphasized ? OpenBitFunTheme.soft : OpenBitFunTheme.transparent)
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay { if emphasized { RoundedRectangle(cornerRadius: 14).stroke(OpenBitFunTheme.line, lineWidth: 1) } }
    }

    private var legacyAnswerPanel: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(tool.question ?? model.localized("请输入回复")).font(MobileDesignTypography.bodySmall.font)
                .foregroundStyle(OpenBitFunTheme.ink)
            TextField(model.localized("回复"), text: $answer, axis: .vertical)
                .font(MobileDesignTypography.bodyMedium.font).lineLimit(2...5).padding(10)
                .background(OpenBitFunTheme.card).clipShape(RoundedRectangle(cornerRadius: 11))
                .overlay(RoundedRectangle(cornerRadius: 11).stroke(OpenBitFunTheme.line, lineWidth: 1))
            Button { model.answerTool(tool.id, answer: answer); answer = "" } label: {
                Text(model.localized("发送回复"))
                    .font(MobileDesignTypography.labelMedium.font).foregroundStyle(OpenBitFunTheme.contentOnAction)
                    .frame(maxWidth: .infinity, minHeight: 40)
                    .background(answer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.busy ? OpenBitFunTheme.muted : OpenBitFunTheme.accent)
                    .clipShape(Capsule())
            }
            .buttonStyle(.plain).disabled(answer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.busy)
        }
    }

    private var structuredAnswerPanel: some View {
        VStack(alignment: .leading, spacing: 13) {
            ForEach(tool.questions) { question in
                VStack(alignment: .leading, spacing: 7) {
                    if !question.header.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text(question.header).font(MobileDesignTypography.labelMedium.font).foregroundStyle(OpenBitFunTheme.muted)
                    }
                    Text(question.question).font(MobileDesignTypography.bodySmall.font).foregroundStyle(OpenBitFunTheme.ink)
                    ForEach(options(for: question)) { option in
                        let selected = selectedOptions[question.index, default: []].contains(option.label)
                        Button { toggle(option.label, for: question) } label: {
                            HStack(alignment: .top, spacing: 9) {
                                Image(systemName: selected ? (question.multiSelect ? "checkmark.square.fill" : "largecircle.fill.circle") : (question.multiSelect ? "square" : "circle"))
                                    .foregroundStyle(selected ? OpenBitFunTheme.accent : OpenBitFunTheme.muted)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(option.label).font(MobileDesignTypography.bodySmall.font).foregroundStyle(OpenBitFunTheme.ink)
                                    if let description = option.description, !description.isEmpty {
                                        Text(description).font(MobileDesignTypography.labelSmall.font).foregroundStyle(OpenBitFunTheme.muted)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.vertical, 5)
                        }
                        .buttonStyle(.plain)
                        .disabled(model.busy)
                        if selected && isOther(option) {
                            TextField(model.localized("请输入回复"), text: Binding(
                                get: { otherAnswers[question.index, default: ""] },
                                set: { otherAnswers[question.index] = $0 }
                            ))
                            .font(MobileDesignTypography.bodySmall.font).padding(9)
                            .disabled(model.busy)
                            .background(OpenBitFunTheme.card).clipShape(RoundedRectangle(cornerRadius: 9))
                            .overlay(RoundedRectangle(cornerRadius: 9).stroke(OpenBitFunTheme.line, lineWidth: 1))
                        }
                    }
                }
            }
            Button { submitStructuredAnswers() } label: {
                HStack {
                    if model.busy { ProgressView().controlSize(.small).tint(OpenBitFunTheme.contentOnAction) }
                    Text(model.localized("发送回复"))
                }
                .font(MobileDesignTypography.labelMedium.font).foregroundStyle(OpenBitFunTheme.contentOnAction)
                .frame(maxWidth: .infinity, minHeight: 40)
                .background(structuredAnswersValid && !model.busy ? OpenBitFunTheme.accent : OpenBitFunTheme.muted)
                .clipShape(Capsule())
            }
            .buttonStyle(.plain).disabled(!structuredAnswersValid || model.busy)
        }
    }

    private func options(for question: MobileTimelineQuestion) -> [MobileTimelineOption] {
        question.options.contains(where: isOther) ? question.options : question.options + [MobileTimelineOption(label: model.localized("其他"), description: nil)]
    }

    private func isOther(_ option: MobileTimelineOption) -> Bool {
        let normalized = option.label.trimmingCharacters(in: .whitespacesAndNewlines)
        let localizedOther = model.localized("其他").trimmingCharacters(in: .whitespacesAndNewlines)
        return normalized.lowercased() == "other" || normalized == "其他" || normalized == localizedOther
    }

    private func toggle(_ label: String, for question: MobileTimelineQuestion) {
        guard !model.busy else { return }
        if question.multiSelect {
            if selectedOptions[question.index, default: []].contains(label) {
                selectedOptions[question.index]?.remove(label)
            } else {
                selectedOptions[question.index, default: []].insert(label)
            }
        } else {
            selectedOptions[question.index] = [label]
        }
    }

    private var structuredAnswersValid: Bool {
        tool.questions.allSatisfy { question in
            let selected = selectedOptions[question.index, default: []]
            guard !selected.isEmpty else { return false }
            return !selected.contains(where: { label in
                isOther(MobileTimelineOption(label: label, description: nil)) &&
                    otherAnswers[question.index, default: ""].trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            })
        }
    }

    private func submitStructuredAnswers() {
        let answers = tool.questions.map { question in
            let selected = selectedOptions[question.index, default: []]
            let values = selected.map { label in
                isOther(MobileTimelineOption(label: label, description: nil))
                    ? otherAnswers[question.index, default: ""].trimmingCharacters(in: .whitespacesAndNewlines)
                    : label
            }
            let value: QuestionAnswerValue = question.multiSelect
                ? QuestionAnswerValueChoice(values: values)
                : QuestionAnswerValueText(text: values[0])
            return QuestionAnswer(index: Int32(question.index), value: value)
        }
        model.answerTool(tool.id, answers: answers)
    }

    @ViewBuilder
    private func detailText(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(MobileDesignTypography.labelSmall.font).foregroundStyle(OpenBitFunTheme.muted)
            Text(value).font(.system(size: 12.5, design: .monospaced)).foregroundStyle(OpenBitFunTheme.ink)
                .lineLimit(5).textSelection(.enabled)
        }
    }

    private func toolAction(_ label: String, primary: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label).font(MobileDesignTypography.labelMedium.font)
                .foregroundStyle(primary ? OpenBitFunTheme.contentOnAction : OpenBitFunTheme.ink)
                .frame(maxWidth: .infinity, minHeight: 40).background(primary ? OpenBitFunTheme.accent : OpenBitFunTheme.card)
                .clipShape(Capsule()).overlay { if !primary { Capsule().stroke(OpenBitFunTheme.line, lineWidth: 1) } }
        }
        .buttonStyle(.plain)
    }

    private var operationLabel: String {
        switch tool.operation {
        case "UPDATE_TODOS": model.localized("更新待办")
        case "START_TASK": model.localized("启动子任务")
        case "READ_FILE": model.localized("读取文件")
        case "WRITE_FILE": model.localized("写入文件")
        case "DELETE_FILE": model.localized("删除文件")
        case "VIEW_DIFF": model.localized("查看差异")
        case "EDIT_FILE": model.localized("编辑文件")
        case "RUN_COMMAND": model.localized("运行命令")
        case "SEARCH_WEB": model.localized("搜索网页")
        case "OPEN_WEB": model.localized("打开网页")
        case "SEARCH_CODE": model.localized("搜索代码")
        case "ASK_CONFIRMATION": model.localized("请求确认")
        default: tool.name.isEmpty ? model.localized("工具") : tool.name
        }
    }

    private var statusLabel: String {
        let target = tool.target.isEmpty ? "" : " · \(tool.target)"
        return switch tool.phase {
        case "RUNNING": model.localizedFormat("正在%@%@", operationLabel, target)
        case "FAILED": model.localizedFormat("%@失败%@", operationLabel, target)
        case "PENDING_CONFIRMATION": model.localizedFormat("等待确认 · %@", operationLabel)
        case "WAITING": model.localizedFormat("等待执行 · %@", operationLabel)
        default: "\(operationLabel)\(target)"
        }
    }

    private var toolSymbol: String {
        switch tool.kind {
        case "QUESTION": "questionmark.circle"
        case "TODO": "checklist"
        case "TASK": "person.2"
        case "GIT": "arrow.triangle.branch"
        case "DELETE": "trash"
        case "DIFF": "doc.text.magnifyingglass"
        case "PATCH", "COMMAND": "terminal"
        case "CREATE": "doc.badge.plus"
        case "MUTATE": "square.and.pencil"
        case "FOLDER": "folder"
        case "DOCUMENT": "doc.text"
        case "SEARCH": "magnifyingglass"
        case "WEB": "link"
        default: "wrench.and.screwdriver"
        }
    }

    private var statusColor: Color {
        switch tool.phase { case "FAILED": OpenBitFunTheme.statusDanger; case "COMPLETED": OpenBitFunTheme.statusSuccess; default: OpenBitFunTheme.muted }
    }

    private var statusMark: String {
        switch tool.phase { case "FAILED": "!"; case "PENDING_CONFIRMATION": "?"; case "CANCELLED": "×"; case "COMPLETED": "✓"; default: "•" }
    }
}
