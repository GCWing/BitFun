import React from 'react';
import type { MiniAppDraft } from '@/infrastructure/api/service-api/MiniAppAPI';
import MiniAppRunner from '../components/MiniAppRunner';

interface MiniAppDraftPreviewProps {
  draft: MiniAppDraft;
  previewKey: number;
}

export const MiniAppDraftPreview: React.FC<MiniAppDraftPreviewProps> = ({ draft, previewKey }) => {
  return (
    <div className="miniapp-customize-panel__preview-frame">
      <MiniAppRunner
        key={`${draft.appId}-${draft.draftId}-${previewKey}`}
        app={draft.app}
        runScope={{ kind: 'draft', appId: draft.appId, draftId: draft.draftId }}
      />
    </div>
  );
};

export default MiniAppDraftPreview;
