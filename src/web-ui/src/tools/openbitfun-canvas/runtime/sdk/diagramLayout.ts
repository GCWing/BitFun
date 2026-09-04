import type {
  CanvasDagEdge,
  CanvasDagLayout,
  CanvasDagLayoutEdge,
  CanvasDagLayoutNode,
  CanvasDagLayoutOptions,
  CanvasDagLayoutRank,
} from './types';

export function normalizeDagEdges(
  edges: CanvasDagEdge[],
): Array<CanvasDagEdge & { from: string | number; to: string | number }> {
  return edges
    .map((edge) => ({
      ...edge,
      from: edge.from ?? edge.source,
      to: edge.to ?? edge.target,
    }))
    .filter((edge): edge is CanvasDagEdge & { from: string | number; to: string | number } => (
      edge.from !== undefined && edge.to !== undefined
    ));
}

export function edgePath(
  edge: Pick<CanvasDagLayoutEdge, 'sourceX' | 'sourceY' | 'targetX' | 'targetY'>,
  direction: CanvasDagLayout['direction'],
): string {
  if (direction === 'horizontal') {
    const midX = edge.sourceX + (edge.targetX - edge.sourceX) / 2;
    return `M ${edge.sourceX} ${edge.sourceY} C ${midX} ${edge.sourceY}, ${midX} ${edge.targetY}, ${edge.targetX} ${edge.targetY}`;
  }
  const midY = edge.sourceY + (edge.targetY - edge.sourceY) / 2;
  return `M ${edge.sourceX} ${edge.sourceY} C ${edge.sourceX} ${midY}, ${edge.targetX} ${midY}, ${edge.targetX} ${edge.targetY}`;
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
  const nodeMetaById = new Map(nodes.map((node) => [String(node.id), node]));
  const ids = nodes.map((node) => String(node.id));
  const idSet = new Set(ids);
  const outgoing = new Map(ids.map((id) => [id, [] as string[]]));
  const incoming = new Map(ids.map((id) => [id, [] as string[]]));

  for (const edge of edges) {
    const from = String(edge.from);
    const to = String(edge.to);
    if (!idSet.has(from) || !idSet.has(to)) continue;
    outgoing.get(from)?.push(to);
    incoming.get(to)?.push(from);
  }

  const rankById = new Map(ids.map((id) => [id, 0]));
  for (let index = 0; index < ids.length; index += 1) {
    for (const edge of edges) {
      if (!idSet.has(String(edge.from)) || !idSet.has(String(edge.to))) continue;
      rankById.set(
        String(edge.to),
        Math.max(
          rankById.get(String(edge.to)) || 0,
          (rankById.get(String(edge.from)) || 0) + 1,
        ),
      );
    }
  }

  const rankKeys = Array.from(new Set(ids.map((id) => rankById.get(id) || 0)))
    .sort((left, right) => left - right);
  const byRank = new Map(rankKeys.map((rank) => [rank, [] as string[]]));
  ids.forEach((id) => byRank.get(rankById.get(id) || 0)?.push(id));

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
    const rankMeta = ranks.find((item) => item.rank === rank);
    Object.assign(rankMeta || {}, {
      x: rankX,
      y: rankY,
      width: rankWidth,
      height: rankHeight,
    });
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

  const positions = new Map(positioned.map((node) => [node.id, node]));
  ranks.forEach((rank) => {
    const rankNodes = positioned.filter((node) => node.rank === rank.rank);
    rank.nodeIds = rankNodes.map((node) => node.id);
    rank.nodes = rankNodes;
  });
  const layoutEdges = edges
    .map((edge): CanvasDagLayoutEdge | null => {
      const source = positions.get(String(edge.from));
      const target = positions.get(String(edge.to));
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
