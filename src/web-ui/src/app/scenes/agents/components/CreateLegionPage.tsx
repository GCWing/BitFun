import React, { useCallback, useMemo, useState } from 'react';
import { ArrowLeft, GitBranch, Network } from 'lucide-react';
import { Button, IconButton } from '@/component-library';
import PATTERNS, {
  type LegionPatternNode,
  type LegionPatternEdge,
} from '../data/orchestration-patterns';
import '../AgentsView.scss';

interface CreateLegionPageProps {
  onBack: () => void;
}

const CreateLegionPage: React.FC<CreateLegionPageProps> = ({ onBack }) => {
  const [selectedPatternId, setSelectedPatternId] = useState<string>(PATTERNS[0]?.id ?? '');

  const patternOptions = useMemo(() => PATTERNS, []);

  const selectedPattern = useMemo(
    () => patternOptions.find((p) => p.id === selectedPatternId) ?? null,
    [patternOptions, selectedPatternId],
  );

  const handleSelectPattern = useCallback((id: string) => {
    setSelectedPatternId(id);
  }, []);

  const handleSave = useCallback(() => {
    // TODO: save legion preset to backend via Tauri command
    onBack();
  }, [onBack]);

  const renderNodeList = (nodes: LegionPatternNode[]) => (
    <div className="legion-node-list">
      {nodes.map((node, i) => (
        <div key={node.id} className="legion-node-item">
          <span className="legion-node-index">{i + 1}</span>
          <div className="legion-node-info">
            <span className="legion-node-role">{node.role}</span>
            <span className="legion-node-agent">{node.agent}</span>
          </div>
          {node.gate ? <span className="legion-node-gate">GATE</span> : null}
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
          aria-label="Back"
          data-testid="create-legion-back"
        >
          <ArrowLeft size={18} />
        </IconButton>
        <h1 className="create-agent-page__title">
          {selectedPattern ? selectedPattern.name : 'Choose a Pattern'}
        </h1>
      </div>

      {/* Pattern selector */}
      <section className="create-agent-page__section">
        <h2 className="create-agent-page__section-title">Orchestration Patterns</h2>
        <div className="legion-pattern-grid">
          {patternOptions.map((pattern) => (
            <div
              key={pattern.id}
              className={`legion-pattern-chip ${pattern.id === selectedPatternId ? 'legion-pattern-chip--active' : ''}`}
              onClick={() => handleSelectPattern(pattern.id)}
              role="button"
              tabIndex={0}
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
            <h2 className="create-agent-page__section-title">Overview</h2>
            <p className="legion-summary-desc">{selectedPattern.description}</p>
            <div className="legion-summary-meta">
              <span>Complexity: L{selectedPattern.complexityLevel}</span>
              <span>{selectedPattern.nodes.length} nodes</span>
              <span>{selectedPattern.edges.length} edges</span>
            </div>
          </section>

          {/* Nodes */}
          <section className="create-agent-page__section">
            <h2 className="create-agent-page__section-title">
              Nodes ({selectedPattern.nodes.length})
            </h2>
            {renderNodeList(selectedPattern.nodes)}
          </section>

          {/* Edges */}
          <section className="create-agent-page__section">
            <h2 className="create-agent-page__section-title">
              Edges ({selectedPattern.edges.length})
            </h2>
            {selectedPattern.edges.length > 0
              ? renderEdgeList(selectedPattern.edges, selectedPattern.nodes)
              : <p className="legion-empty-hint">No edges (self-contained agent)</p>}
          </section>

          {/* Actions */}
          <div className="create-agent-page__actions">
            <Button variant="secondary" onClick={onBack}>
              Back
            </Button>
            <Button variant="primary" onClick={handleSave} data-testid="create-legion-save">
              Use Pattern
            </Button>
          </div>
        </>
      ) : null}
    </div>
  );
};

export default CreateLegionPage;
