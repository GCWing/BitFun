import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, INK, LINE, MUTED, PAGE_BG, SOFT, SUBTLE } from './Theme';

@Component
export struct CreateSessionSheet {
  @Prop createAgentType: string = 'code';
  @Prop workspaceName: string = '';
  @Prop workspaceBranch: string = '';
  @Prop isBusy: boolean = false;
  @Link sessionTitle: string;
  @Link instruction: string;
  onClose: () => void = () => {};
  onChooseWorkspace: () => void = () => {};
  onStart: () => void = () => {};

  build() {
    Column() {
      Blank()
      Column({ space: 16 }) {
        Text('')
          .width(42)
          .height(4)
          .backgroundColor(SUBTLE)
          .borderRadius(2)

        Row() {
          Text(RemoteI18n.f('sheet.createTitle', this.displayAgent()))
            .fontSize(20)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
          Blank()
          Text('×')
            .fontSize(26)
            .fontColor(INK)
            .width(34)
            .height(34)
            .textAlign(TextAlign.Center)
            .onClick(() => {
              this.onClose();
            })
        }
        .width('100%')

        Scroll() {
          Column({ space: 18 }) {
            Text(RemoteI18n.t('sheet.workspace'))
              .fontSize(14)
              .fontWeight(FontWeight.Medium)
              .fontColor(INK)
              .width('100%')
              .margin({ top: 4 })

            Row({ space: 10 }) {
              this.FolderGlyph()
              Text(this.workspaceTitle())
                .fontSize(15)
                .fontWeight(FontWeight.Medium)
                .fontColor(INK)
                .layoutWeight(1)
                .maxLines(1)
                .textOverflow({ overflow: TextOverflow.Ellipsis })
              Text('›')
                .fontSize(18)
                .fontColor(INK)
            }
            .width('100%')
            .height(58)
            .padding({ left: 14, right: 12 })
            .backgroundColor(SOFT)
            .borderRadius(14)
            .border({ width: 1, color: LINE })
            .onClick(() => {
              this.onChooseWorkspace();
            })

            this.InstructionField()
            this.AdvancedOptions()
          }
          .width('100%')
          .padding({ bottom: 8 })
        }
        .layoutWeight(1)
        .scrollBar(BarState.Off)

        Button(this.isBusy ? RemoteI18n.t('sheet.creating') : RemoteI18n.t('sheet.start'))
          .width('100%')
          .height(56)
          .fontSize(17)
          .fontWeight(FontWeight.Medium)
          .fontColor(CARD)
          .backgroundColor(ACCENT)
          .borderRadius(14)
          .enabled(!this.isBusy)
          .onClick(() => {
            this.onStart();
          })
      }
      .width('100%')
      .height('64%')
      .padding({ left: 22, right: 22, top: 10, bottom: 26 })
      .backgroundColor(PAGE_BG)
      .borderRadius({ topLeft: 20, topRight: 20 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#99000000')
  }

  @Builder
  TitleField() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('sheet.sessionTitleOptional'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      TextInput({ placeholder: this.defaultSessionTitle(), text: this.sessionTitle })
        .height(52)
        .fontSize(15)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .padding({ left: 14, right: 14 })
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.sessionTitle = value;
        })
    }
    .width('100%')
  }

  @Builder
  InstructionField() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('sheet.firstInstructionOptional'))
        .fontSize(14)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      TextArea({ placeholder: RemoteI18n.t('sheet.instructionPlaceholder'), text: this.instruction })
        .height(68)
        .fontSize(15)
        .backgroundColor(SOFT)
        .borderRadius(14)
        .padding(14)
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.instruction = value;
        })
    }
    .width('100%')
  }

  @Builder
  AdvancedOptions() {
    Column({ space: 10 }) {
      Text(RemoteI18n.t('sheet.advancedOptions'))
        .fontSize(15)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .width('100%')
      Row() {
        Text('模型')
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text('Auto')
          .fontSize(14)
          .fontColor(INK)
        Text('⌄')
          .fontSize(18)
          .fontColor(INK)
          .margin({ left: 10 })
      }
      .width('100%')
      .height(56)
      .padding({ left: 14, right: 14 })
      .backgroundColor(SOFT)
      .borderRadius(14)
      .border({ width: 1, color: LINE })
    }
    .width('100%')
  }

  @Builder
  FolderGlyph() {
    Stack() {
      Text('')
        .width(26)
        .height(18)
        .borderRadius(4)
        .border({ width: 2, color: INK })
        .position({ x: 2, y: 9 })
      Text('')
        .width(12)
        .height(7)
        .borderRadius({ topLeft: 4, topRight: 4 })
        .border({ width: { top: 2, left: 2, right: 2 }, color: INK })
        .position({ x: 4, y: 5 })
    }
    .width(30)
    .height(30)
  }

  private workspaceTitle(): string {
    const base = this.workspaceName || 'BitFun';
    return this.workspaceBranch.length > 0 ? `${base} / ${this.workspaceBranch}` : base;
  }

  private defaultSessionTitle(): string {
    return `Remote ${this.displayAgent()} Session`;
  }

  private displayAgent(): string {
    const normalized = (this.createAgentType || '').toLowerCase();
    if (normalized === 'cowork') {
      return 'Cowork';
    }
    if (normalized === 'claw') {
      return 'Claw';
    }
    return 'Code';
  }
}
