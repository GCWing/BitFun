import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const readSource = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n?/g, '\n');

describe('OpenHarmony privacy policy management contract', () => {
  it('offers managed full, not-accepted, and read-only detail modes', () => {
    const dialog = readSource('./PrivacyStatementDialog.tsx');
    const about = readSource('../AboutDialog/AboutDialog.tsx');

    expect(dialog).toContain("variant?: 'about' | 'readonly'");
    expect(dialog).toContain("t('privacy.withdraw')");
    expect(dialog).toContain("t('privacy.enableFull')");
    expect(dialog).toContain("variant === 'about'");
    expect(about).toContain("setSubDialog('privacy')");
    expect(about).toContain('privacyStatus.hasUnreadUpdate');
  });

  it('keeps collection disabled when withdrawal persistence or full-mode application fails', () => {
    const native = readSource('../../../../../apps/desktop/src/api/privacy_api.rs');
    const dialog = readSource('./PrivacyStatementDialog.tsx');
    const gate = readSource('./PrivacyGate.tsx');

    const withdraw = native.slice(
      native.indexOf('pub async fn privacy_enter_not_accepted'),
      native.indexOf('pub async fn privacy_mark_viewed'),
    );
    expect(withdraw.indexOf('state.enter_not_accepted_mode()?')).toBeLessThan(
      withdraw.indexOf('.enter_not_accepted('),
    );
    expect(withdraw).not.toContain('suspend_for_privacy');
    expect(dialog).toContain("operationError === 'withdraw'");
    expect(dialog).toContain("operationError === 'apply'");
    expect(gate).toContain('applyRetryRequired');
    expect(gate).toContain("applyCollectionPolicy('full', locale)");
    expect(`${native}\n${dialog}`).not.toContain('quitApp');
  });

  it('returns to the main page after a managed consent mode change succeeds', () => {
    const dialog = readSource('./PrivacyStatementDialog.tsx');
    const about = readSource('../AboutDialog/AboutDialog.tsx');

    expect(dialog).toContain('onModeChangeComplete?: () => void');
    expect(dialog.match(/onModeChangeComplete\?\.\(\)/g)).toHaveLength(3);
    expect(about).toContain('const closeAfterPrivacyModeChange = useCallback');
    expect(about).toContain('setSubDialog(null);\n    onClose();');
    expect(about).toContain('onModeChangeComplete={closeAfterPrivacyModeChange}');
  });

  it('uses only the policy timestamp for editorial update state', () => {
    const service = readSource(
      '../../../../../crates/services/services-integrations/src/privacy/mod.rs',
    );

    expect(service).toContain('PrivacyChangeType::Editorial');
    expect(service).not.toContain('const POLICY_VERSION');
    expect(service).toContain('const CONSENT_VERSION: &str = "4"');
    expect(service).toContain(
      'state.viewed_policy_updated_at.as_deref() != Some(POLICY_UPDATED_AT)',
    );
    expect(service).toContain('new_state_persists_timestamps_without_policy_versions');
    expect(service).not.toContain('accepted_policy_version');
    expect(service).not.toContain('viewed_policy_version');
  });

  it('projects privacy updates and feedback replies onto the home more-options indicator', () => {
    const footer = readSource('../NavPanel/components/PersistentFooterActions.tsx');
    const styles = readSource('../NavPanel/NavPanel.scss');

    expect(footer).toContain('privacyStatus.hasUnreadUpdate');
    expect(footer).toContain('hasUnreadFeedback || hasPrivacyUpdate');
    expect(footer).toContain('bitfun-nav-panel__footer-more-unread');
    expect(footer).toContain("t('privacy.aboutEntryUpdated')");
    expect(styles).toContain('.bitfun-nav-panel__footer-more-unread');
  });
});
