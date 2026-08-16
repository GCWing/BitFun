import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { CARD, INK, LINE, MUTED, PAGE_BG } from './Theme';
import { ComposerPresentation } from './ComposerBar';
import { ConversationView } from './ConversationView';
import { ConversationUiModelCatalog } from '../state/ConversationUiModels';
import { REMOTE_CREATE_COMPOSER_CAPABILITIES } from '../state/ChatComposerCapabilities';
import { ChatSurface } from '../state/ChatSurface';

@ComponentV2
export struct RemoteCreateSessionView {
  @Param state: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param presentation: ComposerPresentation = ComposerPresentation.Create;
  @Param showSidebarRestoreButton: boolean = false;
  @Param hoverOperateMode: boolean = false;
  @Param isVoiceListening: boolean = false;
  @Param modelCatalog: ConversationUiModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Param selectedModelId: string = '';
  @Event onBack: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onToggleDeviceMenu: () => void = () => {};
  @Event onToggleWorkspaceMenu: () => void = () => {};
  @Event onSelectDevice: (device: CloudAccountDevice) => void = (_device: CloudAccountDevice) => {};
  @Event onSelectWorkspace: (workspace?: RecentWorkspaceEntry) => void = (_workspace?: RecentWorkspaceEntry) => {};
  @Event onDraftChange: (value: string) => void = (_value: string) => {};
  @Event onSend: () => void = () => {};
  @Event onVoiceInput: () => void = () => {};
  @Event onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  @Local showSelectorSheet: boolean = false;

  aboutToAppear(): void {
    setTimeout(() => this.keepComposerFocused(), 180);
  }

  build() {
    Column() {
      ConversationView({
        surface: ChatSurface.Create,
        composerCapabilities: REMOTE_CREATE_COMPOSER_CAPABILITIES,
        composerPresentation: this.presentation === ComposerPresentation.Floating ?
          ComposerPresentation.Floating : ComposerPresentation.Create,
        composerInputId: 'remote-create-composer',
        chatInput: this.state.draft,
        isBusy: this.state.isSubmitting,
        inlineStatusText: this.state.errorText,
        connectionState: 'connected',
        isVoiceListening: this.isVoiceListening,
        modelCatalog: this.modelCatalog,
        selectedModelId: this.selectedModelId,
        showBackButton: true,
        showSidebarButton: false,
        showSidebarRestoreButton: this.showSidebarRestoreButton,
        hoverOperateMode: this.hoverOperateMode,
        aboveComposer: () => {
          this.ContextControls();
        },
        onBack: () => this.onBack(),
        onRestoreSidebar: () => this.onRestoreSidebar(),
        onSend: () => this.onSend(),
        onChatInputChange: (value: string) => this.onDraftChange(value),
        onVoiceInput: () => this.onVoiceInput(),
        onSelectModel: (modelId: string) => this.onSelectModel(modelId),
        onBackgroundTap: () => {
          this.state.closeMenu();
          this.keepComposerFocused();
        }
      })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
    .bindSheet($$this.showSelectorSheet, this.SelectorSheet(), this.selectorSheetOptions())
  }

  @Builder
  ContextControls() {
    Column({ space: 2 }) {
      if (this.presentation === ComposerPresentation.Floating) {
        this.ContextRow(
          'device',
          this.state.isLoadingDevices,
          () => this.onToggleDeviceMenu()
        )
      }
      this.ContextRow(
        'workspace',
        this.state.isLoadingWorkspaces,
        () => this.onToggleWorkspaceMenu()
      )
    }
    .width('100%')
    .padding({
      left: this.presentation === ComposerPresentation.Floating ? 10 : 28,
      right: this.presentation === ComposerPresentation.Floating ? 10 : 28,
      top: this.presentation === ComposerPresentation.Floating ? 8 : 0,
      bottom: 4
    })
  }

  @Builder
  ContextRow(kind: string, loading: boolean, onClick: () => void) {
    Row({ space: 13 }) {
      this.ContextGlyph(kind, loading)
      Text(loading ? RemoteI18n.t('common.loading') :
        (kind === 'device' ?
          (this.state.selectedDeviceName || RemoteI18n.t('remote.create.noDevice')) :
          (this.state.selectedWorkspaceName || RemoteI18n.t('remote.create.chat'))))
        .constraintSize({ maxWidth: '74%' })
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(loading ? MUTED : INK)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      SymbolGlyph(this.state.openMenu === (kind === 'device' ? 'devices' : 'workspaces') ?
        $r('sys.symbol.chevron_up') : $r('sys.symbol.chevron_down'))
        .fontSize(13)
        .fontColor([MUTED])
        .width(22)
        .height(30)
        .opacity(loading ? 0.42 : 1)
      Blank()
        .layoutWeight(1)
    }
    .width('100%')
    .height(48)
    .padding({ left: 8, right: 10 })
    .borderRadius(12)
    .bindPopup(this.state.openMenu === (kind === 'device' ? 'devices' : 'workspaces'), {
      builder: () => {
        if (kind === 'device') {
          this.DeviceMenu(false)
        } else {
          this.WorkspaceMenu(false)
        }
      },
      placement: Placement.Top,
      popupColor: '#00000000',
      enableArrow: false,
      autoCancel: true,
      mask: false,
      targetSpace: 8,
      onStateChange: (event) => {
        if (!event.isVisible) {
          this.state.closeMenu();
        }
      }
    })
    .onClick(() => {
      onClick();
      this.keepComposerFocused();
    })
  }

  @Builder
  ContextGlyph(kind: string, loading: boolean) {
    if (kind === 'device') {
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    } else if (this.state.selectedWorkspacePath.length > 0) {
      SymbolGlyph($r('sys.symbol.folder'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    } else {
      SymbolGlyph($r('sys.symbol.message'))
        .fontSize(22).fontColor([MUTED]).width(26).height(26).opacity(loading ? 0.42 : 1)
    }
  }

  @Builder
  DeviceMenu(asSheet: boolean) {
    Column() {
      if (this.state.devices.length === 0 && this.state.isLoadingDevices) {
        this.MenuMessage(RemoteI18n.t('common.loading'))
      } else if (this.state.devices.length === 0) {
        this.MenuMessage(RemoteI18n.t('remote.create.noOnlineDevice'))
      } else {
        ForEach(this.state.devices, (device: CloudAccountDevice) => {
          this.DeviceMenuRow(device)
        }, (device: CloudAccountDevice) => device.deviceId)
      }
    }
    .width(asSheet ? '100%' : (this.presentation === ComposerPresentation.Floating ? 360 : 340))
    .padding({ top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(asSheet ? 0 : 16)
    .border({ width: 1, color: LINE })
    .shadow({ radius: asSheet ? 0 : 20, color: asSheet ? '#00000000' : '#1A000000', offsetY: 8 })
  }

  @Builder
  DeviceMenuRow(device: CloudAccountDevice) {
    Row({ space: 12 }) {
      this.SelectionMark(device.deviceId === this.state.selectedDeviceId)
      SymbolGlyph($r('sys.symbol.desktop'))
        .fontSize(22).fontColor([MUTED]).width(27).height(27)
      Text(device.deviceName)
        .layoutWeight(1)
        .fontSize(16)
        .fontColor(INK)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .height(58)
    .padding({ left: 16, right: 18, top: 8, bottom: 8 })
    .onClick(() => {
      this.showSelectorSheet = false;
      this.onSelectDevice(device);
      this.keepComposerFocused();
    })
  }

  @Builder
  WorkspaceMenu(asSheet: boolean) {
    Column() {
      Row({ space: 12 }) {
        this.SelectionMark(this.state.selectedWorkspacePath.length === 0)
        SymbolGlyph($r('sys.symbol.message'))
          .fontSize(22).fontColor([MUTED]).width(27).height(27)
        Text(RemoteI18n.t('remote.create.chat'))
          .layoutWeight(1)
          .fontSize(16)
          .fontColor(INK)
      }
      .width('100%')
      .height(58)
      .padding({ left: 16, right: 18 })
      .onClick(() => {
        this.showSelectorSheet = false;
        this.onSelectWorkspace(undefined);
        this.keepComposerFocused();
      })

      Divider().color(LINE).margin({ left: 58, right: 18 })

      if (this.state.workspaces.length === 0 && this.state.isLoadingWorkspaces) {
        this.MenuMessage(RemoteI18n.t('common.loading'))
      } else if (this.state.workspaces.length === 0) {
        this.MenuMessage(RemoteI18n.t('home.noRecentWorkspaces'))
      } else {
        Scroll() {
          Column() {
            ForEach(this.state.workspaces, (workspace: RecentWorkspaceEntry) => {
              this.WorkspaceMenuRow(workspace)
            }, (workspace: RecentWorkspaceEntry) => workspace.path)
          }
        }
        .width('100%')
        .constraintSize({
          maxHeight: this.presentation === ComposerPresentation.Floating ? 190 : 92
        })
        .scrollBar(BarState.Off)
      }
    }
    .width(asSheet ? '100%' : (this.presentation === ComposerPresentation.Floating ? 360 : 340))
    .padding({ top: 8, bottom: 8 })
    .backgroundColor(CARD)
    .borderRadius(asSheet ? 0 : 16)
    .border({ width: 1, color: LINE })
    .shadow({ radius: asSheet ? 0 : 20, color: asSheet ? '#00000000' : '#1A000000', offsetY: 8 })
  }

  @Builder
  WorkspaceMenuRow(workspace: RecentWorkspaceEntry) {
    Row({ space: 12 }) {
      this.SelectionMark(workspace.path === this.state.selectedWorkspacePath)
      SymbolGlyph($r('sys.symbol.folder'))
        .fontSize(22).fontColor([MUTED]).width(27).height(27)
      Column({ space: 2 }) {
        Text(workspace.name)
          .width('100%')
          .fontSize(16)
          .fontColor(INK)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(workspace.path)
          .width('100%')
          .fontSize(11)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .height(64)
    .padding({ left: 16, right: 18 })
    .onClick(() => {
      this.showSelectorSheet = false;
      this.onSelectWorkspace(workspace);
      this.keepComposerFocused();
    })
  }

  @Builder
  SelectionMark(selected: boolean) {
    Stack({ alignContent: Alignment.Center }) {
      if (selected) {
        SymbolGlyph($r('sys.symbol.checkmark_circle'))
          .fontSize(19)
          .fontColor([INK])
      }
    }
    .width(20)
    .height(24)
  }

  @Builder
  MenuMessage(message: string) {
    Text(message)
      .width('100%')
      .padding({ left: 20, right: 20, top: 18, bottom: 18 })
      .fontSize(13)
      .fontColor(MUTED)
  }

  @Builder
  SelectorSheet() {
    Column() {
      Row() {
        Text(this.state.openMenu === 'devices' ? RemoteI18n.t('remote.device') : RemoteI18n.t('remote.workspace'))
          .layoutWeight(1)
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.xmark'))
            .fontSize(16)
            .fontColor([MUTED])
            .width(20)
            .height(20)
        }
        .width(44)
        .height(44)
        .accessibilityText(RemoteI18n.t('common.close'))
        .onClick(() => this.closeSelectorSheet())
      }
      .width('100%')
      .height(52)
      .padding({ left: 18, right: 8 })
      if (this.state.openMenu === 'devices') {
        this.DeviceMenu(true)
      } else {
        this.WorkspaceMenu(true)
      }
    }
    .width('100%')
    .padding({ left: 12, right: 12, bottom: 16 })
    .backgroundColor(CARD)
    .borderRadius(16)
  }

  private keepComposerFocused(): void {
    setTimeout(() => focusControl.requestFocus('remote-create-composer'), 30);
  }

  private closeSelectorSheet(): void {
    this.showSelectorSheet = false;
    this.state.closeMenu();
    this.keepComposerFocused();
  }

  private selectorSheetOptions(): SheetOptions {
    return {
      height: this.state.openMenu === 'devices' ? 420 : 440,
      backgroundColor: '#00000000',
      maskColor: '#44000000',
      showClose: false,
      dragBar: true,
      onWillDismiss: () => {
        this.state.closeMenu();
      }
    };
  }
}
