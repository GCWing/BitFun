import { describe, expect, it } from 'vitest';

import { I18nService } from './I18nService';
import { DEFAULT_LOCALE, WEB_UI_BOOTSTRAP_NAMESPACES } from '../presets';

describe('I18nService shared namespace contract', () => {
  it('keeps bootstrap translations available synchronously after construction', () => {
    const service = new I18nService();

    expect(service.t('common:actions.copy')).not.toBe('common:actions.copy');
  });

  it('keeps shared terms explicit so surface namespaces retain priority', () => {
    const service = new I18nService();
    const i18n = service.getI18nInstance();
    const locale = service.getCurrentLocale();

    i18n.addResource(locale, 'common', 'overrideProbe', 'surface label');
    i18n.addResource(locale, 'shared', 'overrideProbe', 'shared label');
    i18n.addResource(locale, 'shared', 'sharedOnlyProbe', 'shared-only label');

    expect(service.t('overrideProbe')).toBe('surface label');
    expect(service.t('shared:overrideProbe')).toBe('shared label');
    expect(service.t('sharedOnlyProbe')).toBe('sharedOnlyProbe');
    expect(service.t('shared:sharedOnlyProbe')).toBe('shared-only label');
  });

  it('uses the generated locale fallback chain before the global fallback locale', async () => {
    const service = new I18nService();
    const i18n = service.getI18nInstance();

    i18n.addResource('zh-CN', 'common', 'fallbackProbe', 'simplified fallback');
    i18n.addResource('en-US', 'common', 'fallbackProbe', 'english fallback');
    await i18n.changeLanguage('zh-TW');

    expect(service.t('fallbackProbe')).toBe('simplified fallback');
  });

  it('keeps non-bootstrap web-ui namespaces out of the startup resource bundle', async () => {
    const service = new I18nService();
    const i18n = service.getI18nInstance();

    for (const namespace of WEB_UI_BOOTSTRAP_NAMESPACES) {
      expect(i18n.hasResourceBundle(DEFAULT_LOCALE, namespace)).toBe(true);
    }
    expect(i18n.hasResourceBundle(DEFAULT_LOCALE, 'settings/basics')).toBe(false);

    await service.loadNamespace('settings/basics');

    expect(i18n.hasResourceBundle(DEFAULT_LOCALE, 'settings/basics')).toBe(true);
  });
});
