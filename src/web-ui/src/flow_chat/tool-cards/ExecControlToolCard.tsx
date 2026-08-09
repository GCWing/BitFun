import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { ExecProcessToolCardView } from './ExecProcessToolCardView';
import { buildExecControlCardModel } from './execProcessToolCardModel';

export const ExecControlToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  onExpand,
  isLastItem,
}) => {
  const { t } = useTranslation('flow-chat');
  const model = useMemo(
    () => buildExecControlCardModel(toolItem, t),
    [t, toolItem],
  );

  return (
    <ExecProcessToolCardView
      toolItem={toolItem}
      model={model}
      onExpand={onExpand}
      isLastItem={isLastItem}
    />
  );
};

export default ExecControlToolCard;
