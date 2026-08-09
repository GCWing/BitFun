import { RemoteI18n } from '../../i18n/RemoteI18n';
import {
  CodeSyntaxHighlightCache,
  CodeSyntaxHighlighter,
  CodeSyntaxToken,
  CodeSyntaxTokenKind
} from '../../services/CodeSyntaxHighlighter';
import { RemoteUiState } from '../../services/RemoteUiState';
import {
  FilePreviewPhase,
  FilePreviewRendererKind,
  FilePreviewState
} from '../state/FilePreviewState';
import { MarkdownContent } from './MarkdownContent';
import {
  CARD,
  CODE_COMMENT,
  CODE_CONSTANT,
  CODE_FUNCTION,
  CODE_KEYWORD,
  CODE_LINE_NUMBER,
  CODE_NUMBER,
  CODE_PROPERTY,
  CODE_STRING,
  CODE_TARGET_BG,
  CODE_TYPE,
  INK,
  LINE,
  MUTED,
  PAGE_BG,
  SOFT
} from './Theme';

@ComponentV2
export struct FilePreviewSurface {
  private readonly textScroller: Scroller = new Scroller();
  private readonly markdownScroller: Scroller = new Scroller();
  private readonly syntaxHighlightCache: CodeSyntaxHighlightCache = new CodeSyntaxHighlightCache();
  private restoreScrollTimerId: number = 0;
  @Local imageOriginalSizeTarget: string = '';
  @Param state: FilePreviewState = new FilePreviewState();
  @Param remoteAvailable: boolean = true;
  @Param downloadPath: string = '';
  @Param downloadedPath: string = '';
  @Param downloadStatus: string = '';
  @Event onClose: () => void = () => {};
  @Event onRefresh: () => void = () => {};
  @Event onDownload: (path: string) => void = (_path: string) => {};
  @Event onOpenLink: (reference: string, label: string) => void = (_reference: string, _label: string) => {};

  aboutToDisappear(): void {
    this.clearRestoreScrollTimer();
  }

  build() {
    Column() {
      this.Header()
      this.DownloadStatus()
      this.Body()
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  Header() {
    Row({ space: 10 }) {
      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.chevron_left'))
          .fontSize(22)
          .fontColor([INK])
      }
      .width(44)
      .height(44)
      .accessibilityText(RemoteI18n.t('common.close'))
      .onClick(() => {
        this.onClose();
      })

      Column({ space: 2 }) {
        Text(this.state.fileName || this.state.target.displayName || RemoteI18n.t('filePreview.title'))
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.headerDetail())
          .fontSize(11)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)

      if (this.isImageReady()) {
        Stack({ alignContent: Alignment.Center }) {
          Text('1:1')
            .fontSize(12)
            .fontWeight(FontWeight.Medium)
            .fontColor(this.isImageOriginalSize() ? CODE_FUNCTION : INK)
        }
        .width(44)
        .height(44)
        .accessibilityText(RemoteI18n.t(
          this.isImageOriginalSize() ? 'filePreview.fitImage' : 'filePreview.actualImageSize'
        ))
        .onClick(() => {
          this.toggleImageSize();
        })
      }

      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.arrow_clockwise'))
          .fontSize(20)
          .fontColor([this.canUseRemoteAction() ? INK : MUTED])
      }
      .width(44)
      .height(44)
      .accessibilityText(RemoteI18n.t('common.refresh'))
      .opacity(this.canUseRemoteAction() ? 1 : 0.45)
      .onClick(() => {
        if (this.canUseRemoteAction()) {
          this.onRefresh();
        }
      })

      Stack({ alignContent: Alignment.Center }) {
        SymbolGlyph($r('sys.symbol.arrow_down_to_line'))
          .fontSize(20)
          .fontColor([this.canDownload() ? INK : MUTED])
      }
      .width(44)
      .height(44)
      .accessibilityText(RemoteI18n.t('chat.download'))
      .opacity(this.canDownload() ? 1 : 0.45)
      .onClick(() => {
        if (this.canDownload()) {
          this.onDownload(this.state.target.rawReference || this.state.target.remotePath);
        }
      })
    }
    .width('100%')
    .height(68)
    .padding({ left: 8, right: 8, top: 8, bottom: 8 })
    .alignItems(VerticalAlign.Center)
    .border({ width: { bottom: 1 }, color: LINE })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  Body() {
    if (this.state.phase === FilePreviewPhase.Loading && !this.remoteAvailable) {
      this.OfflineLoadingState()
    } else if (this.state.phase === FilePreviewPhase.Loading) {
      this.LoadingState()
    } else if (this.state.phase === FilePreviewPhase.Error) {
      this.ErrorState()
    } else if (this.state.phase === FilePreviewPhase.Unsupported) {
      this.UnsupportedState()
    } else if (this.state.phase === FilePreviewPhase.Ready &&
      this.state.rendererKind === FilePreviewRendererKind.Image) {
      this.ImagePreview()
    } else if (this.state.phase === FilePreviewPhase.Ready &&
      this.state.rendererKind === FilePreviewRendererKind.Markdown) {
      this.MarkdownPreview()
    } else if (this.state.phase === FilePreviewPhase.Ready) {
      this.TextPreview()
    } else {
      this.EmptyState()
    }
  }

  @Builder
  DownloadStatus() {
    if (this.downloadStatus.length > 0 && this.downloadMatchesTarget()) {
      Row() {
        Text(this.downloadStatus)
          .width('100%')
          .fontSize(12)
          .fontColor(MUTED)
          .maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%')
      .padding({ left: 14, right: 14, top: 8, bottom: 8 })
      .backgroundColor(SOFT)
      .border({ width: { bottom: 1 }, color: LINE })
    }
  }

  @Builder
  LoadingState() {
    Column({ space: 12 }) {
      LoadingProgress()
        .width(28)
        .height(28)
        .color(MUTED)
      Text(RemoteI18n.t('filePreview.loading'))
        .fontSize(14)
        .fontColor(MUTED)
    }
    .width('100%')
    .layoutWeight(1)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  OfflineLoadingState() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('filePreview.loadFailed'))
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text(RemoteI18n.t('filePreview.offline'))
        .fontSize(13)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .layoutWeight(1)
    .padding({ left: 32, right: 32 })
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  ErrorState() {
    Column({ space: 12 }) {
      Text(RemoteI18n.t('filePreview.loadFailed'))
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text(this.state.errorText)
        .fontSize(13)
        .lineHeight(19)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
        .maxLines(4)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      if (this.state.errorRetryable) {
        Button(RemoteI18n.t('common.retry'))
          .height(42)
          .fontSize(14)
          .fontColor(CARD)
          .backgroundColor(INK)
          .borderRadius(21)
          .enabled(this.remoteAvailable)
          .opacity(this.remoteAvailable ? 1 : 0.45)
          .onClick(() => {
            this.onRefresh();
          })
      }
    }
    .width('100%')
    .layoutWeight(1)
    .padding({ left: 32, right: 32 })
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  UnsupportedState() {
    Column({ space: 10 }) {
      SymbolGlyph($r('sys.symbol.doc'))
        .fontSize(38)
        .fontColor([MUTED])
      Text(RemoteI18n.t('filePreview.unsupported'))
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
      Text(this.metadataDetail())
        .fontSize(13)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
      Button(RemoteI18n.t('chat.download'))
        .height(42)
        .fontSize(14)
        .fontColor(CARD)
        .backgroundColor(INK)
        .borderRadius(21)
        .enabled(this.remoteAvailable)
        .opacity(this.remoteAvailable ? 1 : 0.45)
        .onClick(() => {
          this.onDownload(this.state.target.rawReference || this.state.target.remotePath);
        })
    }
    .width('100%')
    .layoutWeight(1)
    .padding({ left: 32, right: 32 })
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  @Builder
  TextPreview() {
    Column() {
      if (this.state.truncated) {
        Text(RemoteI18n.f('filePreview.truncated', RemoteUiState.formatBytes(this.state.loadedBytes)))
          .width('100%')
          .fontSize(11)
          .fontColor(MUTED)
          .padding({ left: 14, right: 14, top: 7, bottom: 7 })
          .backgroundColor(SOFT)
          .border({ width: { bottom: 1 }, color: LINE })
      }
      Scroll(this.textScroller) {
        Text() {
          ForEach(this.syntaxTokens(), (token: CodeSyntaxToken) => {
            Span(token.text)
              .fontColor(this.syntaxTokenColor(token.kind))
              .textBackgroundStyle({
                color: this.isTargetLine(token.lineNumber) ? CODE_TARGET_BG : '#00000000'
              })
          }, (token: CodeSyntaxToken) => token.id)
        }
          .fontSize(12)
          .lineHeight(19)
          .fontColor(INK)
          .fontFamily('monospace')
          .textSelectable(TextSelectableMode.SELECTABLE_UNFOCUSABLE)
          .padding({ left: 14, right: 20, top: 14, bottom: 24 })
          .constraintSize({ minWidth: '100%' })
      }
      .scrollable(ScrollDirection.FREE)
      .scrollBar(BarState.Auto)
      .layoutWeight(1)
      .width('100%')
      .onAppear(() => {
        this.restoreTextScroll();
      })
      .onDidScroll((xOffset: number, yOffset: number, _scrollState: ScrollState) => {
        this.state.recordScroll(xOffset, yOffset);
      })
    }
    .layoutWeight(1)
    .width('100%')
    .backgroundColor(SOFT)
  }

  @Builder
  MarkdownPreview() {
    Scroll(this.markdownScroller) {
      MarkdownContent({
        text: this.state.textContent,
        onCopyText: (_text: string) => {},
        onOpenLink: (reference: string, label: string) => {
          this.onOpenLink(reference, label);
        }
      })
        .padding({ left: 20, right: 20, top: 16, bottom: 28 })
    }
    .scrollable(ScrollDirection.Vertical)
    .scrollBar(BarState.Auto)
    .layoutWeight(1)
    .width('100%')
    .onAppear(() => {
      this.restoreMarkdownScroll();
    })
    .onDidScroll((xOffset: number, yOffset: number, _scrollState: ScrollState) => {
      this.state.recordScroll(xOffset, yOffset);
    })
  }

  @Builder
  ImagePreview() {
    Stack({ alignContent: Alignment.Center }) {
      Image(`data:${this.state.mimeType};base64,${this.state.contentBase64}`)
        .width('100%')
        .height('100%')
        .objectFit(this.isImageOriginalSize() ? ImageFit.None : ImageFit.Contain)
        .onClick(() => {
          this.toggleImageSize();
        })
        .onError((_error: ImageError) => {
          this.imageOriginalSizeTarget = '';
          this.state.phase = FilePreviewPhase.Error;
          this.state.errorText = RemoteI18n.t('filePreview.imageDecodeFailed');
          this.state.errorRetryable = true;
        })
    }
    .layoutWeight(1)
    .width('100%')
    .padding(18)
    .backgroundColor(SOFT)
  }

  @Builder
  EmptyState() {
    Column() {
    }
    .layoutWeight(1)
    .width('100%')
  }

  private canUseRemoteAction(): boolean {
    return this.remoteAvailable && this.state.visible && this.state.phase !== FilePreviewPhase.Loading &&
      (this.state.phase !== FilePreviewPhase.Error || this.state.errorRetryable) &&
      this.state.target.isValid();
  }

  private canDownload(): boolean {
    return this.remoteAvailable && this.state.visible && !this.isDownloadingTarget() && this.state.target.isValid();
  }

  private isDownloadingTarget(): boolean {
    return this.downloadPath.length > 0 && this.downloadMatchesTarget() && this.downloadedPath.length === 0;
  }

  private downloadMatchesTarget(): boolean {
    const target = this.state.target.rawReference || this.state.target.remotePath;
    return target.length > 0 && (target === this.downloadPath || target === this.downloadedPath);
  }

  private isImageReady(): boolean {
    return this.state.phase === FilePreviewPhase.Ready &&
      this.state.rendererKind === FilePreviewRendererKind.Image;
  }

  private isImageOriginalSize(): boolean {
    return this.imageOriginalSizeTarget.length > 0 &&
      this.imageOriginalSizeTarget === this.state.target.remotePath;
  }

  private toggleImageSize(): void {
    this.imageOriginalSizeTarget = this.isImageOriginalSize() ? '' : this.state.target.remotePath;
  }

  private headerDetail(): string {
    if (!this.remoteAvailable) {
      return RemoteI18n.t('filePreview.offline');
    }
    if (this.state.phase === FilePreviewPhase.Loading) {
      return RemoteI18n.t('filePreview.loading');
    }
    return this.metadataDetail();
  }

  private metadataDetail(): string {
    const size = this.state.fileSize > 0 ? RemoteUiState.formatBytes(this.state.fileSize) : '';
    if (this.state.mimeType.length > 0 && size.length > 0) {
      return `${this.state.mimeType} · ${size}`;
    }
    return this.state.mimeType || size || this.state.target.remotePath;
  }

  private syntaxTokens(): CodeSyntaxToken[] {
    return this.syntaxHighlightCache.tokensFor(
      this.state.textContent,
      this.state.fileName || this.state.target.remotePath
    );
  }

  private syntaxTokenColor(kind: CodeSyntaxTokenKind): ResourceColor {
    if (kind === CodeSyntaxTokenKind.LineNumber) {
      return CODE_LINE_NUMBER;
    }
    if (kind === CodeSyntaxTokenKind.Keyword) {
      return CODE_KEYWORD;
    }
    if (kind === CodeSyntaxTokenKind.String) {
      return CODE_STRING;
    }
    if (kind === CodeSyntaxTokenKind.Number) {
      return CODE_NUMBER;
    }
    if (kind === CodeSyntaxTokenKind.Comment) {
      return CODE_COMMENT;
    }
    if (kind === CodeSyntaxTokenKind.Function) {
      return CODE_FUNCTION;
    }
    if (kind === CodeSyntaxTokenKind.Type) {
      return CODE_TYPE;
    }
    if (kind === CodeSyntaxTokenKind.Constant) {
      return CODE_CONSTANT;
    }
    if (kind === CodeSyntaxTokenKind.Property) {
      return CODE_PROPERTY;
    }
    return INK;
  }

  private isTargetLine(lineNumber: number): boolean {
    if (lineNumber <= 0 || this.state.target.lineStart <= 0) {
      return false;
    }
    const end = this.state.target.lineEnd > 0 ? this.state.target.lineEnd : this.state.target.lineStart;
    return lineNumber >= this.state.target.lineStart && lineNumber <= end;
  }

  private restoreTextScroll(): void {
    this.clearRestoreScrollTimer();
    this.restoreScrollTimerId = setTimeout(() => {
      this.restoreScrollTimerId = 0;
      const xOffset = this.state.initialScrollX();
      const yOffset = this.state.initialScrollY();
      this.textScroller.scrollTo({ xOffset, yOffset, animation: false });
      this.state.recordScroll(xOffset, yOffset);
    }, 30);
  }

  private restoreMarkdownScroll(): void {
    this.clearRestoreScrollTimer();
    this.restoreScrollTimerId = setTimeout(() => {
      this.restoreScrollTimerId = 0;
      const xOffset = this.state.initialScrollX();
      const yOffset = this.state.hasRecordedScroll ? this.state.initialScrollY() : 0;
      this.markdownScroller.scrollTo({ xOffset, yOffset, animation: false });
      this.state.recordScroll(xOffset, yOffset);
    }, 30);
  }

  private clearRestoreScrollTimer(): void {
    if (this.restoreScrollTimerId !== 0) {
      clearTimeout(this.restoreScrollTimerId);
      this.restoreScrollTimerId = 0;
    }
  }
}
