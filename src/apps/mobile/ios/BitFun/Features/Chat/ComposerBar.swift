import AVFoundation
import PhotosUI
import Speech
import SwiftUI
import UniformTypeIdentifiers

struct ComposerBar: View {
    @ObservedObject var model: MobileAppModel
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @FocusState private var focused: Bool
    @StateObject private var speech = SpeechInputController()
    @State private var pickerItems: [PhotosPickerItem] = []
    @State private var modelSelectorOpen = ProcessInfo.processInfo.arguments.contains(
        "--composer-model-picker"
    ) || ProcessInfo.processInfo.environment["BITFUN_COMPOSER_MODEL_PICKER"] == "1"

    private var placeholder: String {
        model.localized(model.surface == .remote
            ? "向 BitFun 提问"
            : (model.localSessionSelected ? "输入消息" : "问问 BitFun"))
    }

    private var expanded: Bool {
        focused || modelSelectorOpen || model.draft.contains("\n")
    }

    private var hasContent: Bool {
        !model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !model.composerImages.isEmpty
    }

    private var canSend: Bool {
        hasContent && !model.busy && !model.isSending &&
            (model.surface == .local || model.connectionPhase != .disconnected)
    }

    var body: some View {
        VStack(spacing: 2) {
            if !model.composerImages.isEmpty {
                attachmentStrip
            }

            if expanded {
                expandedInputRow
                expandedActionRow
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            } else {
                collapsedRow
            }
        }
        .padding(.horizontal, 8)
        .padding(.top, expanded ? 4 : 0)
        .padding(.bottom, expanded ? 2 : 0)
        .frame(minHeight: expanded
            ? MobileDesignGeometry.composerExpandedHeight
            : MobileDesignGeometry.composerCollapsedHeight)
        .background(BitFunTheme.card)
        .clipShape(
            RoundedRectangle(
                cornerRadius: expanded || !model.composerImages.isEmpty
                    ? MobileDesignGeometry.composerExpandedRadius
                    : MobileDesignGeometry.composerCollapsedRadius
            )
        )
        .shadow(color: BitFunTheme.shadowSubtle, radius: 10, y: 2)
        .padding(.horizontal, MobileDesignGeometry.contentGutter)
        .padding(.top, 8)
        .padding(.bottom, 14)
        .background(BitFunTheme.page)
        .animation(.easeOut(duration: 0.22), value: expanded)
        .animation(.easeOut(duration: 0.18), value: model.composerImages.count)
        .onAppear {
            if model.composerModelPickerPreview {
                modelSelectorOpen = true
            }
        }
        .onChange(of: pickerItems) { items in
            guard !items.isEmpty else { return }
            Task { await importPickedImages(items) }
        }
        .overlayPreferenceValue(ComposerModelSelectorAnchorKey.self) { anchor in
            GeometryReader { proxy in
                if horizontalSizeClass == .regular, modelSelectorOpen, let anchor {
                    let frame = proxy[anchor]
                    modelSelector(asSheet: false)
                        .frame(
                            width: MobileDesignGeometry.composerModelSelectorWidth,
                            height: modelSelectorHeight(asSheet: false)
                        )
                        .background(MobileDesignColors.floatingPanelBg)
                        .clipShape(
                            RoundedRectangle(
                                cornerRadius: MobileDesignGeometry.composerModelSelectorRadius
                            )
                        )
                        .overlay(
                            RoundedRectangle(
                                cornerRadius: MobileDesignGeometry.composerModelSelectorRadius
                            )
                                .stroke(BitFunTheme.line, lineWidth: 1)
                        )
                        .shadow(color: BitFunTheme.line, radius: 20, y: 8)
                        .position(
                            x: min(
                                max(
                                    MobileDesignGeometry.composerModelSelectorWidth / 2 + 8,
                                    frame.midX
                                ),
                                proxy.size.width -
                                    MobileDesignGeometry.composerModelSelectorWidth / 2 - 8
                            ),
                            y: frame.minY - modelSelectorHeight(asSheet: false) / 2 - 8
                        )
                        .transition(.opacity.combined(with: .scale(scale: 0.96, anchor: .bottom)))
                        .zIndex(20)
                }
            }
            .allowsHitTesting(horizontalSizeClass == .regular && modelSelectorOpen)
        }
        .sheet(
            isPresented: Binding(
                get: { horizontalSizeClass != .regular && modelSelectorOpen },
                set: { if !$0 { modelSelectorOpen = false } }
            )
        ) {
            modelSelector(asSheet: true)
                .presentationDetents([.height(modelSelectorHeight(asSheet: true))])
                .presentationDragIndicator(.visible)
        }
    }

    private var collapsedRow: some View {
        HStack(spacing: 5) {
            attachmentAction
            inputField(maxLines: 1)
                .frame(height: MobileDesignGeometry.composerInputHeight)
            primaryAction
        }
        .frame(height: MobileDesignGeometry.composerCollapsedHeight)
    }

    private var expandedInputRow: some View {
        HStack(spacing: 0) {
            inputField(maxLines: 4)
                .frame(height: MobileDesignGeometry.composerExpandedInputHeight)
        }
        .frame(height: MobileDesignGeometry.composerExpandedInputRowHeight)
    }

    private var expandedActionRow: some View {
        HStack(spacing: 6) {
            attachmentAction
            if !model.modelOptions.isEmpty {
                Button { modelSelectorOpen = true } label: {
                    HStack(spacing: 3) {
                        Text(selectedModel?.primaryLabel ?? model.localized("模型"))
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(BitFunTheme.ink)
                            .lineLimit(1)
                        Image(systemName: "chevron.down")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(BitFunTheme.muted)
                    }
                    .frame(height: 34)
                    .padding(.horizontal, 4)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("选择模型")))
                .anchorPreference(key: ComposerModelSelectorAnchorKey.self, value: .bounds) { $0 }
            }
            Spacer(minLength: 0)
            primaryAction
        }
        .frame(height: MobileDesignGeometry.composerExpandedActionRowHeight)
        .padding(.leading, 2)
    }

    private func inputField(maxLines: Int) -> some View {
        HStack(spacing: speech.isListening ? 8 : 0) {
            if speech.isListening {
                ListeningWave()
            }
            TextField(
                "",
                text: $model.draft,
                prompt: Text(speech.isListening ? model.localized("正在聆听") : placeholder)
                    .foregroundColor(speech.isListening ? BitFunTheme.statusSuccess : BitFunTheme.muted),
                axis: .vertical
            )
            .font(MobileDesignTypography.bodyLarge.font)
            .foregroundStyle(BitFunTheme.ink)
            .lineLimit(1...maxLines)
            .focused($focused)
            .submitLabel(.send)
            .onSubmit {
                if canSend { model.send() }
            }
            .onChange(of: model.draft) { _ in model.syncDraftToCore() }
        }
        .padding(.leading, speech.isListening ? 12 : 4)
        .padding(.trailing, 4)
        .background(speech.isListening ? BitFunTheme.soft : BitFunTheme.transparent)
        .overlay(
            RoundedRectangle(cornerRadius: 20)
                .stroke(speech.isListening ? BitFunTheme.statusSuccess : BitFunTheme.transparent, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 20))
    }

    @ViewBuilder
    private var attachmentAction: some View {
        if model.composerImages.count < 4 {
            PhotosPicker(
                selection: $pickerItems,
                maxSelectionCount: 4 - model.composerImages.count,
                matching: .images
            ) {
                plusGlyph
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text(model.localized("添加图片")))
        } else {
            Button { model.showToast(model.localized("最多添加 4 张图片")) } label: { plusGlyph }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(model.localized("已达到图片上限")))
        }
    }

    private var plusGlyph: some View {
        ReferenceGlyph(assetName: "ComposerPlusGlyph", width: 18, height: 18)
            .frame(
                width: MobileDesignGeometry.composerActionSize,
                height: MobileDesignGeometry.composerActionSize
            )
    }

    private var primaryAction: some View {
        Button(action: performPrimaryAction) {
            Group {
                if model.isSending {
                    Image(systemName: "stop.fill")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(BitFunTheme.accent)
                } else if hasContent {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(canSend ? BitFunTheme.accent : BitFunTheme.muted)
                } else {
                    ReferenceGlyph(assetName: "ComposerMicGlyph", width: 16, height: 19)
                        .foregroundStyle(model.busy ? BitFunTheme.muted.opacity(0.45) : BitFunTheme.muted)
                }
            }
            .frame(
                width: MobileDesignGeometry.composerActionSize,
                height: MobileDesignGeometry.composerActionSize
            )
        }
        .buttonStyle(.plain)
        .disabled(!model.isSending && hasContent && !canSend)
        .accessibilityLabel(primaryActionLabel)
    }

    private var primaryActionLabel: String {
        if model.isSending { return model.localized("停止") }
        if hasContent { return model.localized("发送") }
        return model.localized(speech.isListening ? "停止听写" : "语音输入")
    }

    private var attachmentStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(model.composerImages) { attachment in
                    ZStack(alignment: .topTrailing) {
                        Group {
                            if let image = UIImage(data: attachment.data) {
                                Image(uiImage: image)
                                    .resizable()
                                    .scaledToFill()
                            } else {
                                Image(systemName: "photo")
                                    .foregroundStyle(BitFunTheme.muted)
                            }
                        }
                        .frame(width: 64, height: 64)
                        .background(BitFunTheme.soft)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(BitFunTheme.line, lineWidth: 1)
                        )

                        Button { model.removeComposerImage(id: attachment.id) } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 9, weight: .bold))
                                .foregroundStyle(BitFunTheme.contentOnAction)
                                .frame(width: 20, height: 20)
                                .background(BitFunTheme.mediaScrim)
                                .clipShape(Circle())
                        }
                        .buttonStyle(.plain)
                        .offset(x: 5, y: -5)
                        .accessibilityLabel(Text(model.localized("移除图片")))
                    }
                    .padding(.top, 6)
                }
            }
            .padding(.horizontal, 2)
        }
        .frame(height: 72)
    }

    @ViewBuilder
    private func modelSelector(asSheet: Bool) -> some View {
        VStack(spacing: 10) {
            if asSheet {
                HStack(spacing: 0) {
                    Text(model.localized("选择模型"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(BitFunTheme.muted)
                    Spacer(minLength: 0)
                    Button { modelSelectorOpen = false } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 15, weight: .regular))
                            .foregroundStyle(BitFunTheme.muted)
                            .frame(
                                width: MobileDesignGeometry.selectionCloseSize,
                                height: MobileDesignGeometry.selectionCloseSize
                            )
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text(model.localized("关闭")))
                }
                .frame(height: MobileDesignGeometry.selectionCloseSize)
            }

            ScrollView(showsIndicators: false) {
                LazyVStack(spacing: MobileDesignGeometry.composerModelSelectorRowGap) {
                    ForEach(selectorModels) { option in
                        Button {
                            model.selectModel(option.id)
                            modelSelectorOpen = false
                        } label: {
                            HStack(spacing: 10) {
                                Image(systemName: option.selected ? "checkmark.circle" : "circle")
                                    .font(.system(size: 16))
                                    .foregroundStyle(option.selected ? BitFunTheme.ink : BitFunTheme.transparent)
                                    .frame(width: 20, height: 20)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(option.primaryLabel)
                                        .font(.system(size: 13, weight: .medium))
                                        .foregroundStyle(BitFunTheme.ink)
                                        .lineLimit(1)
                                    Text(option.secondaryLabel)
                                        .font(.system(size: 11))
                                        .foregroundStyle(BitFunTheme.muted)
                                        .lineLimit(1)
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 10)
                            .frame(height: MobileDesignGeometry.composerModelSelectorRowHeight)
                            .background(option.selected ? BitFunTheme.soft : BitFunTheme.transparent)
                            .clipShape(
                                RoundedRectangle(
                                    cornerRadius: MobileDesignGeometry.composerModelSelectorRowRadius
                                )
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .padding(10)
        .background(asSheet ? BitFunTheme.card : MobileDesignColors.floatingPanelBg)
    }

    private var selectedModel: ComposerModelOption? {
        model.modelOptions.first(where: \.selected) ?? model.modelOptions.first
    }

    private var selectorModels: [ComposerModelOption] {
        model.modelOptions.filter(\.selected) + model.modelOptions.filter { !$0.selected }
    }

    private func modelSelectorHeight(asSheet: Bool) -> CGFloat {
        let visibleRows = min(model.modelOptions.count, 7)
        let listHeight = CGFloat(visibleRows) *
            MobileDesignGeometry.composerModelSelectorRowHeight +
            CGFloat(max(0, visibleRows - 1)) *
            MobileDesignGeometry.composerModelSelectorRowGap
        return min(480, listHeight + (asSheet ? 86 : 20))
    }

    private func performPrimaryAction() {
        if model.isSending {
            model.stopSending()
            return
        }
        if hasContent {
            if canSend { model.send() }
            return
        }
        if speech.isListening {
            speech.stop()
            return
        }
        guard !model.busy else { return }
        let existing = model.draft.trimmingCharacters(in: .whitespacesAndNewlines)
        speech.start(
            localeIdentifier: model.appLanguage == .simplifiedChinese ? "zh-CN" : "en-US",
            onPartial: { transcript in
                model.draft = [existing, transcript]
                    .filter { !$0.isEmpty }
                    .joined(separator: existing.isEmpty ? "" : " ")
                model.syncDraftToCore()
            },
            onFailure: { message in model.showToast(model.localized(message)) }
        )
    }

    private func importPickedImages(_ items: [PhotosPickerItem]) async {
        for item in items {
            guard let data = try? await item.loadTransferable(type: Data.self) else {
                model.showToast(model.localized("无法读取所选图片"))
                continue
            }
            let mimeType = item.supportedContentTypes
                .compactMap(\.preferredMIMEType)
                .first ?? "image/jpeg"
            model.addComposerImage(data: data, mimeType: mimeType)
        }
        pickerItems = []
    }
}

private struct ComposerModelSelectorAnchorKey: PreferenceKey {
    static var defaultValue: Anchor<CGRect>?

    static func reduce(value: inout Anchor<CGRect>?, nextValue: () -> Anchor<CGRect>?) {
        value = nextValue() ?? value
    }
}

private struct ListeningWave: View {
    var body: some View {
        HStack(spacing: 2) {
            ForEach([8.0, 14.0, 10.0, 17.0], id: \.self) { height in
                Capsule()
                    .fill(BitFunTheme.statusSuccess)
                    .frame(width: 2, height: height)
            }
        }
        .frame(width: 18, height: 22)
        .accessibilityHidden(true)
    }
}

final class SpeechInputController: ObservableObject {
    @Published private(set) var isListening = false

    private var recognizer: SFSpeechRecognizer?
    private let audioEngine = AVAudioEngine()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var tapInstalled = false

    func start(
        localeIdentifier: String,
        onPartial: @escaping (String) -> Void,
        onFailure: @escaping (String) -> Void
    ) {
        recognizer = SFSpeechRecognizer(locale: Locale(identifier: localeIdentifier))
        SFSpeechRecognizer.requestAuthorization { [weak self] speechStatus in
            guard speechStatus == .authorized else {
                DispatchQueue.main.async { onFailure("请在系统设置中允许语音识别") }
                return
            }
            AVAudioSession.sharedInstance().requestRecordPermission { granted in
                guard granted else {
                    DispatchQueue.main.async { onFailure("请在系统设置中允许麦克风访问") }
                    return
                }
                DispatchQueue.main.async {
                    self?.beginRecognition(onPartial: onPartial, onFailure: onFailure)
                }
            }
        }
    }

    func stop() {
        recognitionTask?.finish()
        finishRecognition()
    }

    private func beginRecognition(
        onPartial: @escaping (String) -> Void,
        onFailure: @escaping (String) -> Void
    ) {
        guard let recognizer, recognizer.isAvailable else {
            onFailure("当前设备暂时无法使用语音识别")
            return
        }

        finishRecognition()
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true, options: .notifyOthersOnDeactivation)

            let request = SFSpeechAudioBufferRecognitionRequest()
            request.shouldReportPartialResults = true
            self.request = request

            let input = audioEngine.inputNode
            let format = input.outputFormat(forBus: 0)
            input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
                request.append(buffer)
            }
            tapInstalled = true
            audioEngine.prepare()
            try audioEngine.start()
            isListening = true

            recognitionTask = recognizer.recognitionTask(with: request) { [weak self] result, error in
                DispatchQueue.main.async {
                    if let text = result?.bestTranscription.formattedString, !text.isEmpty {
                        onPartial(text)
                    }
                    if result?.isFinal == true || error != nil {
                        if error != nil && result == nil { onFailure("语音识别已中断，请重试") }
                        self?.finishRecognition()
                    }
                }
            }
        } catch {
            finishRecognition()
            onFailure("无法启动语音输入，请检查麦克风")
        }
    }

    private func finishRecognition() {
        if audioEngine.isRunning {
            audioEngine.stop()
        }
        if tapInstalled {
            audioEngine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        request?.endAudio()
        request = nil
        recognitionTask?.cancel()
        recognitionTask = nil
        isListening = false
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}
