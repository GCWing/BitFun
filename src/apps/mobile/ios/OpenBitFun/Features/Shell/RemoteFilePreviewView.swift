import SwiftUI
import UniformTypeIdentifiers

private var filePreviewScrollAnchorByTarget: [String: Int] = [:]

private struct FilePreviewVisibleAnchor: Equatable {
    let id: Int
    let distanceFromTop: CGFloat
}

private struct MarkdownSourceBlock: Identifiable {
    let startLine: Int
    let endLine: Int
    let text: String
    var id: Int { startLine }
}

private func markdownSourceBlocks(_ content: String) -> [MarkdownSourceBlock] {
    let parts = content.components(separatedBy: "\n\n")
    var nextStart = 1
    return parts.map { text in
        let end = nextStart + text.reduce(0) { $0 + ($1 == "\n" ? 1 : 0) }
        defer { nextStart = end + 2 }
        return MarkdownSourceBlock(startLine: nextStart, endLine: max(nextStart, end), text: text)
    }
}

private func markdownAnchor(for lineStart: Int32, blocks: [MarkdownSourceBlock]) -> Int {
    guard !blocks.isEmpty else { return 1 }
    let requested = max(1, Int(lineStart))
    if let containing = blocks.first(where: { $0.startLine <= requested && requested <= $0.endLine }) {
        return containing.startLine
    }
    return blocks.last(where: { $0.startLine <= requested })?.startLine ?? blocks[0].startLine
}

private struct FilePreviewVisibleLinePreferenceKey: PreferenceKey {
    static var defaultValue: FilePreviewVisibleAnchor?
    static func reduce(value: inout FilePreviewVisibleAnchor?, nextValue: () -> FilePreviewVisibleAnchor?) {
        guard let next = nextValue() else { return }
        if value == nil || next.distanceFromTop < value!.distanceFromTop { value = next }
    }
}

struct MobileDownloadDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.data] }
    let data: Data

    init(data: Data) {
        self.data = data
    }

    init(configuration: ReadConfiguration) throws {
        data = configuration.file.regularFileContents ?? Data()
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}

struct RemoteFilePreviewSheet: View {
    @ObservedObject var model: MobileAppModel
    let preview: MobileFilePreview
    var embedded = false
    @Environment(\.dismiss) private var dismiss
    @State private var visibleLine: Int = 0

    private var scrollTargetKey: String {
        "\(preview.sessionID)|\(preview.controlTargetEpoch)|\(preview.id)"
    }

    private func formatBytes(_ value: Int64) -> String {
        if value < 1024 { return "\(value) B" }
        if value < 1024 * 1024 { return "\(Int((Double(value) / 1024).rounded())) KB" }
        return "\(Int((Double(value) / (1024 * 1024)).rounded())) MB"
    }

    private var metadataText: String {
        let type = preview.mimeType
        let size = preview.sizeBytes > 0 ? formatBytes(preview.sizeBytes) : ""
        if type.isEmpty { return size }
        if size.isEmpty { return type }
        return "\(type) · \(size)"
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: preview.imageData == nil ? "doc.text" : "photo")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(MobileDesignColors.fileLink)
                    .frame(width: 34, height: 34)
                    .background(MobileDesignColors.fileLink.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: 9))
                VStack(alignment: .leading, spacing: 2) {
                    Text(preview.name)
                        .font(MobileDesignTypography.titleSmall.font)
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .lineLimit(1)
                    if !preview.mimeType.isEmpty || preview.sizeBytes > 0 {
                        Text(metadataText)
                            .font(MobileDesignTypography.labelSmall.font)
                            .foregroundStyle(OpenBitFunTheme.muted)
                            .lineLimit(1)
                    }
                }
                Spacer()
                Button {
                    model.downloadRemoteFile(
                        reference: "computer://\(preview.id)",
                        label: preview.name
                    )
                } label: {
                    Image(systemName: "arrow.down.circle")
                        .font(.system(size: 18, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .frame(width: 36, height: 36)
                }
                .buttonStyle(.plain)
                .disabled([.preparing, .downloading, .saving].contains(model.downloadPhase))
                .opacity([.preparing, .downloading, .saving].contains(model.downloadPhase) ? 0.45 : 1)
                .accessibilityLabel(Text(model.localizedFormat("下载 %@", preview.name)))
                Button {
                    model.dismissFilePreview()
                    if !embedded { dismiss() }
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .frame(width: 36, height: 36)
                        .background(OpenBitFunTheme.soft)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("关闭文件预览")))
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)

            Rectangle().fill(OpenBitFunTheme.line).frame(height: 1)

            if let status = model.downloadStatus(for: preview.id), !status.isEmpty {
                HStack(spacing: 8) {
                    if [.preparing, .downloading, .saving].contains(model.downloadPhase) {
                        ProgressView().controlSize(.small)
                    } else if model.downloadPhase == .saved {
                        Image(systemName: "checkmark.circle")
                    } else if model.downloadPhase == .failed {
                        Image(systemName: "exclamationmark.triangle")
                            .foregroundStyle(OpenBitFunTheme.statusDanger)
                    }
                    Text(status).font(MobileDesignTypography.labelSmall.font)
                        .foregroundStyle(model.downloadPhase == .failed ? OpenBitFunTheme.statusDanger : OpenBitFunTheme.muted)
                        .lineLimit(2)
                    if model.downloadPhase == .failed {
                        Button(model.localized("重试")) { model.retryRemoteDownload() }
                            .buttonStyle(.bordered)
                            .disabled(!model.canRetryRemoteDownload)
                            .accessibilityLabel(Text(model.localized("重试下载")))
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 18).padding(.vertical, 8)
                .background(OpenBitFunTheme.soft)
            }

            Group {
                if model.filePreviewLoading {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text(model.localized("正在加载文件"))
                            .font(MobileDesignTypography.bodySmall.font)
                            .foregroundStyle(OpenBitFunTheme.muted)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if preview.unsupported {
                    VStack(spacing: 10) {
                        Image(systemName: "doc.badge.questionmark")
                            .font(.system(size: 28, weight: .medium))
                        Text(model.localized("此文件类型暂不支持预览"))
                            .font(MobileDesignTypography.titleSmall.font)
                        Text(preview.mimeType.isEmpty ? model.localized("此文件暂不支持预览") : preview.mimeType)
                            .font(MobileDesignTypography.bodySmall.font)
                            .multilineTextAlignment(.center)
                    }
                    .foregroundStyle(OpenBitFunTheme.muted).padding(24)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let failure = preview.failure {
                    VStack(spacing: 10) {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: 28, weight: .medium))
                        Text(model.localized("无法预览"))
                            .font(MobileDesignTypography.titleSmall.font)
                        Text(failure).font(MobileDesignTypography.bodySmall.font)
                            .multilineTextAlignment(.center)
                        if preview.retryable {
                            Button(model.localized("重试")) {
                                model.openRemoteFile(reference: preview.id, label: preview.name)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(MobileDesignColors.fileLink)
                            .accessibilityLabel(Text(model.localized("重试文件预览")))
                        }
                    }
                    .foregroundStyle(OpenBitFunTheme.muted).padding(24)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let data = preview.imageData, let image = UIImage(data: data) {
                    ScrollView([.horizontal, .vertical], showsIndicators: false) {
                        Image(uiImage: image)
                            .resizable()
                            .scaledToFit()
                            .padding(18)
                    }
                } else {
                    ScrollView(showsIndicators: false) {
                        if preview.markdown {
                            ScrollViewReader { proxy in
                                LazyVStack(alignment: .leading, spacing: 12) {
                                    let blocks = markdownSourceBlocks(preview.content)
                                    ForEach(blocks) { block in
                                        MarkdownMessageView(text: block.text, model: model)
                                            .id(block.startLine)
                                            .background(GeometryReader { geometry in
                                                OpenBitFunTheme.transparent.preference(
                                                    key: FilePreviewVisibleLinePreferenceKey.self,
                                                    value: {
                                                        let frame = geometry.frame(in: .named("file-preview-scroll"))
                                                        guard frame.minY <= 0, frame.maxY >= 0 else { return nil }
                                                        return FilePreviewVisibleAnchor(id: block.startLine, distanceFromTop: abs(frame.minY))
                                                    }()
                                                )
                                            })
                                    }
                                }
                                .padding(18)
                                .onAppear {
                                    let blocks = markdownSourceBlocks(preview.content)
                                    let anchor = filePreviewScrollAnchorByTarget[scrollTargetKey]
                                        ?? markdownAnchor(for: preview.lineStart, blocks: blocks)
                                    proxy.scrollTo(anchor, anchor: .center)
                                }
                                .onDisappear {
                                    if visibleLine > 0 { filePreviewScrollAnchorByTarget[scrollTargetKey] = visibleLine }
                                }
                            }
                        } else {
                            ScrollViewReader { proxy in
                                LazyVStack(alignment: .leading, spacing: 0) {
                                    ForEach(Array(preview.content.split(separator: "\n", omittingEmptySubsequences: false).enumerated()), id: \.offset) { index, line in
                                        Text(String(line))
                                            .font(.system(size: 13, design: .monospaced))
                                            .foregroundStyle(OpenBitFunTheme.ink)
                                            .frame(maxWidth: .infinity, alignment: .leading)
                                            .background(GeometryReader { geometry in
                                                OpenBitFunTheme.transparent.preference(
                                                    key: FilePreviewVisibleLinePreferenceKey.self,
                                                    value: {
                                                        let frame = geometry.frame(in: .named("file-preview-scroll"))
                                                        guard frame.minY <= 0, frame.maxY >= 0 else { return nil }
                                                        return FilePreviewVisibleAnchor(id: index + 1, distanceFromTop: abs(frame.minY))
                                                    }()
                                                )
                                            })
                                            .id(index + 1)
                                    }
                                }
                                .padding(18).textSelection(.enabled)
                                .onAppear {
                                    let anchor = filePreviewScrollAnchorByTarget[scrollTargetKey] ??
                                        (preview.lineStart > 1 ? Int(preview.lineStart) : 1)
                                    proxy.scrollTo(anchor, anchor: .center)
                                }
                                .onDisappear {
                                    if visibleLine > 0 { filePreviewScrollAnchorByTarget[scrollTargetKey] = visibleLine }
                                }
                            }
                        }
                    }
                    .coordinateSpace(name: "file-preview-scroll")
                    .onPreferenceChange(FilePreviewVisibleLinePreferenceKey.self) { anchor in
                        guard let anchor else { return }
                        visibleLine = anchor.id
                        filePreviewScrollAnchorByTarget[scrollTargetKey] = anchor.id
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            if preview.truncated {
                Text(model.localized("文件较大，当前仅显示部分内容"))
                    .font(MobileDesignTypography.labelSmall.font)
                    .foregroundStyle(OpenBitFunTheme.muted)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(OpenBitFunTheme.soft)
            }
        }
        .background(OpenBitFunTheme.page)
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
    }
}
