import { RemoteModelCatalog, RemoteModelConfig, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, PAGE_BG } from './Theme';

@Component
export struct ChatHeader {
  @Prop activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Prop workspaceName: string = '';
  @Prop workspaceBranch: string = '';
  @Prop canStop: boolean = false;
  @Prop modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Prop selectedModelId: string = '';
  onBack: () => void = () => {};
  onStop: () => void = () => {};
  onRenameSession: (title: string) => void = (_title: string) => {};
  onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  @State showTitleEditor: boolean = false;
  @State showModelSelector: boolean = false;
  @State renameTitle: string = '';

  build() {
    Column() {
      if (this.isGeneralChat()) {
        this.GeneralHeaderRow()
      } else {
        this.HeaderRow()
      }
      if (!this.isGeneralChat() && this.showTitleEditor) {
        this.TitleEditor()
      }
      if (!this.isGeneralChat() && this.showModelSelector) {
        this.ModelSelector()
      }
    }
    .width('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  GeneralHeaderRow() {
    Row() {
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.gpt_home_menu_glyph'))
          .width(26)
          .height(26)
          .objectFit(ImageFit.Contain)
      }
      .width(48)
      .height(48)
      .onClick(() => {
        this.onBack();
      })
      Text('BitFun')
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(INK)
        .height(48)
        .padding({ left: 18, right: 18 })
        .margin({ left: 12 })
      Blank()
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.gpt_home_chat_glyph'))
          .width(27)
          .height(27)
          .objectFit(ImageFit.Contain)
      }
      .width(48)
      .height(48)
      .onClick(() => {
        this.onBack();
      })
    }
    .width('100%')
    .height(52)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 24, right: 24 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  HeaderRow() {
    Row({ space: 14 }) {
      Text('‹')
        .width(54)
        .height(54)
        .fontSize(34)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor(CARD)
        .borderRadius(27)
        .border({ width: 1, color: LINE })
        .onClick(() => {
          this.onBack();
        })
      Column({ space: 3 }) {
        Text(this.activeSession.title || RemoteI18n.t('chat.remoteSession'))
          .fontSize(20)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .textAlign(TextAlign.Start)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .onClick(() => {
            this.renameTitle = this.activeSession.title || '';
            this.showTitleEditor = true;
          })
        Row({ space: 6 }) {
          Text('')
            .width(7)
            .height(7)
            .backgroundColor(GREEN)
            .borderRadius(4)
          Text(this.headerContextTitle())
            .fontSize(14)
            .fontColor(MUTED)
            .textAlign(TextAlign.Start)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .width('100%')
        .justifyContent(FlexAlign.Start)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Row({ space: 8 }) {
        if (this.canStop) {
          Text('□')
            .width(54)
            .height(54)
            .fontSize(16)
            .fontColor(INK)
            .textAlign(TextAlign.Center)
            .backgroundColor(CARD)
            .borderRadius(27)
            .border({ width: 1, color: LINE })
            .onClick(() => {
              this.onStop();
            })
        }
        Text('···')
          .width(54)
          .height(54)
          .fontSize(19)
          .fontColor(INK)
          .textAlign(TextAlign.Center)
          .backgroundColor(CARD)
          .borderRadius(27)
          .border({ width: 1, color: LINE })
          .onClick(() => {
            this.showModelSelector = !this.showModelSelector;
          })
      }
    }
    .width('100%')
    .alignItems(VerticalAlign.Center)
    .padding({ left: 18, right: 18, top: 22, bottom: 24 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  TitleEditor() {
    Row({ space: 8 }) {
      TextInput({ placeholder: RemoteI18n.t('chat.sessionTitle'), text: this.renameTitle })
        .layoutWeight(1)
        .height(42)
        .fontSize(14)
        .backgroundColor(CARD)
        .borderRadius(14)
        .padding({ left: 12, right: 12 })
        .border({ width: 1, color: LINE })
        .defaultFocus(false)
        .onChange((value: string) => {
          this.renameTitle = value;
        })
      Text(RemoteI18n.t('common.save'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(CARD)
        .textAlign(TextAlign.Center)
        .backgroundColor(this.renameTitle.trim().length > 0 ? ACCENT : '#EDEBE6')
        .borderRadius(14)
        .onClick(() => {
          if (this.renameTitle.trim().length > 0) {
            this.onRenameSession(this.renameTitle.trim());
            this.showTitleEditor = false;
          }
        })
      Text(RemoteI18n.t('common.cancel'))
        .width(52)
        .height(42)
        .fontSize(13)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor('#F0EFEB')
        .borderRadius(14)
        .onClick(() => {
          this.showTitleEditor = false;
        })
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 10, bottom: 8 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  ModelSelector() {
    Column({ space: 8 }) {
      Row() {
        Text(`${RemoteI18n.t('chat.selectModel')} · ${this.displaySelectedModel()}`)
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text(RemoteI18n.t('common.close'))
          .fontSize(12)
          .fontColor(MUTED)
          .onClick(() => {
            this.showModelSelector = false;
          })
      }
      .width('100%')
      List({ space: 8 }) {
        ForEach(this.enabledModels(), (model: RemoteModelConfig) => {
          ListItem() {
            this.ModelRow(model)
          }
        }, (model: RemoteModelConfig) => model.id)
      }
      .width('100%')
      .height(this.modelListHeight())
      .scrollBar(BarState.Auto)
      .edgeEffect(EdgeEffect.Spring)
      .divider(null)
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 10, bottom: 12 })
    .backgroundColor(PAGE_BG)
  }

  @Builder
  ModelRow(model: RemoteModelConfig) {
    Row({ space: 10 }) {
      Text(this.selectedModelId === model.id ? '●' : '○')
        .fontSize(12)
        .fontColor(this.selectedModelId === model.id ? GREEN : MUTED)
        .width(18)
        .textAlign(TextAlign.Center)
      Column({ space: 2 }) {
        Text(this.primaryModelLabel(model))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(this.secondaryModelLabel(model))
          .fontSize(11)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .padding({ left: 12, right: 12, top: 10, bottom: 10 })
    .backgroundColor(this.selectedModelId === model.id ? '#F0EFEB' : CARD)
    .borderRadius(14)
    .border({ width: 1, color: this.selectedModelId === model.id ? '#D8D4CA' : LINE })
    .onClick(() => {
      this.showModelSelector = false;
      this.onSelectModel(model.id);
    })
  }

  private headerContextTitle(): string {
    const brand = 'BitFun';
    return this.workspaceBranch.length > 0 ? `${brand} · ${this.workspaceBranch}` : brand;
  }

  private isGeneralChat(): boolean {
    return this.activeSession.agentType === 'chat';
  }

  private enabledModels(): RemoteModelConfig[] {
    return this.modelCatalog.models.filter((model: RemoteModelConfig) => model.enabled);
  }

  private modelListHeight(): number {
    const rowHeight = 58;
    const rowGap = 8;
    const visibleRows = Math.min(this.enabledModels().length, 5);
    if (visibleRows <= 0) {
      return 0;
    }
    return visibleRows * rowHeight + Math.max(0, visibleRows - 1) * rowGap;
  }

  private displaySelectedModel(): string {
    const selected = this.selectedModel();
    if (selected) {
      return this.primaryModelLabel(selected);
    }
    return RemoteI18n.t('chat.model');
  }

  private selectedModel(): RemoteModelConfig | undefined {
    const modelId = this.selectedModelId || this.modelCatalog.session_model_id || this.modelCatalog.default_models.primary || '';
    if (modelId.length === 0) {
      return undefined;
    }
    return this.modelCatalog.models.find((model: RemoteModelConfig) => model.id === modelId);
  }

  private primaryModelLabel(model: RemoteModelConfig): string {
    const modelName = this.cleanModelLabel(model.model_name || '');
    if (this.isSpecificModelLabel(modelName)) {
      return modelName;
    }
    const name = this.cleanModelLabel(model.name || '');
    if (this.isSpecificModelLabel(name)) {
      return name;
    }
    const id = this.cleanModelLabel(model.id || '');
    if (id.length > 0) {
      return id;
    }
    return RemoteI18n.t('chat.model');
  }

  private secondaryModelLabel(model: RemoteModelConfig): string {
    const provider = this.cleanModelLabel(model.provider || '');
    const name = this.cleanModelLabel(model.name || '');
    const primary = this.primaryModelLabel(model);
    if (provider.length > 0 && name.length > 0 && name !== primary && name !== provider) {
      return `${provider} · ${name}`;
    }
    if (provider.length > 0 && provider !== primary) {
      return provider;
    }
    if (name.length > 0 && name !== primary) {
      return name;
    }
    return model.id || primary;
  }

  private cleanModelLabel(value: string): string {
    const trimmed = (value || '').trim();
    if (trimmed.length === 0) {
      return '';
    }
    const withoutScheme = trimmed.replace(/^openbitfun[:/_-]+/i, '').replace(/^anthropic[:/_-]+/i, '');
    const parts = withoutScheme.split(/[/:]/).filter((part: string) => part.length > 0);
    return parts.length > 0 ? parts[parts.length - 1] : withoutScheme;
  }

  private isSpecificModelLabel(label: string): boolean {
    const normalized = label.toLowerCase();
    return label.length > 0 &&
      normalized !== 'openbitfun' &&
      normalized !== 'anthropic' &&
      normalized !== 'openai' &&
      normalized !== 'google' &&
      normalized !== 'azure' &&
      normalized !== 'bitfun';
  }
}
