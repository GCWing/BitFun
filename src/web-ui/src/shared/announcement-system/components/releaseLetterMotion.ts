// Choreography and connected rig from the authored openbitfun-letter.html.
// Sampling is independent of the DOM so seeking, replay, and reduced motion agree.
export const INTRO_MS = 4800;
export const HANDOFF_MS = 1800;
export const CONTENT_AT = INTRO_MS + HANDOFF_MS * 0.58;
export const SIGNATURE_AT = CONTENT_AT + 2200;
export const SIGNATURE = 'OpenBitFun Team';
export const MASCOT_MS = 1480;

export const clamp = (value: number) => Math.max(0, Math.min(1, value));
export const smooth = (value: number) => { const t = clamp(value); return t * t * (3 - 2 * t); };
export const segment = (time: number, from: number, to: number) => smooth((time - from) / (to - from));
export const mix = (from: number, to: number, t: number) => from + (to - from) * t;
export const round = (value: number) => Math.round(value * 1000) / 1000;

export const CHARACTER_TIMES = Array.from(SIGNATURE).reduce<number[]>((times, _char, index) => {
  times.push(index === 0 ? SIGNATURE_AT : times[index - 1] + (SIGNATURE[index - 1] === ' ' ? 110 : 65));
  return times;
}, []);
export const MASCOT_AT = CHARACTER_TIMES[CHARACTER_TIMES.length - 1] + 65;
export const LETTER_END = MASCOT_AT + MASCOT_MS;

export interface Point { x: number; y: number }
export interface LetterBox { left: number; top: number; width: number }
export type Matrix = [number, number, number, number, number, number];
type Beat = [number, number];

export function sampleHandoffBox(progress: number, from: LetterBox, to: LetterBox): LetterBox {
  const t = clamp(progress);
  const eased = t * t * t * (t * (t * 6 - 15) + 10);
  return { left: mix(from.left, to.left, eased), top: mix(from.top, to.top, eased), width: mix(from.width, to.width, eased) };
}

const beats = {
  squash: [[.13,.82],[.20,.79],[.27,1.16],[.42,1.02],[.55,1.06],[.635,.82],[.70,1.055],[.78,1.035],[.835,.94],[.90,1.015],[.97,.997]],
  lift: [[.205,0],[.28,-29],[.405,-53],[.475,-49],[.565,-27],[.635,0],[.66,0],[.745,-11],[.835,0],[.86,0],[.91,-2.1],[.965,0]],
  x: [[.16,3],[.29,8],[.45,2],[.635,-3],[.76,1],[.90,0]],
  tilt: [[.16,-12],[.255,-18],[.43,12],[.615,9],[.67,-14],[.80,7],[.91,-2],[.975,.6]],
  rock: [[.16,-5],[.29,-2],[.45,6],[.635,-3.5],[.76,2],[.84,-1],[.94,.3]],
  shear: [[.18,-.025],[.30,.025],[.48,.008],[.635,-.025],[.78,.012],[.90,0]],
  eyeOpen: [[.16,.70],[.29,1.15],[.46,1.08],[.635,.20],[.70,1],[.835,.70],[.90,1]],
  gazeX: [[.16,-1.4],[.44,.6],[.65,-.8],[.85,-.6]],
  gazeY: [[.19,-1.6],[.43,-2],[.615,.8],[.70,.2],[.85,.6]],
} satisfies Record<string, Beat[]>;

function compileCurve(keys: Beat[], rest: number) {
  const points: Beat[] = [[0, rest], ...keys, [1, rest]];
  const secant = (a: Beat, b: Beat) => (b[1] - a[1]) / (b[0] - a[0]);
  const slopes = points.map((point, i) => {
    if (i === 0 || i === points.length - 1) return 0;
    const a = secant(points[i - 1], point), b = secant(point, points[i + 1]);
    const h0 = point[0] - points[i - 1][0], h1 = points[i + 1][0] - point[0];
    return a * b <= 0 ? 0 : 3 * (h0 + h1) / ((2 * h1 + h0) / a + (h1 + 2 * h0) / b);
  });
  return (p: number) => {
    let i = 0;
    while (i < points.length - 2 && p > points[i + 1][0]) i++;
    const [t0, v0] = points[i], [t1, v1] = points[i + 1];
    const h = t1 - t0, u = clamp((p - t0) / h), u2 = u * u, u3 = u2 * u;
    return (2*u3 - 3*u2 + 1)*v0 + (u3 - 2*u2 + u)*h*slopes[i]
      + (-2*u3 + 3*u2)*v1 + (u3 - u2)*h*slopes[i + 1];
  };
}

type PoseKey = keyof typeof beats;
const poseKeys = Object.keys(beats) as PoseKey[];
const curves = Object.fromEntries(poseKeys.map(key => [key, compileCurve(beats[key], key === 'squash' || key === 'eyeOpen' ? 1 : 0)])) as Record<PoseKey, (p: number) => number>;
const translate = (x: number, y: number): Matrix => [1, 0, 0, 1, x, y];
const rotate = (degrees: number): Matrix => {
  const a = degrees * Math.PI / 180;
  return [Math.cos(a), Math.sin(a), -Math.sin(a), Math.cos(a), 0, 0];
};
const multiply = (a: Matrix, b: Matrix): Matrix => [
  a[0]*b[0]+a[2]*b[1], a[1]*b[0]+a[3]*b[1], a[0]*b[2]+a[2]*b[3],
  a[1]*b[2]+a[3]*b[3], a[0]*b[4]+a[2]*b[5]+a[4], a[1]*b[4]+a[3]*b[5]+a[5],
];
export const transformPoint = (m: Matrix, p: Point): Point => ({ x: m[0]*p.x+m[2]*p.y+m[4], y: m[1]*p.x+m[3]*p.y+m[5] });
export const formatMatrix = (matrix: Matrix) => `matrix(${matrix.map(value => Number(value.toFixed(6))).join(' ')})`;

export function sampleMascot(progress: number) {
  const p = clamp(progress);
  const fx = Object.fromEntries(poseKeys.map(key => [key, curves[key](p)])) as Record<PoseKey, number>;
  const sx = 1 / Math.sqrt(fx.squash), sy = fx.squash;
  const deform: Matrix = [sx, 0, fx.shear, sy, 53*(1-sx)-fx.shear*132, 132*(1-sy)];
  const mount = transformPoint(deform, { x: 53, y: 59.8 });
  const body = multiply(multiply(translate(53, 132), rotate(fx.rock)), translate(-53, -132));
  const feet = [{ x: 0, y: 104.5 }, { x: 53, y: 132 }, { x: 106, y: 104.5 }];
  body[5] += 132 - Math.max(...feet.map(foot => transformPoint(body, transformPoint(deform, foot)).y));
  const head = multiply(multiply(translate(mount.x, mount.y), rotate(fx.tilt)), translate(-52.965, -59.8));
  const height = Math.max(0, -fx.lift), airborne = Math.min(1, height / 53);
  return {
    deform, body, head, lift: translate(fx.x, fx.lift), fx, alpha: smooth(p / .07),
    shadow: { cx: 53 + fx.x*.55, cy: 134.8, rx: 36.04*(1-.38*airborne)/Math.sqrt(fx.squash),
      ry: 3 + height*.018, opacity: .2*(1-.72*airborne), blur: 3.33 + height*.065 },
  };
}
