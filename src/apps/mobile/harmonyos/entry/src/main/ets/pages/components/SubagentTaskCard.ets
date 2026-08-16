import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ToolCollapsePolicy } from '../policy/ToolCollapsePolicy';
import { ChatTypingDots } from './ChatMessageChrome';
import { INK, LINE, MUTED, SOFT } from './Theme';

@ComponentV2
export struct SubagentTaskCard {
  @Param title: string = '';
  @Param running: boolean = false;
  @Param content: string = '';
  @Param hasChildren: boolean = false;
  @BuilderParam childItems: () => void = this.EmptyChildren;
  @Local expanded: boolean = false;
  @Local userToggled: boolean = false;

  aboutToAppear(): void {
    this.userToggled = false;
    this.expanded = ToolCollapsePolicy.defaultSubagentExpanded(this.running);
  }

  @Monitor('running')
  onRunningChanged(): void {
    this.expanded = ToolCollapsePolicy.subagentExpandedForStatus(
      this.running,
      this.userToggled,
      this.expanded
    );
    if (!this.running) {
      this.userToggled = false;
    }
  }

  build() {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph(this.expanded ? $r('sys.symbol.chevron_down') : $r('sys.symbol.chevron_right'))
            .fontSize(14)
            .fontColor([MUTED])
            .width(16)
            .height(16)
        }
        .width(18)
        .height(32)
        Text(this.title)
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .layoutWeight(1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%')
      .height(32)
      .alignItems(VerticalAlign.Center)
      .onClick(() => {
        this.userToggled = this.running;
        this.expanded = !this.expanded;
      })
      .accessibilityText(this.expanded ? RemoteI18n.t('chat.showLess') : RemoteI18n.t('chat.showMore'))
      if (this.expanded) {
        if (this.running) {
          ChatTypingDots()
        }
        if (this.content.trim().length > 0) {
          Text(this.content)
            .fontSize(13)
            .lineHeight(19)
            .fontColor(MUTED)
            .width('100%')
        }
        if (this.hasChildren) {
          this.childItems()
        }
      }
    }
    .width('100%')
    .padding({ left: 10, right: 10, top: 8, bottom: 8 })
    .backgroundColor(SOFT)
    .borderRadius(12)
    .border({ width: 1, color: LINE })
  }

  @Builder
  EmptyChildren() {
  }
}
