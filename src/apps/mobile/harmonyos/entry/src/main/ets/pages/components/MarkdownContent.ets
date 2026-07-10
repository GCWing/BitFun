import { RemoteI18n } from '../../i18n/RemoteI18n';
import { MarkdownParser, ParsedMarkdownBlock, ParsedMarkdownInline, ParsedMarkdownListItem } from '../../services/MarkdownParser';
import { CARD, INK, LINE, MUTED } from './Theme';

@Component
export struct MarkdownContent {
  @Prop text: string = '';
  onCopyText: (text: string) => void = (_text: string) => {};

  build() {
    Column({ space: 5 }) {
      ForEach(MarkdownParser.parse(this.text), (block: ParsedMarkdownBlock) => {
        this.MarkdownBlockView(block)
      }, (block: ParsedMarkdownBlock) => block.id)
    }
    .width('100%')
  }

  @Builder
  MarkdownBlockView(block: ParsedMarkdownBlock) {
    if (block.type === 'code') {
      this.CodeBlock(block.language, block.text)
    } else if (block.type === 'heading') {
      this.InlineText(block.inlines, this.headingFontSize(block.level), this.headingLineHeight(block.level), INK, true)
    } else if (block.type === 'quote') {
      Column() {
        this.InlineText(block.inlines, 13, 18, MUTED, false)
      }
        .width('100%')
        .padding({ left: 9 })
        .border({ width: { left: 2 }, color: LINE })
    } else if (block.type === 'list') {
      this.MarkdownList(block.items)
    } else if (block.type === 'table') {
      this.MarkdownTable(block.text)
    } else if (block.type === 'divider') {
      Text('')
        .width('100%')
        .height(1)
        .backgroundColor(LINE)
    } else {
      this.InlineText(block.inlines, 14, 21, INK, false)
    }
  }

  @Builder
  InlineText(inlines: ParsedMarkdownInline[], fontSize: number, lineHeight: number, color: string, bold: boolean) {
    Text() {
      ForEach(inlines, (inline: ParsedMarkdownInline) => {
        this.InlineSpan(inline, color, bold)
      }, (inline: ParsedMarkdownInline) => inline.id)
    }
    .fontSize(fontSize)
    .lineHeight(lineHeight)
    .fontColor(color)
    .width('100%')
  }

  @Builder
  InlineSpan(inline: ParsedMarkdownInline, color: string, bold: boolean) {
    if (inline.type === 'strong') {
      Span(inline.text)
        .fontWeight(FontWeight.Bold)
        .fontColor(color)
    } else if (inline.type === 'emphasis') {
      Span(inline.text)
        .fontStyle(FontStyle.Italic)
        .fontWeight(bold ? FontWeight.Bold : FontWeight.Regular)
        .fontColor(color)
    } else if (inline.type === 'code') {
      Span(inline.text)
        .fontFamily('monospace')
        .fontWeight(bold ? FontWeight.Bold : FontWeight.Regular)
        .fontColor(INK)
    } else if (inline.type === 'link') {
      Span(inline.text)
        .fontWeight(bold ? FontWeight.Bold : FontWeight.Regular)
        .fontColor('#1D64C8')
    } else {
      Span(inline.text)
        .fontWeight(bold ? FontWeight.Bold : FontWeight.Regular)
        .fontColor(color)
    }
  }

  @Builder
  MarkdownList(items: ParsedMarkdownListItem[]) {
    Column({ space: 4 }) {
      ForEach(items, (item: ParsedMarkdownListItem) => {
        Row({ space: 7 }) {
          Text(item.marker)
            .width(20)
            .fontSize(14)
            .lineHeight(21)
            .fontColor(MUTED)
            .textAlign(TextAlign.End)
          Column() {
            this.InlineText(item.inlines, 14, 21, INK, false)
          }
            .layoutWeight(1)
        }
        .width('100%')
        .alignItems(VerticalAlign.Top)
      }, (item: ParsedMarkdownListItem) => item.id)
    }
    .width('100%')
  }

  @Builder
  MarkdownTable(text: string) {
    Scroll() {
      Text(text)
        .fontSize(10)
        .lineHeight(15)
        .fontColor(INK)
        .fontFamily('monospace')
        .constraintSize({ minWidth: '100%' })
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
    .padding({ left: 9, right: 9, top: 8, bottom: 8 })
    .backgroundColor('#F3F2EE')
    .borderRadius(12)
    .border({ width: 1, color: LINE })
  }

  @Builder
  CodeBlock(language: string, body: string) {
    Column({ space: 7 }) {
      Row() {
        Text(language.length > 0 ? language : 'code')
          .fontSize(10)
          .fontColor(MUTED)
        Blank()
        Text(RemoteI18n.t('common.copy'))
          .fontSize(10)
          .fontColor(MUTED)
          .onClick(() => {
            this.onCopyText(body);
          })
      }
      .width('100%')
      Scroll() {
        Text(body)
          .fontSize(10)
          .lineHeight(15)
          .fontColor(INK)
          .fontFamily('monospace')
          .constraintSize({ minWidth: '100%' })
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .width('100%')
    }
    .width('100%')
    .padding({ left: 9, right: 9, top: 8, bottom: 8 })
    .backgroundColor('#F3F2EE')
    .borderRadius(12)
    .border({ width: 1, color: LINE })
  }

  private headingFontSize(level: number): number {
    if (level <= 1) {
      return 16;
    }
    if (level === 2) {
      return 15;
    }
    if (level === 3) {
      return 14;
    }
    return 14;
  }

  private headingLineHeight(level: number): number {
    if (level <= 1) {
      return 23;
    }
    if (level === 2) {
      return 22;
    }
    if (level === 3) {
      return 20;
    }
    return 20;
  }
}
