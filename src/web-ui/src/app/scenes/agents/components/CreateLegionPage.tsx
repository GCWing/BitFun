import React, { useCallback, useState } from 'react';
import { ArrowLeft, GitBranch, Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton } from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import PATTERNS, {
  type LegionPatternNode,
  type LegionPatternEdge,
} from '../data/orchestration-patterns';
import { LegionPresetAPI } from '@/infrastructure/api/service-api/LegionPresetAPI';
import '../AgentsView.scss';

interface CreateLegionPageProps {
  onBack: () => void;
}

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
      notifySuccess(`Legion preset "${selectedPattern.name}" saved`);
      onBack();
    } catch (err) {
      notifyError(`Failed to save legion preset: ${err}`);
    } finally {
      setSaving(false);
    }
  }, [selectedPattern, saving, onBack, notifySuccess, notifyError]);

  const renderNodeList = (nodes: LegionPatternNode[]) => (
    <div className="legion-node-list">
      {nodes.map((node, i) => (
        <div key={node.id} className="legion-node-item">
          <span className="legion-node-index">{i + 1}</span>
          <div className="legion-node-info">
            <span className="legion-node-role">{node.role}</span>
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

  return (
    <div className="create-agent-page" data-testid="create-legion-page">
      <div className="create-agent-page__header">
        <IconButton
          onClick={onBack}
          aria-label={t('legionPattern.back')}
          data-testid="create-legion-back"
        >
          <ArrowLeft size={18} />
        </IconButton>
        <h1 className="create-agent-page__title">
          {selectedPattern ? selectedPattern.name : t('legionPattern.choosePattern')}
        </h1>
      </div>

      {/* Pattern selector */}
      <section className="create-agent-page__section">
        <h2 className="create-agent-page__section-title">{t('legionPattern.orchestrationPatterns')}</h2>
        <div className="legion-pattern-grid">
          {PATTERNS.map((pattern) => (
            <div
              key={pattern.id}
              className={`legion-pattern-chip ${pattern.id === selectedPatternId ? 'legion-pattern-chip--active' : ''}`}
              onClick={() => handleSelectPattern(pattern.id)}
              role="button"
              tabIndex={0}
              aria-pressed={pattern.id === selectedPatternId}
              onKeyDown={(e) => e.key === 'Enter' && handleSelectPattern(pattern.id)}
              data-testid="legion-pattern-option"
              data-pattern-id={pattern.id}
            >
              <Network size={14} />
              <span>{pattern.name}</span>
            </div>
          ))}
        </div>
      </section>

      {selectedPattern ? (
        <>
          {/* Summary */}
          <section className="create-agent-page__section">
            <h2 className="create-agent-page__section-title">{t('legionPattern.overview')}</h2>
            <p className="legion-summary-desc">{selectedPattern.description}</p>
            <div className="legion-summary-meta">
              <span>{t('legionPattern.complexity', { level: selectedPattern.complexityLevel })}</span>
              <span>{t('legionPattern.nodesCount', { count: selectedPattern.nodes.length })}</span>
              <span>{t('legionPattern.edgesCount', { count: selectedPattern.edges.length })}</span>
            </div>
          </section>

          {/* Nodes */}
          <section className="create-agent-page__section">
            <h2 className="create-agent-page__section-title">
              {t('legionPattern.nodes', { count: selectedPattern.nodes.length })}
            </h2>
            {renderNodeList(selectedPattern.nodes)}
          </section>

          {/* Edges */}
          <section className="create-agent-page__section">
            <h2 className="create-agent-page__section-title">
              {t('legionPattern.edges', { count: selectedPattern.edges.length })}
            </h2>
            {selectedPattern.edges.length > 0
              ? renderEdgeList(selectedPattern.edges, selectedPattern.nodes)
              : <p className="legion-empty-hint">{t('legionPattern.noEdges')}</p>}
          </section>

          {/* Actions */}
          <div className="create-agent-page__actions">
            <Button variant="secondary" onClick={onBack}>
              {t('legionPattern.back')}
            </Button>
            <Button variant="primary" onClick={handleSave} disabled={saving} data-testid="create-legion-save">
              {saving ? t('loading') : t('legionPattern.usePattern')}
            </Button>
          </div>
        </>
      ) : null}
    </div>
  );
};

export default CreateLegionPage;
