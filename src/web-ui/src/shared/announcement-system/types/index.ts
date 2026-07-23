/**
 * Announcement system types.
 *
 * Mirror of the Rust types in `core/src/service/announcement/types.rs`.
 * Keep these in sync when the Rust structs change.
 */

export type CardType = 'feature' | 'news' | 'tip' | 'announcement';

export type CardSource = 'local' | 'remote' | 'builtin_tip';

export type TriggerConditionType =
  | 'version_first_open'
  | 'app_nth_open'
  | 'feature_used'
  | 'manual'
  | 'always';

export interface TriggerConditionVersionFirstOpen {
  type: 'version_first_open';
}

export interface TriggerConditionAppNthOpen {
  type: 'app_nth_open';
  n: number;
}

export interface TriggerConditionFeatureUsed {
  type: 'feature_used';
  feature: string;
}

export interface TriggerConditionManual {
  type: 'manual';
}

export interface TriggerConditionAlways {
  type: 'always';
}

export type TriggerCondition =
  | TriggerConditionVersionFirstOpen
  | TriggerConditionAppNthOpen
  | TriggerConditionFeatureUsed
  | TriggerConditionManual
  | TriggerConditionAlways;

export interface TriggerRule {
  condition: TriggerCondition;
  delay_ms: number;
  once_per_version: boolean;
}

export interface ToastConfig {
  icon: string;
  title: string;
  description: string;
  action_label: string;
  dismissible: boolean;
  auto_dismiss_ms: number | null;
}

export type ModalSize = 'sm' | 'md' | 'lg' | 'xl';
export type CompletionAction = 'dismiss' | 'never_show_again';
export type PageLayout = 'text_only' | 'media_left' | 'media_right' | 'media_top' | 'fullscreen_media';
export type MediaType = 'lottie' | 'video' | 'image' | 'gif';

export interface MediaConfig {
  media_type: MediaType;
  /** Relative path under `public/announcements/` or HTTPS URL */
  src: string;
}

export interface ModalPage {
  layout: PageLayout;
  title: string;
  body: string;
  media: MediaConfig | null;
}

export interface ModalConfig {
  size: ModalSize;
  closable: boolean;
  pages: ModalPage[];
  completion_action: CompletionAction;
}

export interface AnnouncementCard {
  id: string;
  card_type: CardType;
  source: CardSource;
  app_version: string | null;
  priority: number;
  trigger: TriggerRule;
  toast: ToastConfig;
  modal: ModalConfig | null;
  expires_at: number | null;
}
