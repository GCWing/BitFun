import { clamp, mix, round, segment, type Point } from './releaseLetterMotion';

const COS30 = Math.sqrt(3) / 2;
const point = (p: Point) => `${round(p.x)} ${round(p.y)}`;

export function roundedHex(apothem: number, radius: number, angle: number) {
  const vertices = Array.from({ length: 6 }, (_, index) => {
    const radians = (angle + index * 60) * Math.PI / 180;
    return { x: 256 + apothem / COS30 * Math.cos(radians), y: 256 + apothem / COS30 * Math.sin(radians) };
  });
  const toward = (from: Point, to: Point) => {
    const distance = radius / Math.sqrt(3) / Math.hypot(to.x - from.x, to.y - from.y);
    return { x: mix(from.x, to.x, distance), y: mix(from.y, to.y, distance) };
  };
  const corners = vertices.map((vertex, index) => ({
    entry: toward(vertex, vertices[(index + 5) % 6]),
    exit: toward(vertex, vertices[(index + 1) % 6]),
    center: { x: 256 + (vertex.x - 256) * (apothem - radius) / apothem, y: 256 + (vertex.y - 256) * (apothem - radius) / apothem },
  }));
  const d = radius < .001 ? `M${vertices.map(point).join('L')}Z`
    : corners.map((corner, i) => `${i ? 'L' : 'M'}${point(corner.entry)}A${round(radius)} ${round(radius)} 0 0 1 ${point(corner.exit)}`).join('') + 'Z';
  return { vertices, corners, d };
}

/** Scoped to one mounted drawing. Only these generated groups are DOM-owned. */
export function createLetterDrawing(svg: SVGSVGElement) {
  const nodesByName = new Map(Array.from(svg.querySelectorAll<SVGElement>('[data-drawing]')).map(node => [node.dataset.drawing!, node]));
  const get = (name: string) => nodesByName.get(name)!;
  const attribute = (node: Element, name: string, value: number | string) => node.setAttribute(name, typeof value === 'number' ? String(round(value)) : value);
  const opacity = (name: string, value: number) => attribute(get(name), 'opacity', clamp(value));
  const element = (tag: string, attrs: Record<string, string | number>, parent: Element) => {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    Object.entries(attrs).forEach(([name, value]) => attribute(node, name, value));
    parent.append(node);
    return node;
  };
  const trace = (node: Element, value: number) => {
    const progress = clamp(value);
    // A completed contour must not retain a dash mask during the later scale.
    attribute(node, 'stroke-dasharray', progress === 1 ? 'none' : '1 1');
    attribute(node, 'stroke-dashoffset', 1 - progress);
    attribute(node, 'opacity', progress <= 0 ? 0 : 1);
  };
  const compass = (circle: string, pen: string, radius: number, value: number) => {
    trace(get(circle), value);
    const angle = -Math.PI / 2 + Math.PI * 2 * value;
    attribute(get(pen), 'cx', 256 + Math.cos(angle) * radius);
    attribute(get(pen), 'cy', 256 + Math.sin(angle) * radius);
    opacity(pen, value > 0 && value < .998 ? 1 : 0);
  };

  // Clearing only renderer-owned groups makes StrictMode effect replay idempotent.
  ['radials', 'anchorNodes', 'filletGuides'].forEach(name => get(name).replaceChildren());
  const radials: Element[] = [];
  const anchors: { node: Element; delay: number }[] = [];
  const fillets: { node: Element; arc: Element; delay: number }[] = [];
  [roundedHex(185.5, 0, -90), roundedHex(144, 0, 0)].forEach((hex, ring) => {
    hex.vertices.forEach((vertex, index) => {
      radials.push(element('path', { d: `M256 256L${point(vertex)}`, pathLength: 1 }, get('radials')));
      anchors.push({ node: element('circle', { cx: vertex.x, cy: vertex.y, r: 2.3, class: 'node' }, get('anchorNodes')), delay: index * .017 + ring * .08 });
    });
  });
  [{ hex: roundedHex(185.5, 60, -90), radius: 60 }, { hex: roundedHex(144, 32, 0), radius: 32 }].forEach(({ hex, radius }) => {
    [0, 2, 4].forEach((cornerIndex, index) => {
      const corner = hex.corners[cornerIndex];
      const node = element('g', {}, get('filletGuides'));
      element('circle', { cx: corner.center.x, cy: corner.center.y, r: radius, class: 'guide-soft' }, node);
      element('path', { d: `M${point(corner.entry)}L${point(corner.center)}L${point(corner.exit)}`, class: 'guide-soft' }, node);
      element('path', { d: `M${round(corner.center.x - 3)} ${round(corner.center.y)}h6M${round(corner.center.x)} ${round(corner.center.y - 3)}v6`, class: 'cross' }, node);
      const arc = element('path', { d: `M${point(corner.entry)}A${radius} ${radius} 0 0 1 ${point(corner.exit)}`, class: 'fillet-arc', pathLength: 1 }, node);
      [corner.entry, corner.exit].forEach(p => element('circle', { cx: p.x, cy: p.y, r: 1.7, class: 'node' }, node));
      fillets.push({ node, arc, delay: index * .033 + (radius === 32 ? .028 : 0) });
    });
  });
  const axes = Array.from(svg.querySelectorAll('[data-axis]'));
  const bounds = Array.from(svg.querySelectorAll('[data-bound]'));
  let lastRound = -1;

  function render(progress: number) {
    const t = clamp(progress), form = segment(t, .5, .73);
    const outerRadius = 185.5 / COS30 - 60 * form * (1 / COS30 - 1);
    ['axes', 'circles', 'diagonals', 'bounds'].forEach(name => get(name).style.removeProperty('stroke'));
    opacity('guideField', 1 - segment(t, .88, 1) * .91);
    opacity('axes', .43);
    axes.forEach((node, i) => trace(node, segment(t, .025 + i * .012, .17 + i * .012)));
    attribute(get('outerCircle'), 'r', outerRadius);
    compass('outerCircle', 'penOuter', outerRadius, segment(t, .115, .315));
    compass('innerCircle', 'penInner', 144, segment(t, .17, .37));
    compass('radiusCircle', 'penRadius', 60, segment(t, .22, .4));
    opacity('circles', .62);
    opacity('diagonals', segment(t, .405, .52) * .28);
    opacity('bounds', .4);
    bounds.forEach((node, i) => trace(node, segment(t, .29 + i * .018, .465 + i * .018)));
    radials.forEach((node, i) => { const offset = (i % 6) * .013 + (i > 5 ? .055 : 0); trace(node, segment(t, .29 + offset, .46 + offset)); });
    opacity('radials', .34 * (1 - segment(t, .54, .78)));
    opacity('dimensionLines', segment(t, .4, .52) * .5);
    opacity('dimensionLabels', segment(t, .4, .52) * .8);
    if (Math.abs(form - lastRound) > .00001) {
      lastRound = form;
      const path = roundedHex(185.5, 60 * form, -90).d + ' ' + roundedHex(144, 32 * form, 0).d;
      ['materialRim', 'outlineEdge', 'startupClipPath'].forEach(name => attribute(get(name), 'd', path));
    }
    anchors.forEach(({ node, delay }) => attribute(node, 'opacity', segment(t, .31 + delay, .36 + delay) * (1 - segment(t, .55, .7))));
    fillets.forEach(({ node, arc, delay }) => {
      attribute(node, 'opacity', segment(t, .485 + delay, .57 + delay) * (1 - segment(t, .69, .86)) * .82);
      trace(arc, segment(t, .51 + delay, .665 + delay));
    });
    opacity('formOutline', segment(t, .53, .72) * (1 - segment(t, .78, .96)) * .82);
    trace(get('outlineEdge'), segment(t, .53, .72));
    opacity('paperSurface', 0);
    opacity('materialRim', 1);
    opacity('material', segment(t, .75, 1));
    opacity('centerPoint', segment(t, 0, .05) * (1 - segment(t, .67, .89)));
    attribute(get('centerPoint'), 'r', 1.5 + 2.5 * (1 - segment(t, .025, .15)));
  }

  function handoff(progress: number) {
    const tone = segment(progress, .06, .74), quiet = segment(progress, .2, .9);
    opacity('paperSurface', tone);
    opacity('material', mix(1, .72, tone));
    opacity('materialRim', 1 - segment(progress, 0, .46));
    opacity('guideField', 1);
    [['axes', .09 * .43, .72 * .58 * .45], ['circles', .09 * .62, .72 * .58 * .78],
      ['diagonals', .09 * .28, .72 * .42 * .68], ['bounds', .09 * .4, .72 * .42 * .38]].forEach(([name, from, to]) => {
      opacity(name as string, mix(from as number, to as number, quiet));
      get(name as string).style.stroke = `color-mix(in srgb, var(--openbitfun-color-content-muted) ${round(mix(58, 20, quiet))}%, var(--openbitfun-color-surface-panel))`;
    });
    opacity('dimensionLines', .09 * .5 * (1 - quiet));
    opacity('dimensionLabels', .09 * .8 * (1 - quiet));
  }
  return { render, handoff };
}
