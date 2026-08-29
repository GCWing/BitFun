import React from 'react';
import {
  BadgeCheck,
  BookOpenCheck,
  CheckCircle2,
  Compass,
  FileText,
  Gavel,
  ListTree,
  Scale,
  SearchCheck,
  ShieldCheck,
  Swords,
  UsersRound,
  type LucideIcon,
} from 'lucide-react';
import { StatusPill, type StatusPillTone } from '@bitfun/ui';
import { AmbientToolCard, AmbientToolCardHeader } from '@bitfun/ui/flow-chat';
import { useI18n } from '@/infrastructure/i18n';
import type {
  DeepResearchPhaseId,
  DeepResearchProtocolMarker,
  DeepResearchVerdictStatus,
} from './deepResearchProtocol';
import './DeepResearchProtocolGroup.scss';

interface DeepResearchProtocolGroupProps {
  kind: DeepResearchProtocolMarker['kind'];
  markers: DeepResearchProtocolMarker[];
}

type PhaseMarker = Extract<DeepResearchProtocolMarker, { kind: 'phase' }>;
type SubquestionMarker = Extract<DeepResearchProtocolMarker, { kind: 'subquestion' }>;
type CitationMarker = Extract<DeepResearchProtocolMarker, { kind: 'citation' }>;
type VerdictMarker = Extract<DeepResearchProtocolMarker, { kind: 'verdict' }>;

const PHASE_PRESENTATION: Record<DeepResearchPhaseId, {
  current: number;
  icon: LucideIcon;
}> = {
  'phase-0-orient': { current: 1, icon: Compass },
  'phase-1-specialists': { current: 2, icon: UsersRound },
  'phase-2-citations': { current: 3, icon: BookOpenCheck },
  'phase-3-debate-r1': { current: 4, icon: Swords },
  'phase-3-debate-r2': { current: 5, icon: Scale },
  'phase-4-factcheck': { current: 6, icon: ShieldCheck },
  'phase-5-arbitration': { current: 7, icon: Gavel },
  'phase-6-report': { current: 8, icon: FileText },
  complete: { current: 8, icon: CheckCircle2 },
};

const PHASE_COUNT = 8;

function phaseLabel(phaseId: DeepResearchPhaseId, t: (key: string) => string): string {
  switch (phaseId) {
    case 'phase-0-orient':
      return t('deepResearchProtocol.phases.orient');
    case 'phase-1-specialists':
      return t('deepResearchProtocol.phases.specialists');
    case 'phase-2-citations':
      return t('deepResearchProtocol.phases.citations');
    case 'phase-3-debate-r1':
      return t('deepResearchProtocol.phases.debateRound1');
    case 'phase-3-debate-r2':
      return t('deepResearchProtocol.phases.debateRound2');
    case 'phase-4-factcheck':
      return t('deepResearchProtocol.phases.factCheck');
    case 'phase-5-arbitration':
      return t('deepResearchProtocol.phases.arbitration');
    case 'phase-6-report':
      return t('deepResearchProtocol.phases.report');
    case 'complete':
      return t('deepResearchProtocol.phases.complete');
  }
}

function verdictLabel(status: DeepResearchVerdictStatus, t: (key: string) => string): string {
  switch (status) {
    case 'DECIDED':
      return t('deepResearchProtocol.verdicts.status.decided');
    case 'CONTESTED':
      return t('deepResearchProtocol.verdicts.status.contested');
    case 'GAP':
      return t('deepResearchProtocol.verdicts.status.gap');
    case 'TENTATIVE':
      return t('deepResearchProtocol.verdicts.status.tentative');
  }
}

function verdictTone(status: DeepResearchVerdictStatus): StatusPillTone {
  switch (status) {
    case 'DECIDED':
      return 'success';
    case 'CONTESTED':
    case 'TENTATIVE':
      return 'warning';
    case 'GAP':
      return 'danger';
  }
}

const PhaseGroup: React.FC<{ markers: PhaseMarker[] }> = ({ markers }) => {
  const { t, formatNumber } = useI18n('flow-chat');

  return (
    <div
      className="deep-research-protocol__phase-list"
      data-bf-component="deep-research-protocol"
      data-bf-part="phaseList"
    >
      {markers.map((marker, index) => {
        const presentation = PHASE_PRESENTATION[marker.phaseId];
        const Icon = presentation.icon;
        const complete = marker.phaseId === 'complete';
        const progress = t('deepResearchProtocol.phaseProgress', {
          current: formatNumber(presentation.current),
          total: formatNumber(PHASE_COUNT),
        });

        return (
          <AmbientToolCard
            className="deep-research-protocol__card deep-research-protocol__phase"
            data-testid="deep-research-phase"
            key={`${marker.phaseId}:${index}`}
            status="completed"
            header={(
              <AmbientToolCardHeader
                icon={<Icon aria-hidden="true" />}
                content={phaseLabel(marker.phaseId, t)}
                extra={(
                  <StatusPill tone={complete ? 'success' : 'accent'}>
                    {complete ? t('shared:statuses.done') : progress}
                  </StatusPill>
                )}
              />
            )}
          />
        );
      })}
    </div>
  );
};

const SubquestionGroup: React.FC<{ markers: SubquestionMarker[] }> = ({ markers }) => {
  const { t, formatNumber } = useI18n('flow-chat');
  const count = formatNumber(markers.length);

  return (
    <AmbientToolCard
      className="deep-research-protocol__card"
      data-testid="deep-research-subquestions"
      status="completed"
      isExpanded
      header={(
        <AmbientToolCardHeader
          icon={<ListTree aria-hidden="true" />}
          content={t('deepResearchProtocol.subquestions.title')}
          extra={<StatusPill tone="info">{t('deepResearchProtocol.subquestions.count', { count })}</StatusPill>}
        />
      )}
      expandedContent={(
        <ol
          aria-label={t('deepResearchProtocol.subquestions.title')}
          className="deep-research-protocol__list"
          data-bf-component="deep-research-protocol"
          data-bf-part="list"
        >
          {markers.map(marker => (
            <li
              className="deep-research-protocol__list-item"
              data-bf-component="deep-research-protocol"
              data-bf-part="listItem"
              key={marker.id}
            >
              <StatusPill tone="neutral">{marker.id}</StatusPill>
              <span
                className="deep-research-protocol__item-title"
                data-bf-component="deep-research-protocol"
                data-bf-part="itemTitle"
              >
                {marker.title}
              </span>
            </li>
          ))}
        </ol>
      )}
    />
  );
};

const CitationGroup: React.FC<{ markers: CitationMarker[] }> = ({ markers }) => {
  const { t, formatNumber } = useI18n('flow-chat');
  const count = formatNumber(markers.length);
  const corroboratedCount = markers.filter(marker => marker.corroborated).length;
  const corroborated = corroboratedCount > 0
    ? t('deepResearchProtocol.citations.corroborated', {
        count: formatNumber(corroboratedCount),
      })
    : undefined;

  return (
    <AmbientToolCard
      className="deep-research-protocol__card"
      data-testid="deep-research-citations"
      status="completed"
      header={(
        <AmbientToolCardHeader
          icon={<SearchCheck aria-hidden="true" />}
          content={t('deepResearchProtocol.citations.title')}
          extra={(
            <span
              className="deep-research-protocol__summary"
              data-bf-component="deep-research-protocol"
              data-bf-part="summary"
            >
              {corroborated && <span>{corroborated}</span>}
              <StatusPill tone="info">{t('deepResearchProtocol.citations.count', { count })}</StatusPill>
            </span>
          )}
        />
      )}
    />
  );
};

const VerdictGroup: React.FC<{ markers: VerdictMarker[] }> = ({ markers }) => {
  const { t, formatNumber } = useI18n('flow-chat');
  const count = formatNumber(markers.length);

  return (
    <AmbientToolCard
      className="deep-research-protocol__card"
      data-testid="deep-research-verdicts"
      status="completed"
      isExpanded
      header={(
        <AmbientToolCardHeader
          icon={<BadgeCheck aria-hidden="true" />}
          content={t('deepResearchProtocol.verdicts.title')}
          extra={<StatusPill tone="success">{t('deepResearchProtocol.verdicts.count', { count })}</StatusPill>}
        />
      )}
      expandedContent={(
        <ul
          aria-label={t('deepResearchProtocol.verdicts.title')}
          className="deep-research-protocol__list"
          data-bf-component="deep-research-protocol"
          data-bf-part="list"
        >
          {markers.map(marker => (
            <li
              className="deep-research-protocol__list-item"
              data-bf-component="deep-research-protocol"
              data-bf-part="listItem"
              key={marker.subquestionId}
            >
              <StatusPill tone="neutral">{marker.subquestionId}</StatusPill>
              <StatusPill tone={verdictTone(marker.status)}>
                {verdictLabel(marker.status, t)}
              </StatusPill>
              <span
                className="deep-research-protocol__confidence"
                data-bf-component="deep-research-protocol"
                data-bf-part="confidence"
              >
                {t('deepResearchProtocol.verdicts.confidence', {
                  value: formatNumber(marker.confidence, {
                    style: 'percent',
                    maximumFractionDigits: 0,
                  }),
                })}
              </span>
            </li>
          ))}
        </ul>
      )}
    />
  );
};

export const DeepResearchProtocolGroup: React.FC<DeepResearchProtocolGroupProps> = ({
  kind,
  markers,
}) => {
  let content: React.ReactNode;

  switch (kind) {
    case 'phase':
      content = <PhaseGroup markers={markers.filter((marker): marker is PhaseMarker => marker.kind === 'phase')} />;
      break;
    case 'subquestion':
      content = <SubquestionGroup markers={markers.filter((marker): marker is SubquestionMarker => marker.kind === 'subquestion')} />;
      break;
    case 'citation':
      content = <CitationGroup markers={markers.filter((marker): marker is CitationMarker => marker.kind === 'citation')} />;
      break;
    case 'verdict':
      content = <VerdictGroup markers={markers.filter((marker): marker is VerdictMarker => marker.kind === 'verdict')} />;
      break;
  }

  return (
    <div
      className="deep-research-protocol__group"
      data-bf-component="deep-research-protocol"
      data-bf-part="root"
    >
      {content}
    </div>
  );
};
