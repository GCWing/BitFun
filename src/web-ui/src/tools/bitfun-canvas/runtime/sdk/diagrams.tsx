import type {
  CanvasDagEdge,
  CanvasDagLayout,
  CanvasDagNode,
  CanvasDependencyGraphProps,
  CanvasFlowDiagramProps,
  CanvasFlowStep,
} from './types';
import { toneColor } from './style';
import { computeDAGLayout, edgePath, normalizeDagEdges } from './diagramLayout';

function nodeLabel(node?: CanvasDagNode | CanvasFlowStep, fallback = '') {
  return node?.label ?? node?.title ?? fallback;
}

function nodeDescription(node?: CanvasDagNode | CanvasFlowStep) {
  const meta = node?.meta;
  return node?.description
    ?? node?.subtitle
    ?? node?.sub
    ?? (typeof meta === 'string' || typeof meta === 'number' ? meta : undefined);
}

function DiagramShell({
  title,
  height,
  style,
  className,
  children,
  ...props
}: Omit<CanvasDependencyGraphProps, 'nodes' | 'edges'>) {
  return (
    <div
      {...props}
      className={['bf-diagram', className].filter(Boolean).join(' ')}
      style={{
        minWidth: 0,
        overflow: 'auto',
        border: '1px solid color-mix(in srgb, var(--border-subtle) 78%, transparent)',
        borderRadius: 8,
        background: 'color-mix(in srgb, var(--color-bg-elevated) 56%, transparent)',
        padding: 14,
        ...style,
      }}
    >
      {title ? (
        <div style={{ marginBottom: 10, color: 'var(--color-text-primary)', fontSize: 12, fontWeight: 650, lineHeight: 1.25 }}>
          {title}
        </div>
      ) : null}
      <div style={{ minHeight: height }}>{children}</div>
    </div>
  );
}

function renderGraphSvg({
  layout,
  nodes,
  edges,
  title,
}: {
  layout: CanvasDagLayout;
  nodes: CanvasDagNode[];
  edges: CanvasDagEdge[];
  title?: string;
}) {
  const nodeById = new Map(nodes.map(node => [String(node.id), node]));
  const edgeByKey = new Map(normalizeDagEdges(edges).map(edge => [`${String(edge.from)}\u0000${String(edge.to)}`, edge]));

  return (
    <svg
      viewBox={`0 0 ${Math.max(layout.width, 1)} ${Math.max(layout.height, 1)}`}
      role="img"
      aria-label={title || 'Dependency graph'}
      style={{ display: 'block', width: '100%', minWidth: layout.width, height: layout.height, overflow: 'visible' }}
    >
      <g aria-hidden="true">
        {layout.ranks.map((rank, index) => (
          <rect
            key={`rank-${rank.rank}`}
            x={rank.x - 8}
            y={rank.y - 8}
            width={rank.width + 16}
            height={rank.height + 16}
            rx={8}
            fill={index % 2 === 0 ? 'var(--element-bg-subtle)' : 'var(--color-bg-chrome)'}
            opacity={index % 2 === 0 ? 0.72 : 0.46}
          />
        ))}
      </g>
      <g fill="none">
        {layout.edges.map((edge, index) => {
          const meta = edgeByKey.get(`${edge.from}\u0000${edge.to}`);
          const color = toneColor(meta?.tone);
          return (
            <g key={`${edge.from}-${edge.to}-${index}`}>
              <path
                d={edgePath(edge, layout.direction)}
                stroke={color}
                strokeWidth={1.35}
                opacity={edge.isBackEdge ? 0.32 : 0.46}
              />
              <circle cx={edge.targetX} cy={edge.targetY} r={2.35} fill={color} opacity={0.62} />
              {meta?.label ? (
                <text
                  x={(edge.sourceX + edge.targetX) / 2}
                  y={(edge.sourceY + edge.targetY) / 2 - 4}
                  textAnchor="middle"
                  fill="var(--color-text-muted)"
                  fontSize={10}
                >
                  {String(meta.label).slice(0, 18)}
                </text>
              ) : null}
            </g>
          );
        })}
      </g>
      {layout.nodes.map(layoutNode => {
        const node = nodeById.get(layoutNode.id);
        const label = nodeLabel(node, layoutNode.id);
        const description = nodeDescription(node);
        const color = toneColor(node?.tone);
        return (
          <g key={layoutNode.id} transform={`translate(${layoutNode.x} ${layoutNode.y})`}>
            <rect
              width={layoutNode.width}
              height={layoutNode.height}
              rx={6}
              fill="var(--color-bg-elevated)"
              stroke="var(--border-subtle)"
              strokeWidth={1}
            />
            <rect
              width={4}
              height={layoutNode.height}
              rx={4}
              fill={color}
              opacity={0.78}
            />
            <text x={14} y={description ? 18 : layoutNode.height / 2 + 4} fill="var(--color-text-primary)" fontSize={12} fontWeight={650}>
              {String(label).slice(0, 22)}
            </text>
            {description ? (
              <text x={14} y={34} fill="var(--color-text-muted)" fontSize={10}>
                {String(description).slice(0, 26)}
              </text>
            ) : null}
          </g>
        );
      })}
    </svg>
  );
}

export function DependencyGraph({
  nodes = [],
  edges = [],
  direction = 'vertical',
  nodeWidth = 160,
  nodeHeight = 46,
  rankGap = 64,
  nodeGap = 48,
  padding = 24,
  title,
  height,
  style,
  className,
  ...props
}: CanvasDependencyGraphProps) {
  const layout = computeDAGLayout({ nodes, edges, direction, nodeWidth, nodeHeight, rankGap, nodeGap, padding });
  const resolvedEdges = normalizeDagEdges(edges);
  const resolvedHeight = height ?? layout.height;

  return (
    <DiagramShell {...props} title={title} height={resolvedHeight} style={style} className={className}>
      {nodes.length ? (
        renderGraphSvg({ layout, nodes, edges: resolvedEdges, title: String(title || 'Dependency graph') })
      ) : (
        <div style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>No graph nodes</div>
      )}
    </DiagramShell>
  );
}

function normalizeFlowSteps(steps: CanvasFlowDiagramProps['steps']): CanvasDagNode[] {
  if (!Array.isArray(steps)) return [];
  return steps.map((step, index) => {
    if (typeof step === 'string') {
      return { id: `step-${index + 1}`, label: step };
    }
    return {
      id: step.id ?? `step-${index + 1}`,
      label: nodeLabel(step, `Step ${index + 1}`),
      description: step.description ?? step.subtitle ?? step.sub,
      tone: step.tone,
      meta: step.meta,
    };
  });
}

function flowEdges(nodes: CanvasDagNode[]): CanvasDagEdge[] {
  return nodes.slice(0, -1).map((node, index) => ({ from: node.id, to: nodes[index + 1].id }));
}

export function FlowDiagram({
  steps,
  nodes,
  edges,
  direction = 'horizontal',
  nodeWidth = 150,
  nodeHeight = 46,
  rankGap = 54,
  nodeGap = 36,
  padding = 20,
  title,
  height,
  style,
  className,
  ...props
}: CanvasFlowDiagramProps) {
  const stepNodes = normalizeFlowSteps(steps);
  const resolvedNodes = nodes?.length ? nodes : stepNodes;
  const resolvedEdges = edges?.length ? edges : flowEdges(resolvedNodes);
  const layout = computeDAGLayout({
    nodes: resolvedNodes,
    edges: resolvedEdges,
    direction,
    nodeWidth,
    nodeHeight,
    rankGap,
    nodeGap,
    padding,
  });

  return (
    <DiagramShell {...props} title={title} height={height ?? layout.height} style={style} className={className}>
      {resolvedNodes.length ? (
        renderGraphSvg({ layout, nodes: resolvedNodes, edges: resolvedEdges, title: String(title || 'Flow diagram') })
      ) : (
        <div style={{ color: 'var(--color-text-muted)', fontSize: 12 }}>No flow steps</div>
      )}
    </DiagramShell>
  );
}
