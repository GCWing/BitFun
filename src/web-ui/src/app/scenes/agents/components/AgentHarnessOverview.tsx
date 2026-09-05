import React from 'react';
import { Icon } from '@openbitfun/ui';
import { GalleryZone } from '@/app/components';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';

const HARNESS_STRATEGIES = [
  { id: 'minimal', icon: 'minimal', gear: 1 },
  { id: 'balanced', icon: 'standard', gear: 2 },
  { id: 'ultimate', icon: 'ultimate', gear: 3 },
  { id: 'creative', icon: 'creative', gear: 'creative' },
] as const;

type HarnessStrategyId = typeof HARNESS_STRATEGIES[number]['id'];

// Route coordinates describe execution structure; labels and icons keep their
// natural size while the connecting lines adapt to the available width.
const STRATEGY_ROUTES: Record<HarnessStrategyId, {
  paths: string[];
  nodes: Array<[number, number]>;
}> = {
  minimal: {
    paths: ['M 0 22 H 320'],
    nodes: [[160, 22]],
  },
  balanced: {
    paths: ['M 0 22 H 320'],
    nodes: [[64, 22], [128, 22], [192, 22], [256, 22]],
  },
  ultimate: {
    paths: [
      'M 0 22 H 48 C 68 22 70 7 92 7 H 222 C 242 7 245 22 267 22 H 320',
      'M 48 22 C 68 22 70 37 92 37 H 222 C 242 37 245 22 267 22',
    ],
    nodes: [[48, 22], [116, 7], [174, 7], [124, 37], [192, 37], [267, 22]],
  },
  creative: {
    paths: ['M 0 22 C 20 22 24 18 48 18 C 88 18 96 34 144 34 C 184 34 192 10 224 10 C 252 10 256 22 280 22 H 320'],
    nodes: [[48, 18], [144, 34], [280, 22]],
  },
};

function HarnessConnections({ converge = false }: { converge?: boolean }) {
  return (
    <div className="openbitfun-agents-scene__harness-connections" aria-hidden="true">
      <svg viewBox="0 0 88 176" preserveAspectRatio="none" focusable="false">
        {[22, 66, 110, 154].map(y => (
          <path
            key={y}
            d={converge
              ? `M 0 ${y} H 8 C 40 ${y} 48 88 72 88 H 88`
              : `M 0 88 H 8 C 40 88 48 ${y} 80 ${y} H 88`}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      {converge && (
        <Icon name="arrow-right" size="2xs" className="openbitfun-agents-scene__harness-result-arrow" />
      )}
      <Icon name="arrow-down" size="sm" className="openbitfun-agents-scene__harness-connection-arrow" />
    </div>
  );
}

function HarnessStrategyRoute({ profile }: { profile: HarnessStrategyId }) {
  const route = STRATEGY_ROUTES[profile];

  return (
    <div className="openbitfun-agents-scene__harness-route" aria-hidden="true">
      <svg viewBox="0 0 320 44" preserveAspectRatio="none" focusable="false">
        {route.paths.map(path => (
          <path key={path} d={path} vectorEffect="non-scaling-stroke" />
        ))}
      </svg>
      {route.nodes.map(([x, y]) => (
        <span
          key={`${x}-${y}`}
          className="openbitfun-agents-scene__harness-route-node"
          style={{ left: `${x / 320 * 100}%`, top: `${y / 44 * 100}%` }}
        >
          <Icon name="unselected" size="2xs" style={{ width: 9, height: 9 }} />
        </span>
      ))}
      {profile === 'minimal' && (
        <span className="openbitfun-agents-scene__harness-route-arrow">
          <Icon name="arrow-right" size="2xs" />
        </span>
      )}
    </div>
  );
}

const AgentHarnessOverview: React.FC = () => {
  const { t } = useI18n('scenes/agents');

  return (
    <div className="openbitfun-agents-scene__harness">
      <GalleryZone
        id="harness-zone"
        className="openbitfun-agents-scene__harness-zone"
        data-testid="agents-harness-zone"
        title={t('harnessZone.title')}
        subtitle={(
          <>
            <span>{t('harnessZone.subtitle')}</span>
            <span className="openbitfun-agents-scene__harness-caption">
              {t('harnessZone.flowCaption')}
            </span>
          </>
        )}
      >
        <div
          className="openbitfun-agents-scene__harness-presentation"
          role="group"
          aria-label={t('harnessZone.flowCaption')}
          data-openbitfun-scene="agents"
          data-openbitfun-part="harnessPresentation"
        >
          <span className="openbitfun-agents-scene__harness-endpoint">
            {t('harnessZone.task')}
          </span>
          <HarnessConnections />
          <div className="openbitfun-agents-scene__harness-profiles">
            {HARNESS_STRATEGIES.map(({ id, icon, gear }) => (
              <div
                key={id}
                className="openbitfun-agents-scene__harness-profile"
                data-openbitfun-component="harness-profile-step"
                data-openbitfun-part="root"
                data-openbitfun-profile={id}
                data-harness-gear={gear}
                data-testid={`agents-harness-${id}`}
              >
                <Icon name={icon} size="lg" className="openbitfun-agents-scene__harness-profile-icon" />
                <strong className="openbitfun-agents-scene__harness-profile-name">
                  {t(`harnessZone.profiles.${id}.name`)}
                </strong>
                <span className="openbitfun-agents-scene__harness-profile-purpose">
                  {t(`harnessZone.profiles.${id}.purpose`)}
                </span>
                <HarnessStrategyRoute profile={id} />
              </div>
            ))}
          </div>
          <HarnessConnections converge />
          <span className="openbitfun-agents-scene__harness-endpoint">
            {t('harnessZone.result')}
          </span>
        </div>
      </GalleryZone>
    </div>
  );
};

export default AgentHarnessOverview;
