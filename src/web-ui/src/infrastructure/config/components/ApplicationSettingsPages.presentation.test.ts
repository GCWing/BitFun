import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
  fileURLToPath(new URL('./ApplicationSettingsPages.tsx', import.meta.url)),
  'utf8',
);

describe('Application settings presentation', () => {
  it('groups general settings by task instead of rendering one section per row', () => {
    expect(source).toContain("applicationGroups.startupAndUpdates.title");
    expect(source).toContain("applicationGroups.windowAndNotifications.title");
    expect(source).toContain('<LaunchAtLoginSetting />');
    expect(source).toContain('<PreventSleepSetting />');
    expect(source).toContain('<AutoUpdateSetting />');
    expect(source).toContain('<WindowBehaviorSetting />');
    expect(source).toContain('<NotificationSettings />');
    expect(source).not.toContain('function LaunchAtLoginSection');
    expect(source).not.toContain('function NotificationsSection');
  });

  it('does not render an empty desktop-only startup group on web', () => {
    expect(source).toContain('{isTauri && (');
  });

  it('keeps the previous-exit notice outside the settings section surface', () => {
    const loggingSection = source.match(
      /function LoggingSection\(\)[\s\S]*?\r?\n}\r?\n\r?\nfunction TerminalSection/,
    )?.[0] ?? '';
    const noticeStart = loggingSection.indexOf(
      '{runtimeInfo?.previousUnexpectedExit?.detected && (',
    );
    const settingsSectionStart = loggingSection.indexOf('<ConfigPageSection');
    const settingsSectionEnd = loggingSection.indexOf('</ConfigPageSection>');

    expect(loggingSection).not.toBe('');
    expect(noticeStart).toBeGreaterThanOrEqual(0);
    expect(settingsSectionStart).toBeGreaterThan(noticeStart);
    expect(settingsSectionEnd).toBeGreaterThan(settingsSectionStart);
    expect(
      loggingSection.slice(settingsSectionStart, settingsSectionEnd),
    ).not.toContain('previousUnexpectedExit');
  });
});
