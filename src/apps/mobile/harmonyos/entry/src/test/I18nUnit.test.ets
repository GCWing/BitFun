import { describe, expect, it } from '@ohos/hypium';
import { AppLocale, APP_LOCALE_EN_US, APP_LOCALE_ZH_CN } from '../main/ets/i18n/AppLocale';
import { RemoteI18n } from '../main/ets/i18n/RemoteI18n';
import { ConnectionErrorPolicy } from '../main/ets/services/ConnectionErrorPolicy';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../main/ets/services/general-chat/GeneralChatServiceState';
import { TimeFormat } from '../main/ets/services/TimeFormat';

function restoreZhCn(): void {
  RemoteI18n.setLanguage(APP_LOCALE_ZH_CN);
}

export default function i18nUnitTest() {
  describe('HarmonyOS i18n', () => {
    it('resolves canonical locale ids and aliases', 0, () => {
      expect(AppLocale.resolve('zh')).assertEqual(APP_LOCALE_ZH_CN);
      expect(AppLocale.resolve('zh-Hans')).assertEqual(APP_LOCALE_ZH_CN);
      expect(AppLocale.resolve('zh_CN')).assertEqual(APP_LOCALE_ZH_CN);
      expect(AppLocale.resolve('en')).assertEqual(APP_LOCALE_EN_US);
      expect(AppLocale.resolve('en-GB')).assertEqual(APP_LOCALE_EN_US);
      expect(AppLocale.resolve('fr-FR')).assertEqual('');
    });

    it('prefers a stored language over the system language', 0, () => {
      expect(AppLocale.initial('en-US', 'zh-CN')).assertEqual(APP_LOCALE_EN_US);
      expect(AppLocale.initial('', 'en')).assertEqual(APP_LOCALE_EN_US);
      expect(AppLocale.initial('', 'ja-JP')).assertEqual(APP_LOCALE_ZH_CN);
    });

    it('keeps zh-CN and en-US catalogs in key lockstep', 0, () => {
      const zhKeys = RemoteI18n.keys(APP_LOCALE_ZH_CN).sort();
      const enKeys = RemoteI18n.keys(APP_LOCALE_EN_US).sort();
      expect(zhKeys.length).assertEqual(enKeys.length);
      expect(zhKeys.join('\n')).assertEqual(enKeys.join('\n'));
      expect(zhKeys.indexOf('settings.language.title') >= 0).assertTrue();
    });

    it('switches in-app copy between Chinese and English', 0, () => {
      restoreZhCn();
      expect(RemoteI18n.t('settings.title')).assertEqual('设置');
      expect(RemoteI18n.t('settings.modelService.section')).assertEqual('通用');
      expect(RemoteI18n.t('remote.title')).assertEqual('远程');
      RemoteI18n.setLanguage(APP_LOCALE_EN_US);
      expect(RemoteI18n.language()).assertEqual(APP_LOCALE_EN_US);
      expect(RemoteI18n.t('settings.title')).assertEqual('Settings');
      expect(RemoteI18n.t('settings.modelService.section')).assertEqual('General');
      expect(RemoteI18n.t('remote.title')).assertEqual('Remote');
      expect(RemoteI18n.f('sidebar.moreSessions', '3')).assertEqual('3 more sessions');
      restoreZhCn();
    });

    it('falls back to the other catalog when a key is missing in the current language', 0, () => {
      restoreZhCn();
      expect(RemoteI18n.translate(APP_LOCALE_EN_US, 'settings.title')).assertEqual('Settings');
      expect(RemoteI18n.t('missing.key.does.not.exist')).assertEqual('missing.key.does.not.exist');
    });

    it('matches error and service copy across both locales', 0, () => {
      restoreZhCn();
      const zhAuth = RemoteI18n.translate(APP_LOCALE_ZH_CN, 'generalChat.authenticationFailed');
      const enAuth = RemoteI18n.translate(APP_LOCALE_EN_US, 'generalChat.authenticationFailed');
      expect(RemoteI18n.textEquals('generalChat.authenticationFailed', zhAuth)).assertTrue();
      expect(RemoteI18n.textEquals('generalChat.authenticationFailed', enAuth)).assertTrue();
      expect(GeneralChatServiceStatus.fromError(zhAuth)).assertEqual(GeneralChatServiceState.AuthExpired);
      expect(GeneralChatServiceStatus.fromError(enAuth)).assertEqual(GeneralChatServiceState.AuthExpired);
      expect(ConnectionErrorPolicy.failureKindFor(
        RemoteI18n.translate(APP_LOCALE_EN_US, 'errors.roomNotFound')
      )).assertEqual('expired_room');
      expect(ConnectionErrorPolicy.errorText(new Error(
        RemoteI18n.translate(APP_LOCALE_ZH_CN, 'errors.userIdRequired')
      ))).assertEqual(RemoteI18n.translate(APP_LOCALE_ZH_CN, 'errors.userIdRequired'));
    });

    it('retranslates stored status and templated copy when the language changes', 0, () => {
      restoreZhCn();
      const switching = RemoteI18n.f('remote.settings.deviceSwitching', 'Desk-1');
      expect(switching).assertEqual('正在切换到 Desk-1…');
      RemoteI18n.setLanguage(APP_LOCALE_EN_US);
      expect(RemoteI18n.retranslate(switching)).assertEqual('Switching to Desk-1…');
      expect(RemoteI18n.retranslate(RemoteI18n.translate(APP_LOCALE_ZH_CN, 'common.back')))
        .assertEqual('Back');
      expect(RemoteI18n.retranslateKnown('未连接', ['status.notConnected'])).assertEqual('Not connected');
      expect(RemoteI18n.retranslate('未连接')).assertEqual('未连接');
      restoreZhCn();
      expect(RemoteI18n.retranslate(RemoteI18n.translate(APP_LOCALE_EN_US, 'common.back')))
        .assertEqual('返回');
    });

    it('formats relative and absolute times in the active locale', 0, () => {
      const now = Date.parse('2026-06-10T12:00:00.000Z');
      restoreZhCn();
      expect(TimeFormat.relative('2026-06-10T11:59:45.000Z', now)).assertEqual(RemoteI18n.t('time.justNow'));
      expect(TimeFormat.relative('2026-06-10T11:42:00.000Z', now)).assertEqual(RemoteI18n.f('time.minutesAgo', '18'));
      expect(TimeFormat.relative('2026-05-30T12:00:00.000Z', now)).assertEqual('2026年05月30日');
      RemoteI18n.setLanguage(APP_LOCALE_EN_US);
      expect(TimeFormat.relative('2026-06-10T11:42:00.000Z', now)).assertEqual(RemoteI18n.f('time.minutesAgo', '18'));
      expect(TimeFormat.relative('2026-05-30T12:00:00.000Z', now)).assertEqual('05/30/2026');
      restoreZhCn();
    });

    it('maps speech recognition language to the active locale', 0, () => {
      restoreZhCn();
      expect(RemoteI18n.speechLanguage()).assertEqual(APP_LOCALE_ZH_CN);
      expect(RemoteI18n.speechLocate()).assertEqual('CN');
      RemoteI18n.setLanguage(APP_LOCALE_EN_US);
      expect(RemoteI18n.speechLanguage()).assertEqual(APP_LOCALE_EN_US);
      expect(RemoteI18n.speechLocate()).assertEqual('US');
      restoreZhCn();
    });
  });
}
