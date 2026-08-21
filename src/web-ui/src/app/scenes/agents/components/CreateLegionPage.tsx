import React, { useCallback, useState } from 'react';
import { ArrowLeft, GitBranch, Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton } from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { getCardGradient } from '@/shared/utils/cardGradients';
import PATTERNS, {
  type LegionPatternNode,
  type LegionPatternEdge,
} from '../data/orchestration-patterns';
import { LegionPresetAPI } from '@/infrastructure/api/service-api/LegionPresetAPI';
import { createLogger } from '@/shared/utils/logger';
import { DependencyGraph } from '@/tools/bitfun-canvas/runtime/sdk/diagrams';
import '../AgentsView.scss';
import './CreateLegionPage.scss';

interface CreateLegionPageProps {
  onBack: () => void;
}

const log = createLogger('CreateLegionPage');

// UI-01: the backend create_legion_preset command is now registered on the
// Rust side (desktop api::commands::create_legion_preset), so real saving is
// enabled. Keep this flag in sync with the desktop command registration.
const LEGION_CREATE_BACKEND_READY = true;

const CreateLegionPage: React.FC<CreateLegionPageProps> = ({ onBack }) => {
  const { t } = useTranslation('scenes/agents');
  const { success: notifySuccess, error: notifyError } = useNotification();
  const [selectedPatternId, setSelectedPatternId] = useState<string>(PATTERNS[0]?.id ?? '');
  const [saving, setSaving] = useState(false);

  const selectedPattern = PATTERNS.find((p) => p.id === selectedPatternId) ?? null;

  const handleSelectPattern = useCallback((id: string) => {
    setSelectedPatternId(id);
  }, []);

  const handleSave = useCallback(async () => {
    if (!selectedPattern || saving) return;
    setSaving(true);
    try {
      await LegionPresetAPI.createPreset({
        id: selectedPattern.id,
        name: selectedPattern.name,
        description: selectedPattern.description,
        nodes: selectedPattern.nodes.map((n) => ({
          id: n.id,
          agent: n.agent,
          role: n.role,
          prompt: n.prompt,
          gate: n.gate,
        })),
        edges: selectedPattern.edges.map((e) => ({
          from: e.from,
          to: e.to,
          condition: e.condition,
        })),
      });
      notifySuccess(t('legionPattern.saved', { name: selectedPattern.name }));
      onBack();
    } catch (err) {
      log.warn('Failed to save legion preset', { error: err });
      notifyError(t('legionPattern.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, [selectedPattern, saving, onBack, notifySuccess, notifyError, t]);

  const renderNodeList = (nodes: LegionPatternNode[]) => (
    <div className="legion-node-list">
      {nodes.map((node, i) => (
        <div key={node.id} className="legion-node-item">
          <span className="legion-node-index">{i + 1}</span>
          <div className="legion-node-info">
            <span className="legion-node-role">
              {node.role}
              {/* UX-P1-6: legionRole is orchestration metadata only — the
                  deployed session's RBAC role is always resolved by the
                  standard subagent role resolution (Executor for
                  subagent-marked sessions), never by legionRole. Annotate the
                  UI so the displayed role cannot be mistaken for the runtime
                  permission template. */}
              <span className="legion-node-role-annotation" title={t('legionPattern.roleAnnotationTooltip')}>
                {t('legionPattern.roleAnnotation')}
              </span>
            </span>
            <span className="legion-node-agent">{node.agent}</span>
          </div>
          {node.gate ? <span className="legion-node-gate">{t('legionPattern.gate')}</span> : null}
        </div>
      ))}
    </div>
  );

  const renderEdgeList = (edges: LegionPatternEdge[], nodes: LegionPatternNode[]) => (
    <div className="legion-edge-list">
      {edges.map((edge) => {
        const fromNode = nodes.find((n) => n.id === edge.from);
        const toNode = nodes.find((n) => n.id === edge.to);
        return (
          <div key={`${edge.from}->${edge.to}`} className="legion-edge-item">
            <span className="legion-edge-from">{fromNode?.role ?? edge.from}</span>
            <GitBranch size={12} className="legion-edge-arrow" />
            <span className="legion-edge-to">{toNode?.role ?? edge.to}</span>
            {edge.condition ? (
              <span className="legion-edge-condition">[{edge.condition}]</span>
            ) : null}
          </div>
        );
      })}
    </div>
  );

  const renderPatternCanvas = (pattern: (typeof PATTERNS)[number]) => (
    <div
      className="legion-canvas"
      data-bf-component="create-legion-page"
      data-bf-part="canvas"
      data-testid="legion-pattern-canvas"
    >
      <DependencyGraph
        nodes={pattern.nodes.map((n) => ({ id: n.id, label: n.role, description: n.agent }))}
        edges={pattern.edges.map((e) => ({ from: e.from, to: e.to, label: e.condition }))}
        direction="vertical"
        nodeWidth={172}
        nodeHeight={46}
        rankGap={64}
        nodeGap={48}
        padding={20}
      />
    </div>
  );

  return (
    <div
      className="create-agent-page"
      data-testid="create-legion-page"
      data-bf-component="create-legion-page"
      data-bf-part="root"
      style={{ '--legion-page-gradient': getCardGradient('legion') } as React.CSSProperties}
    >
      <div
        className="create-agent-page__header"
        data-bf-component="create-legion-page"
        data-bf-part="header"
      >
        <IconButton
          onClick={onBack}
          aria-label={t('agentsOverview.backToOverview')}
          data-testid="create-legion-back"
        >
          <ArrowLeft size={18} />
        </IconButton>
        <h1 className="create-agent-page__title">
          {selectedPattern ? selectedPattern.name : t('legionPattern.choosePattern')}
        </h1>
      </div>

      {/* Pattern selector */}
      <section
        className="create-agent-page__section"
        data-bf-component="create-legion-page"
        data-bf-part="section"
        aria-labelledby="create-legion-patterns-title"
      >
        <h2
          id="create-legion-patterns-title"
          className="create-agent-page__section-title"
        >
          {t('legionPattern.orchestrationPatterns')}
        </h2>
        <div
          className="legion-pattern-grid"
          role="radiogroup"
          aria-label={t('legionPattern.orchestrationPatterns')}
          data-bf-component="create-legion-page"
          data-bf-part="patternGrid"
        >
          {PATTERNS.map((pattern) => (
            <div
              key={pattern.id}
              className={`legion-pattern-chip ${pattern.id === selectedPatternId ? 'legion-pattern-chip--active' : ''}`}
              style={{ '--legion-chip-gradient': getCardGradient(pattern.id || pattern.name) } as React.CSSProperties}
              onClick={() => handleSelectPattern(pattern.id)}
              role="radio"
              tabIndex={pattern.id === selectedPatternId ? 0 : -1}
              aria-checked={pattern.id === selectedPatternId}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleSelectPattern(pattern.id);
                }
              }}
              data-testid="legion-pattern-option"
              data-pattern-id={pattern.id}
              data-bf-component="create-legion-page"
              data-bf-part="patternChip"
            >
              <span className="legion-pattern-chip__icon" data-bf-component="create-legion-page" data-bf-part="patternChipIcon">
                <Network size={16} />
              </span>
              <span className="legion-pattern-chip__name">{pattern.name}</span>
            </div>
          ))}
        </div>
      </section>

      {selectedPattern ? (
        <>
          {/* Summary */}
          <section
            className="create-agent-page__section"
            aria-live="polite"
            aria-atomic="true"
            data-bf-component="create-legion-page"
            data-bf-part="summary"
          >
            <h2 className="create-agent-page__section-title">{t('legionPattern.overview')}</h2>
            <p className="legion-summary-desc">{selectedPattern.description}</p>
            <div className="legion-summary-meta">
              <span>{t('legionPattern.complexity', { level: selectedPattern.complexityLevel })}</span>
              <span>{t('legionPattern.nodesCount', { count: selectedPattern.nodes.length })}</span>
              <span>{t('legionPattern.edgesCount', { count: selectedPattern.edges.length })}</span>
            </div>
          </section>

          {/* Canvas (R-WF-17 assertion 1: DAG canvas display via official
              DependencyGraph, not a handcrafted list-only rendering) */}
          <section className="create-agent-page__section" data-bf-component="create-legion-page" data-bf-part="canvasSection">
            <h2 className="create-agent-page__section-title">
              {t('legionPattern.canvas')}
            </h2>
            {renderPatternCanvas(selectedPattern)}
          </section>

          {/* Nodes */}
          <section className="create-agent-page__section" data-bf-component="create-legion-page" data-bf-part="nodes">
            <h2 className="create-agent-page__section-title">
              {t('legionPattern.nodes', { count: selectedPattern.nodes.length })}
            </h2>
            {renderNodeList(selectedPattern.nodes)}
          </section>

          {/* Edges */}
          <section className="create-agent-page__section" data-bf-component="create-legion-page" data-bf-part="edges">
            <h2 className="create-agent-page__section-title">
              {t('legionPattern.edges', { count: selectedPattern.edges.length })}
            </h2>
            {selectedPattern.edges.length > 0
              ? renderEdgeList(selectedPattern.edges, selectedPattern.nodes)
              : <p className="legion-empty-hint">{t('legionPattern.noEdges')}</p>}
          </section>

          {/* Actions */}
          <div
            className="create-agent-page__actions"
            data-bf-component="create-legion-page"
            data-bf-part="actions"
          >
            <Button variant="secondary" onClick={onBack}>
              {t('agentsOverview.backToOverview')}
            </Button>
            <Button
              variant="primary"
              onClick={handleSave}
              disabled={saving || !LEGION_CREATE_BACKEND_READY}
              data-testid="create-legion-save"
            >
              {saving ? t('loading') : !LEGION_CREATE_BACKEND_READY ? t('legionPattern.planning') : t('legionPattern.savePreset')}
            </Button>
          </div>
        </>
      ) : null}
    </div>
  );
};

export default CreateLegionPage;
