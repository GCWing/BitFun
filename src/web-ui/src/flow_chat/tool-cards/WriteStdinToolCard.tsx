import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { ExecProcessToolCardView } from './ExecProcessToolCardView';
import { buildWriteStdinCardModel } from './execProcessToolCardModel';

export const WriteStdinToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand,
}) => {
  const { t } = useTranslation('flow-chat');
  const model = useMemo(
    () => buildWriteStdinCardModel(toolItem, t),
    [t, toolItem],
  );

  return (
    <ExecProcessToolCardView
      toolItem={toolItem}
      model={model}
      onExpand={onExpand}
    />
  );
};

export default WriteStdinToolCard;
