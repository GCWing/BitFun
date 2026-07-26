import { RemoteModelCatalog, RemoteModelConfig, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, GREEN, INK, LINE, MUTED, PAGE_BG } from './Theme';

@Component
export struct RemoteChatHeader {
  @Prop activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Prop workspaceBranch: string = '';
  @Prop desktopName: string = '';
  @Prop canStop: boolean = false;
  @Prop modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Prop selectedModelId: string = '';
  onBack: () => void = () => {};
  onNewSession: () => void = () => {};
  onStop: () => void = () => {};
  onRenameSession: (title: string) => void = (_title: string) => {};
  onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  @State showTitleEditor: boolean = false;
  @State showModelSelector: boolean = false;
  @State renameTitle: string = '';

  build() {
    Column() {
      this.HeaderRow()
      if (this.showTitleEditor) {
        this.TitleEditor()
      }
      if (this.showModelSelector) {
        this.ModelSelector()
      }
    }
    .width('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  HeaderRow() {
    Row({ space: 12 }) {
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.remote_ref_back'))
          .width(16)
          .height(25)
          .objectFit(ImageFit.Contain)
      }
        .width(48)
        .height(48)
        .backgroundColor(CARD)
        .borderRadius(24)
        .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
        .onClick(() => {
          this.onBack();
        })
      Column({ space: 3 }) {
        Text(this.activeSession.title || RemoteI18n.t('chat.remoteSession'))
          .fontSize(18)
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
      Row() {
        Stack({ alignContent: Alignment.Center }) {
          Image($r('app.media.remote_ref_edit'))
            .width(24)
            .height(24)
            .objectFit(ImageFit.Contain)
        }
          .width(44)
          .height(48)
          .onClick(() => {
            this.onNewSession();
          })
        Stack({ alignContent: Alignment.Center }) {
          Image($r('app.media.remote_ref_more'))
            .width(25)
            .height(8)
            .objectFit(ImageFit.Contain)
        }
          .width(44)
          .height(48)
          .onClick(() => {
            this.showModelSelector = !this.showModelSelector;
          })
      }
      .width(90)
      .height(48)
      .padding({ left: 1, right: 1 })
      .backgroundColor(CARD)
      .borderRadius(24)
      .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
    }
    .width('100%')
    .alignItems(VerticalAlign.Center)
    .padding({ left: 18, right: 18, top: 14, bottom: 16 })
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
      if (this.canStop) {
        Row() {
          Text(RemoteI18n.t('chat.stop'))
            .fontSize(14)
            .fontColor(INK)
          Blank()
          Text('■')
            .fontSize(12)
            .fontColor(INK)
        }
        .width('100%')
        .height(42)
        .padding({ left: 12, right: 12 })
        .backgroundColor('#F0EFEB')
        .borderRadius(12)
        .onClick(() => {
          this.showModelSelector = false;
          this.onStop();
        })
      }
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
    if (this.desktopName.length > 0) {
      return this.desktopName;
    }
    const brand = 'BitFun';
    return this.workspaceBranch.length > 0 ? `${brand} · ${this.workspaceBranch}` : brand;
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
