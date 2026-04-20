/**
 * Git Graph Live App — build SVG for one commit row (theme-aware colors).
 */
(function () {
  window.__GG = window.__GG || {};
  const ROW_H = window.__GG.ROW_H;
  const LANE_W = window.__GG.LANE_W;
  const NODE_R = window.__GG.NODE_R;

  window.__GG.buildRowSvg = function (commit, drawInfo, graphW, isStash, isUncommitted) {
    isStash = !!isStash;
    isUncommitted = !!isUncommitted;
    const lane = drawInfo.lane;
    const lanesBefore = drawInfo.lanesBefore;
    const parentLanes = drawInfo.parentLanes;
    const colors = window.__GG.getGraphColors();
    const nodeStroke = window.__GG.getNodeStroke();
    const uncommittedColor = window.__GG.getUncommittedColor();

    const svgNS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(svgNS, 'svg');
    svg.setAttribute('width', graphW);
    svg.setAttribute('height', ROW_H);
    svg.setAttribute('viewBox', '0 0 ' + graphW + ' ' + ROW_H);
    svg.style.display = 'block';
    svg.style.overflow = 'visible';

    const cx = lane * LANE_W + LANE_W / 2 + 4;
    const cy = ROW_H / 2;
    const nodeColor = isUncommitted ? uncommittedColor : (colors[lane % colors.length] || colors[0]);
    const bezierD = ROW_H * 0.8;

    function laneX(l) { return l * LANE_W + LANE_W / 2 + 4; }

    function mkPath(dAttr, stroke, dash) {
      const p = document.createElementNS(svgNS, 'path');
      p.setAttribute('d', dAttr);
      p.setAttribute('stroke', stroke);
      p.setAttribute('fill', 'none');
      p.setAttribute('stroke-width', '1.5');
      p.setAttribute('class', 'graph-line');
      if (dash) p.setAttribute('stroke-dasharray', dash);
      return p;
    }

    for (let l = 0; l < lanesBefore.length; l++) {
      if (lanesBefore[l] !== null && l !== lane) {
        const stroke = colors[l % colors.length] || colors[0];
        svg.appendChild(mkPath('M' + laneX(l) + ' 0 L' + laneX(l) + ' ' + ROW_H, stroke, null));
      }
    }

    const wasActive = lane < lanesBefore.length && lanesBefore[lane] !== null;
    if (wasActive) {
      const path = mkPath('M' + cx + ' 0 L' + cx + ' ' + cy, nodeColor, isUncommitted ? '3 3' : null);
      if (isUncommitted) path.setAttribute('class', 'graph-line graph-line--uncommitted');
      svg.appendChild(path);
    }

    for (let i = 0; i < parentLanes.length; i++) {
      const pl = parentLanes[i];
      const px = laneX(pl.lane);
      const lineColor = isUncommitted ? uncommittedColor : (colors[pl.lane % colors.length] || colors[0]);
      const dash = isUncommitted ? '3 3' : null;
      var path;
      if (px === cx) {
        path = mkPath('M' + cx + ' ' + cy + ' L' + cx + ' ' + ROW_H, lineColor, dash);
      } else {
        path = mkPath(
          'M' + cx + ' ' + cy + ' C' + cx + ' ' + (cy + bezierD) + ' ' + px + ' ' + (ROW_H - bezierD) + ' ' + px + ' ' + ROW_H,
          lineColor, dash
        );
      }
      if (isUncommitted) path.setAttribute('class', 'graph-line graph-line--uncommitted');
      svg.appendChild(path);
    }

    if (isStash) {
      const outer = document.createElementNS(svgNS, 'circle');
      outer.setAttribute('cx', cx); outer.setAttribute('cy', cy); outer.setAttribute('r', 4.5);
      outer.setAttribute('fill', 'none'); outer.setAttribute('stroke', nodeColor); outer.setAttribute('stroke-width', '1.5');
      outer.setAttribute('class', 'graph-node graph-node--stash-outer');
      svg.appendChild(outer);
      const inner = document.createElementNS(svgNS, 'circle');
      inner.setAttribute('cx', cx); inner.setAttribute('cy', cy); inner.setAttribute('r', 2);
      inner.setAttribute('fill', nodeColor);
      inner.setAttribute('class', 'graph-node graph-node--stash-inner');
      svg.appendChild(inner);
    } else if (isUncommitted) {
      const circle = document.createElementNS(svgNS, 'circle');
      circle.setAttribute('cx', cx); circle.setAttribute('cy', cy); circle.setAttribute('r', NODE_R);
      circle.setAttribute('fill', 'none'); circle.setAttribute('stroke', uncommittedColor); circle.setAttribute('stroke-width', '1.5');
      circle.setAttribute('class', 'graph-node graph-node--uncommitted');
      svg.appendChild(circle);
    } else {
      const circle = document.createElementNS(svgNS, 'circle');
      circle.setAttribute('cx', cx); circle.setAttribute('cy', cy); circle.setAttribute('r', NODE_R);
      circle.setAttribute('fill', nodeColor); circle.setAttribute('stroke', nodeStroke); circle.setAttribute('stroke-width', '1.5');
      circle.setAttribute('class', 'graph-node graph-node--commit');
      svg.appendChild(circle);
    }

    return svg;
  };
})();
