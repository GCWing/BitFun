import type {
  MiniAppDraft,
  MiniAppPermissionDiff,
} from '@/infrastructure/api/service-api/MiniAppAPI';

export type MiniAppCustomizationStage =
  | 'idle'
  | 'notice'
  | 'drafting'
  | 'preview'
  | 'permission-review'
  | 'applying';

export interface MiniAppCustomizationState {
  stage: MiniAppCustomizationStage;
  draft: MiniAppDraft | null;
  permissionDiff: MiniAppPermissionDiff | null;
  assistantSessionId: string | null;
  error: string | null;
}

export type MiniAppRunScope =
  | { kind: 'active'; appId: string }
  | { kind: 'draft'; appId: string; draftId: string };
