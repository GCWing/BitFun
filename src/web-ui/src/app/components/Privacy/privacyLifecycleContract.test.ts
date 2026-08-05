import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const readSource = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8').replace(/\r\n?/g, '\n');

describe('OpenHarmony privacy lifecycle contract', () => {
  it('mounts business providers while the privacy choice is still visible', () => {
    const main = readSource('../../../main.tsx');
    const business = readSource('../../BusinessApplication.tsx');

    expect(main).toContain('<PrivacyGate>');
    expect(main).toContain('<BusinessApplication />');
    expect(main).not.toContain('onBusinessAuthorized');
    expect(business).toContain('<WorkspaceProvider>');
    expect(business).toContain('<App />');
  });

  it('keeps close separate from a persisted not-accepted choice', () => {
    const gate = readSource('./PrivacyGate.tsx');

    expect(gate).toContain('const dismiss = useCallback');
    expect(gate).toContain('setDismissed(true)');
    expect(gate).toContain('onClose={dismiss}');
    expect(gate).toContain('await enterNotAccepted');
    expect(gate).not.toContain('quitApp');
  });

  it('shows the first consent choice in the same modal size as the managed statement', () => {
    const gate = readSource('./PrivacyGate.tsx');
    const dialog = readSource('./PrivacyStatementDialog.tsx');

    expect(gate).toContain('size="xlarge"');
    expect(gate).toContain(
      'contentClassName="bitfun-privacy-dialog bitfun-privacy-consent-dialog"',
    );
    expect(dialog).toContain('size="xlarge"');
    expect(gate).not.toContain('className="bitfun-privacy-gate"');
  });

  it('uses the explicit lifecycle and collection-policy command surface', () => {
    const api = readSource('../../../infrastructure/api/service-api/PrivacyAPI.ts');
    const nativeApi = readSource('../../../../../apps/desktop/src/api/privacy_api.rs');

    for (const command of [
      'privacy_initialize',
      'privacy_get_status',
      'privacy_accept',
      'privacy_enter_not_accepted',
      'privacy_mark_viewed',
      'privacy_apply_collection_policy',
    ]) {
      expect(`${api}\n${nativeApi}`).toContain(command);
    }
    expect(`${api}\n${nativeApi}`).not.toContain('privacy_withdraw');
    expect(`${api}\n${nativeApi}`).not.toContain('privacy_release_business_integrations');
  });

  it('scopes the collection policy to feedback instead of disabling product capabilities', () => {
    const desktop = readSource('../../../../../apps/desktop/src/lib.rs');
    const privacyApi = readSource('../../../../../apps/desktop/src/api/privacy_api.rs');
    const feedbackApi = readSource('../../../../../apps/desktop/src/api/feedback_api.rs');
    const privacy = readSource('../../../../../crates/services/services-integrations/src/privacy/mod.rs');
    const capabilitySources = [
      'agentic_api.rs',
      'announcement_api.rs',
      'btw_api.rs',
      'commands.rs',
      'editor_ai_api.rs',
      'miniapp_agent_api.rs',
      'remote_connect_api.rs',
      'startchat_agent_api.rs',
      'system_api.rs',
    ].map(file => readSource(`../../../../../apps/desktop/src/api/${file}`));

    expect(desktop).toContain('PrivacyServiceState::enabled(');
    expect(desktop).toContain('remote_connect_api::init_on_startup();');
    expect(desktop).not.toContain('if privacy_state.collection_allowed()');
    expect(privacy).toContain('PrivacyCollectionPolicy::new(false)');
    expect(feedbackApi).toContain('!privacy_state.collection_allowed()');
    expect(privacyApi).not.toContain('require_collection_allowed');
    expect(privacyApi).not.toContain('suspend_for_privacy');
    for (const source of capabilitySources) {
      expect(source).not.toContain('require_collection_allowed');
    }
  });

  it('requests calendar permission only when calendar is used and does not auto-update at startup', () => {
    const entryAbility = readSource('../../../../../apps/ohos/entry/src/main/ets/entryability/EntryAbility.ets');
    const startup = entryAbility.slice(
      entryAbility.indexOf('onWindowStageCreate'),
      entryAbility.indexOf("registerArktsFunction('call_calendar'"),
    );
    const calendar = entryAbility.slice(
      entryAbility.indexOf("registerArktsFunction('call_calendar'"),
      entryAbility.indexOf("registerArktsFunction('call_harmony_build'"),
    );

    expect(startup).not.toContain('requestPermissionsFromUser');
    expect(calendar).toContain('requestPermissionsFromUser');
    expect(entryAbility).not.toContain('this.appUpdater.check');
  });

  it('renders resource failure without an agreement action', () => {
    const gate = readSource('./PrivacyGate.tsx');
    const errorView = gate.slice(
      gate.indexOf('testId="privacy-resource-error"'),
      gate.indexOf('testId="privacy-consent-gate"'),
    );

    expect(errorView).toContain('copy.closeAndContinue');
    expect(errorView).toContain('copy.retry');
    expect(errorView).not.toContain('handleAccept');
  });
});
