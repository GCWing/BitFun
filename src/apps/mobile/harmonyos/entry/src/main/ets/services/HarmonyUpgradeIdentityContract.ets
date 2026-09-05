/**
 * Stable HarmonyOS package and storage identifiers retained across the
 * product-name change.
 *
 * These values are not display branding. Changing any of them strands data
 * written by an installed release (and changing the bundle id prevents an
 * in-place upgrade altogether), so all compatibility-only identifiers live at
 * this single boundary instead of leaking through product code.
 */
export class HarmonyUpgradeIdentityContract {
  static readonly APP_BUNDLE: string = 'com.bitfun.app';
  static readonly REMOTE_IDENTITY_STORE: string = 'bitfun_remote_identity';
  static readonly CLOUD_ACCOUNT_STORE: string = 'bitfun_cloud_account';
  static readonly CLOUD_ACCOUNT_HUKS_ALIAS: string = 'bitfun_cloud_account_session';
  static readonly APP_LOCALE_STORE: string = 'bitfun_app_locale';
  static readonly REMOTE_CHAT_DATABASE: string = 'bitfun_remote_chat.db';
  static readonly REMOTE_SESSIONS_DATABASE: string = 'bitfun_remote_sessions.db';
  static readonly GENERAL_CHAT_CONFIG_STORE: string = 'bitfun_general_chat_config';
  static readonly GENERAL_CHAT_HUKS_ALIAS: string = 'bitfun_general_chat_api_key';
  static readonly GENERAL_CHAT_DATABASE: string = 'bitfun_general_chat.db';
  static readonly WATCH_HANDOFF_STORE: string = 'bitfun_harmony_handoff_s1';
  static readonly WATCH_PROVISION_REQUEST_KEY: string = 'bitfun.account.provision.request';
  static readonly WATCH_PROVISION_RESPONSE_KEY: string = 'bitfun.account.provision.response';
}
