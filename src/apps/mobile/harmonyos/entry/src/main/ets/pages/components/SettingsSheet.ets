import { CARD, INK, LINE } from './Theme';

const SETTINGS_SHEET_BG: string = '#F4F4F7';
const SETTINGS_ROW_VALUE: string = '#8F8F94';
const SETTINGS_DANGER: string = '#D93025';

@Component
export struct SettingsSheet {
  onClose: () => void = () => {};
  @State selectedTheme: string = '系统';
  @State isMockSignedIn: boolean = true;
  @State showMockPanel: boolean = false;
  @State showThemePicker: boolean = false;
  @State showLogoutConfirm: boolean = false;
  @State mockPanelTitle: string = '';
  @State mockPanelBody: string = '';

  private openMockPanel(title: string, body: string) {
    this.mockPanelTitle = title;
    this.mockPanelBody = body;
    this.showMockPanel = true;
  }

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      Scroll() {
        Column({ space: 0 }) {
          this.ProfileHeader()
          this.SectionTitle('自定义 BitFun')
          this.CustomizationCard()

          if (this.isMockSignedIn) {
            this.SectionTitle('账户')
            this.AccountCard()
          }

          this.SectionTitle('主题')
          this.ThemeCard()

          this.SectionTitle('获取帮助')
          this.HelpCard()

          if (this.isMockSignedIn) {
            this.LogoutCard()
          } else {
            this.LoginCard()
          }
        }
        .width('100%')
        .padding({ left: 16, right: 16, top: 46, bottom: 34 })
      }
      .width('100%')
      .height('100%')
      .scrollBar(BarState.Off)

      Button() {
        Image($r('app.media.settings_close_x'))
          .width(28)
          .height(28)
          .objectFit(ImageFit.Contain)
      }
      .width(50)
      .height(50)
      .padding(0)
      .type(ButtonType.Circle)
      .backgroundColor(CARD)
      .shadow({ radius: 16, color: '#10000000', offsetY: 7 })
      .margin({ top: 22, right: 18 })
      .onClick(() => {
        this.onClose();
      })

      if (this.showMockPanel) {
        this.MockPanel()
      }

      if (this.showThemePicker) {
        this.ThemePicker()
      }

      if (this.showLogoutConfirm) {
        this.LogoutConfirm()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(SETTINGS_SHEET_BG)
    .borderRadius({ topLeft: 34, topRight: 34 })
  }

  @Builder
  ProfileHeader() {
    Column({ space: 18 }) {
      Image($r('app.media.settings_profile_avatar_gw'))
        .width(86)
        .height(86)
        .objectFit(ImageFit.Contain)
      Text(this.isMockSignedIn ? 'guanqi wang' : '未登录')
        .fontSize(19)
        .fontWeight(FontWeight.Bold)
        .fontColor(INK)
    }
    .width('100%')
    .alignItems(HorizontalAlign.Center)
    .margin({ bottom: 24 })
    .onClick(() => {
      this.openMockPanel('个人资料', '这里会编辑头像、昵称和账号资料。目前是本地 mock。');
    })
  }

  @Builder
  SectionTitle(title: string) {
    Text(title)
      .fontSize(21)
      .fontWeight(FontWeight.Bold)
      .fontColor('#9A9A9E')
      .width('100%')
      .margin({ left: 16, top: 0, bottom: 8 })
  }

  @Builder
  CustomizationCard() {
    Column() {
      this.SettingsRow($r('app.media.settings_personalization_smile'), '个性化', '', true, () => {
        this.openMockPanel('个性化', '这里会配置 BitFun 的回复风格、称呼和常用偏好。目前是本地 mock。');
      })
      this.SettingsDivider()
      this.SettingsRow($r('app.media.settings_memory_book'), '记忆', '', true, () => {
        this.openMockPanel('记忆', '这里会管理 BitFun 已保存的长期偏好和可删除记忆。目前是本地 mock。');
      })
      this.SettingsDivider()
      this.SettingsRow($r('app.media.settings_apps_grid'), '应用', '', true, () => {
        this.openMockPanel('应用', '这里会展示已连接的应用、插件和集成能力。目前是本地 mock。');
      })
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 24 })
  }

  @Builder
  AccountCard() {
    Column() {
      this.SettingsRow($r('app.media.settings_email_envelope'), '电子邮件', 'wgq0617@gmail.com', true, () => {
        this.openMockPanel('电子邮件', '这里会修改或验证登录邮箱。目前未接入账号服务，仅展示 mock 数据。');
      })
      this.SettingsDivider()
      this.SettingsRow($r('app.media.settings_phone'), '电话号码', '+6285798044835', true, () => {
        this.openMockPanel('电话号码', '这里会绑定或更换手机号。目前未接入短信验证，仅展示 mock 数据。');
      })
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 24 })
  }

  @Builder
  ThemeCard() {
    Column() {
      this.AppearanceRow()
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 24 })
  }

  @Builder
  HelpCard() {
    Column() {
      this.SettingsRow($r('app.media.settings_help_report_flag'), '报告应用问题', '', true, () => {
        this.openMockPanel('报告应用问题', '这里会收集问题描述、设备信息和日志附件。目前是本地 mock，不会真正提交。');
      })
      this.SettingsDivider()
      this.SettingsRow($r('app.media.settings_help_about_info'), '关于', '', true, () => {
        this.openMockPanel('关于 BitFun', 'BitFun Mobile\n版本 0.1.0\n设置页交互当前为 mock。');
      })
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 24 })
  }

  @Builder
  LogoutCard() {
    Column() {
      this.LogoutRow()
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 10 })
  }

  @Builder
  LoginCard() {
    Column() {
      this.LoginRow()
    }
    .width('100%')
    .padding({ left: 0, right: 0, top: 5, bottom: 5 })
    .backgroundColor(CARD)
    .borderRadius(24)
    .hitTestBehavior(HitTestMode.Transparent)
    .margin({ bottom: 10 })
  }

  @Builder
  SettingsRow(icon: Resource, title: string, value: string, showChevron: boolean, action: () => void) {
    Column() {
      Row({ space: 18 }) {
        Image(icon)
          .width(23)
          .height(23)
          .objectFit(ImageFit.Contain)
        Text(title)
          .fontSize(19)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (value.length > 0) {
          Text(value)
            .fontSize(19)
            .fontWeight(FontWeight.Regular)
            .fontColor(SETTINGS_ROW_VALUE)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .constraintSize({ maxWidth: 176 })
        }
        if (showChevron) {
          Image($r('app.media.settings_chevron_right'))
            .width(10)
            .height(14)
            .objectFit(ImageFit.Contain)
        }
      }
      .width('100%')
      .height(52)
      .padding({ left: 22, right: 22 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .height(52)
    .padding(0)
    .backgroundColor('#00000000')
    .borderRadius(0)
    .onClick(action)
    .priorityGesture(TapGesture({ count: 1 }).onAction((_event: GestureEvent) => {
      action();
    }))
  }

  @Builder
  LogoutRow() {
    Column() {
      Row({ space: 18 }) {
        Image($r('app.media.settings_logout_arrow'))
          .width(23)
          .height(23)
          .objectFit(ImageFit.Contain)
        Text('退出登录')
          .fontSize(19)
          .fontWeight(FontWeight.Medium)
          .fontColor(SETTINGS_DANGER)
        Blank()
      }
      .width('100%')
      .height(52)
      .padding({ left: 22, right: 22 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .height(52)
    .padding(0)
    .backgroundColor('#00000000')
    .borderRadius(0)
    .onClick(() => {
      this.showLogoutConfirm = true;
    })
    .priorityGesture(TapGesture({ count: 1 }).onAction((_event: GestureEvent) => {
      this.showLogoutConfirm = true;
    }))
  }

  @Builder
  LoginRow() {
    Column() {
      Row({ space: 18 }) {
        Image($r('app.media.settings_logout_arrow'))
          .width(23)
          .height(23)
          .objectFit(ImageFit.Contain)
        Text('登录')
          .fontSize(19)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Image($r('app.media.settings_chevron_right'))
          .width(10)
          .height(14)
          .objectFit(ImageFit.Contain)
      }
      .width('100%')
      .height(52)
      .padding({ left: 22, right: 22 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .height(52)
    .padding(0)
    .backgroundColor('#00000000')
    .borderRadius(0)
    .onClick(() => {
      this.isMockSignedIn = true;
      this.openMockPanel('登录', '已进入模拟登录态。这里之后会接入真实账号登录流程。');
    })
    .priorityGesture(TapGesture({ count: 1 }).onAction((_event: GestureEvent) => {
      this.isMockSignedIn = true;
      this.openMockPanel('登录', '已进入模拟登录态。这里之后会接入真实账号登录流程。');
    }))
  }

  @Builder
  AppearanceRow() {
    Column() {
      Row({ space: 18 }) {
        Image($r('app.media.settings_appearance_sun_visible'))
          .width(23)
          .height(23)
          .objectFit(ImageFit.Contain)
        Text('外观')
          .fontSize(19)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text(this.selectedTheme)
          .fontSize(18)
          .fontColor(SETTINGS_ROW_VALUE)
        Image($r('app.media.settings_appearance_up_down_chevrons_visible'))
          .width(12)
          .height(16)
          .objectFit(ImageFit.Contain)
      }
      .width('100%')
      .height(52)
      .padding({ left: 22, right: 22 })
      .alignItems(VerticalAlign.Center)
    }
    .width('100%')
    .height(52)
    .padding(0)
    .backgroundColor('#00000000')
    .borderRadius(0)
    .onClick(() => {
      this.showThemePicker = true;
    })
    .priorityGesture(TapGesture({ count: 1 }).onAction((_event: GestureEvent) => {
      this.showThemePicker = true;
    }))
  }

  @Builder
  SettingsDivider() {
    Text('')
      .width('76%')
      .height(1)
      .backgroundColor(LINE)
      .margin({ left: 66 })
  }

  @Builder
  MockPanel() {
    Stack({ alignContent: Alignment.Center }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#33000000')
        .onClick(() => {
          this.showMockPanel = false;
        })

      Column({ space: 14 }) {
        Text(this.mockPanelTitle)
          .fontSize(21)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
        Text(this.mockPanelBody)
          .fontSize(15)
          .lineHeight(22)
          .fontColor(SETTINGS_ROW_VALUE)
          .width('100%')
        Button('知道了')
          .width('100%')
          .height(46)
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor(CARD)
          .backgroundColor(INK)
          .borderRadius(23)
          .onClick(() => {
            this.showMockPanel = false;
          })
      }
      .width('86%')
      .padding({ left: 22, right: 22, top: 22, bottom: 20 })
      .backgroundColor(CARD)
      .borderRadius(24)
      .shadow({ radius: 26, color: '#18000000', offsetY: 10 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  ThemePicker() {
    Stack({ alignContent: Alignment.Bottom }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#33000000')
        .onClick(() => {
          this.showThemePicker = false;
        })

      Column({ space: 0 }) {
        Text('外观')
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
          .margin({ bottom: 10 })
        this.ThemeOption('系统')
        this.SettingsDivider()
        this.ThemeOption('浅色')
        this.SettingsDivider()
        this.ThemeOption('深色')
      }
      .width('100%')
      .padding({ left: 18, right: 18, top: 18, bottom: 26 })
      .backgroundColor(CARD)
      .borderRadius({ topLeft: 28, topRight: 28 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  ThemeOption(label: string) {
    Column() {
      Row() {
        Text(label)
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        if (this.selectedTheme === label) {
          Text('✓')
            .fontSize(20)
            .fontWeight(FontWeight.Bold)
            .fontColor(INK)
        }
      }
      .width('100%')
      .height(52)
      .padding({ left: 10, right: 10 })
    }
    .width('100%')
    .height(52)
    .padding(0)
    .backgroundColor('#00000000')
    .borderRadius(0)
    .onClick(() => {
      this.selectedTheme = label;
      this.showThemePicker = false;
    })
    .priorityGesture(TapGesture({ count: 1 }).onAction((_event: GestureEvent) => {
      this.selectedTheme = label;
      this.showThemePicker = false;
    }))
  }

  @Builder
  LogoutConfirm() {
    Stack({ alignContent: Alignment.Bottom }) {
      Text('')
        .width('100%')
        .height('100%')
        .backgroundColor('#33000000')
        .onClick(() => {
          this.showLogoutConfirm = false;
        })

      Column({ space: 12 }) {
        Text('退出登录？')
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor(INK)
          .width('100%')
          .textAlign(TextAlign.Center)
        Text('这是 mock 流程，只会清空当前设置页里的模拟登录态。')
          .fontSize(14)
          .lineHeight(20)
          .fontColor(SETTINGS_ROW_VALUE)
          .width('100%')
          .textAlign(TextAlign.Center)
          .margin({ bottom: 4 })
        Button('退出登录')
          .width('100%')
          .height(48)
          .fontSize(17)
          .fontWeight(FontWeight.Medium)
          .fontColor(CARD)
          .backgroundColor(SETTINGS_DANGER)
          .borderRadius(24)
          .onClick(() => {
            this.isMockSignedIn = false;
            this.showLogoutConfirm = false;
          })
        Button('取消')
          .width('100%')
          .height(48)
          .fontSize(17)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .backgroundColor('#EFEFF1')
          .borderRadius(24)
          .onClick(() => {
            this.showLogoutConfirm = false;
          })
      }
      .width('100%')
      .padding({ left: 18, right: 18, top: 20, bottom: 28 })
      .backgroundColor(CARD)
      .borderRadius({ topLeft: 28, topRight: 28 })
    }
    .width('100%')
    .height('100%')
  }
}
