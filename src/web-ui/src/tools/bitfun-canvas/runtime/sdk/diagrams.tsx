import type {
  CanvasDagEdge,
  CanvasDagLayout,
  CanvasDagLayoutEdge,
  CanvasDagLayoutNode,
  CanvasDagNode,
  CanvasDependencyGraphProps,
  CanvasDagLayoutOptions,
  CanvasDagLayoutRank,
  CanvasFlowDiagramProps,
  CanvasFlowStep,
} from './types';
import { toneColor } from './style';

function normalizeDagEdges(edges: CanvasDagEdge[]): Array<CanvasDagEdge & { from: string | number; to: string | number }> {
  return edges
    .map(edge => ({
      ...edge,
      from: edge.from ?? edge.source,
      to: edge.to ?? edge.target,
    }))
    .filter((edge): edge is CanvasDagEdge & { from: string | number; to: string | number } => edge.from !== undefined && edge.to !== undefined);
}

export function computeDAGLayout(options: CanvasDagLayoutOptions = {}): CanvasDagLayout {
  const nodes = Array.isArray(options.nodes) ? options.nodes : [];
  const edges = Array.isArray(options.edges) ? normalizeDagEdges(options.edges) : [];
  const direction = options.direction === 'horizontal' ? 'horizontal' : 'vertical';
  const nodeWidth = Number(options.nodeWidth) || 160;
  const nodeHeight = Number(options.nodeHeight) || 40;
  const rankGap = Number(options.rankGap) || 64;
  const nodeGap = Number(options.nodeGap) || 48;
  const padding = Number(options.padding) || 24;
  const nodeMetaById = new Map(nodes.map(node => [String(node.id), node]));
  const ids = nodes.map(node => String(node.id));
  const idSet = new Set(ids);
  const outgoing = new Map(ids.map(id => [id, [] as string[]]));
  const incoming = new Map(ids.map(id => [id, [] as string[]]));

  for (const edge of edges) {
    const from = String(edge.from);
    const to = String(edge.to);
    if (!idSet.has(from) || !idSet.has(to)) continue;
    outgoing.get(from)?.push(to);
    incoming.get(to)?.push(from);
  }

  const rankById = new Map(ids.map(id => [id, 0]));
  for (let i = 0; i < ids.length; i++) {
    for (const edge of edges) {
      if (!idSet.has(String(edge.from)) || !idSet.has(String(edge.to))) continue;
      rankById.set(
        String(edge.to),
        Math.max(rankById.get(String(edge.to)) || 0, (rankById.get(String(edge.from)) || 0) + 1),
      );
    }
  }

  const rankKeys = Array.from(new Set(ids.map(id => rankById.get(id) || 0))).sort((a, b) => a - b);
  const byRank = new Map(rankKeys.map(rank => [rank, [] as string[]]));
  ids.forEach(id => byRank.get(rankById.get(id) || 0)?.push(id));

  const positioned: CanvasDagLayoutNode[] = [];
  const ranks: CanvasDagLayoutRank[] = [];
  let maxRankWidth = 0;
  let maxRankHeight = 0;

  for (const rank of rankKeys) {
    const rankIds = byRank.get(rank) || [];
    const rankWidth = direction === 'vertical'
      ? rankIds.length * nodeWidth + Math.max(0, rankIds.length - 1) * nodeGap
      : nodeWidth;
    const rankHeight = direction === 'vertical'
      ? nodeHeight
      : rankIds.length * nodeHeight + Math.max(0, rankIds.length - 1) * nodeGap;
    maxRankWidth = Math.max(maxRankWidth, rankWidth);
    maxRankHeight = Math.max(maxRankHeight, rankHeight);
    ranks.push({ rank, x: padding, y: padding, width: rankWidth, height: rankHeight });
  }

  const canvasWidth = direction === 'vertical'
    ? padding * 2 + maxRankWidth
    : padding * 2 + rankKeys.length * nodeWidth + Math.max(0, rankKeys.length - 1) * rankGap;
  const canvasHeight = direction === 'vertical'
    ? padding * 2 + rankKeys.length * nodeHeight + Math.max(0, rankKeys.length - 1) * rankGap
    : padding * 2 + maxRankHeight;

  rankKeys.forEach((rank, rankIndex) => {
    const rankIds = byRank.get(rank) || [];
    const rankWidth = direction === 'vertical'
      ? rankIds.length * nodeWidth + Math.max(0, rankIds.length - 1) * nodeGap
      : nodeWidth;
    const rankHeight = direction === 'vertical'
      ? nodeHeight
      : rankIds.length * nodeHeight + Math.max(0, rankIds.length - 1) * nodeGap;
    const rankX = direction === 'vertical'
      ? padding + (maxRankWidth - rankWidth) / 2
      : padding + rankIndex * (nodeWidth + rankGap);
    const rankY = direction === 'vertical'
      ? padding + rankIndex * (nodeHeight + rankGap)
      : padding + (maxRankHeight - rankHeight) / 2;
    const rankMeta = ranks.find(item => item.rank === rank);
    Object.assign(rankMeta || {}, { x: rankX, y: rankY, width: rankWidth, height: rankHeight });
    rankIds.forEach((id, index) => {
      const meta = nodeMetaById.get(id);
      const x = direction === 'vertical' ? rankX + index * (nodeWidth + nodeGap) : rankX;
      const y = direction === 'vertical' ? rankY : rankY + index * (nodeHeight + nodeGap);
      positioned.push({
        ...(meta || {}),
        id,
        meta,
        source: meta,
        x,
        y,
        centerX: x + nodeWidth / 2,
        centerY: y + nodeHeight / 2,
        width: nodeWidth,
        height: nodeHeight,
        rank,
      });
    });
  });

  const pos = new Map(positioned.map(node => [node.id, node]));
  ranks.forEach((rank) => {
    const rankNodes = positioned.filter(node => node.rank === rank.rank);
    rank.nodeIds = rankNodes.map(node => node.id);
    rank.nodes = rankNodes;
  });
  const layoutEdges = edges
    .map((edge): CanvasDagLayoutEdge | null => {
      const source = pos.get(String(edge.from));
      const target = pos.get(String(edge.to));
      if (!source || !target) return null;
      const layoutEdge = direction === 'vertical'
        ? {
            from: String(edge.from),
            to: String(edge.to),
            sourceX: source.x + nodeWidth / 2,
            sourceY: source.y + nodeHeight,
            targetX: target.x + nodeWidth / 2,
            targetY: target.y,
            isBackEdge: (rankById.get(String(edge.to)) || 0) <= (rankById.get(String(edge.from)) || 0),
          }
        : {
            from: String(edge.from),
            to: String(edge.to),
            sourceX: source.x + nodeWidth,
            sourceY: source.y + nodeHeight / 2,
            targetX: target.x,
            targetY: target.y + nodeHeight / 2,
            isBackEdge: (rankById.get(String(edge.to)) || 0) <= (rankById.get(String(edge.from)) || 0),
          };
      return {
        ...edge,
        ...layoutEdge,
        path: edgePath(layoutEdge, direction),
      };
    })
    .filter((edge): edge is CanvasDagLayoutEdge => Boolean(edge));

  return withLayoutNodeArrayCompat({
    nodes: positioned,
    edges: layoutEdges,
    ranks,
    direction,
    width: canvasWidth,
    height: canvasHeight,
  });
}

function withLayoutNodeArrayCompat(layout: {
  nodes: CanvasDagLayoutNode[];
  edges: CanvasDagLayoutEdge[];
  ranks: CanvasDagLayoutRank[];
  direction: 'vertical' | 'horizontal';
  width: number;
  height: number;
}): CanvasDagLayout {
  const compatLayout = layout as CanvasDagLayout;
  return Object.assign(compatLayout, {
    [Symbol.iterator]: () => compatLayout.nodes[Symbol.iterator](),
    find: compatLayout.nodes.find.bind(compatLayout.nodes),
    filter: compatLayout.nodes.filter.bind(compatLayout.nodes),
    forEach: compatLayout.nodes.forEach.bind(compatLayout.nodes),
    map: compatLayout.nodes.map.bind(compatLayout.nodes),
  });
}

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

function edgePath(
  edge: Pick<CanvasDagLayoutEdge, 'sourceX' | 'sourceY' | 'targetX' | 'targetY'>,
  direction: CanvasDagLayout['direction'],
) {
  if (direction === 'horizontal') {
    const midX = edge.sourceX + (edge.targetX - edge.sourceX) / 2;
    return `M ${edge.sourceX} ${edge.sourceY} C ${midX} ${edge.sourceY}, ${midX} ${edge.targetY}, ${edge.targetX} ${edge.targetY}`;
  }
  const midY = edge.sourceY + (edge.targetY - edge.sourceY) / 2;
  return `M ${edge.sourceX} ${edge.sourceY} C ${edge.sourceX} ${midY}, ${edge.targetX} ${midY}, ${edge.targetX} ${edge.targetY}`;
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
