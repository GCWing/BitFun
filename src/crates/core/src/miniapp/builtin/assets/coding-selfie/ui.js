// Built-in MiniApp: 每日编码快照 — full-page Bento dashboard (OLED dark).

const $ = (id) => document.getElementById(id);

// Single-accent palette: greens for the brand, neutral muted for "other".
const LANG_COLORS = [
  '#22c55e', '#10b981', '#06b6d4', '#3b82f6', '#a855f7',
  '#ec4899', '#f59e0b', '#ef4444', '#14b8a6', '#eab308',
];

// Lucide-style inline SVG strings.
const SVG = {
  loader: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>',
  camera: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"/><circle cx="12" cy="13" r="3"/></svg>',
  folder: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 1z"/></svg>',
  gitBranch: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>',
  alert: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>',
  trendUp: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>',
  trendDown: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 18 13.5 8.5 8.5 13.5 1 6"/><polyline points="17 18 23 18 23 12"/></svg>',
  minus: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="5" y1="12" x2="19" y2="12"/></svg>',
  moon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>',
  sun: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/></svg>',
  coffee: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 8h1a4 4 0 1 1 0 8h-1"/><path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z"/><line x1="6" y1="2" x2="6" y2="4"/><line x1="10" y1="2" x2="10" y2="4"/><line x1="14" y1="2" x2="14" y2="4"/></svg>',
  zap: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>',
  star: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>',
};

// Coding-style classifier driven by the busiest hour of the week.
// Style label keys are locale-agnostic; resolved via I18N at render time.
const STYLES = [
  { range: [0, 5],   icon: SVG.moon,   key: 'styleNight' },
  { range: [5, 9],   icon: SVG.sun,    key: 'styleMorning' },
  { range: [9, 12],  icon: SVG.coffee, key: 'styleAm' },
  { range: [12, 18], icon: SVG.zap,    key: 'stylePm' },
  { range: [18, 23], icon: SVG.star,   key: 'styleEvening' },
  { range: [23, 24], icon: SVG.moon,   key: 'styleNight' },
];

function styleForHour(h) {
  for (const s of STYLES) if (h >= s.range[0] && h < s.range[1]) return s;
  return STYLES[STYLES.length - 1];
}

// ---- i18n -------------------------------------------------------------
const I18N = {
  'zh-CN': {
    title: '每日编码快照',
    subtitle: '扫描你的本地 Git 仓库，凝结成一张今天的编码画像。',
    streakLabel: '连续编码',
    streakUnit: 'DAYS',
    todayCommits: '今日提交',
    codeChanges: '代码变动',
    filesTouched: '涉及文件',
    touchedToday: '今日变动',
    languagesUsed: '使用语言',
    donutTitle: '本周语言分布',
    commits: 'commits',
    hoursTitle: '24 小时活跃节律',
    thisWeek: '本周',
    hoursAria: '24 小时提交分布',
    heatmapTitle: '编码热力',
    heatmapAria: '52 周提交热力',
    less: '少',
    more: '多',
    commitsTitle: '今日提交',
    brand: '每日编码快照',
    weekShort: ['日', '一', '二', '三', '四', '五', '六'],
    just: '刚刚',
    noCommitsThisWeek: '本周尚无提交',
    noActivityThisWeek: '本周尚无活动',
    codingStyle: 'CODING STYLE',
    other: '其他',
    last52: (total, days) => `近 52 周 · ${total} 提交 · 活跃 ${days} 天`,
    perCellTitle: (date, count) => `${date} · ${count} 提交`,
    hourTitle: (hh, count) => `${hh}:00 — ${count} 提交`,
    noCommitsToday: '今天还没有提交，去写下第一行代码吧。',
    showXofY: (shown, total) => `显示 ${shown} / 共 ${total} 次`,
    totalX: (total) => `共 ${total} 次提交`,
    greetings: { lateNight: '夜深了', morning: '早上好', am: '上午好', noon: '中午好', pm: '下午好', evening: '晚上好' },
    greetWith: (part, name) => name ? `${part}，${name}` : part,
    subtitleEmpty: '今天还没开张 — 一次小提交，就能让今天的画像不再空白。',
    subtitleSprint: (n) => `今天 ${n} 次提交，全力冲刺中。`,
    subtitleSteady: (n) => `今天 ${n} 次提交，节奏稳健。`,
    subtitleStart: (n) => `今天 ${n} 次提交，刚刚起步。`,
    deltaUp: (n) => `较昨日 +${n}`,
    deltaDown: (n) => `较昨日 ${n}`,
    deltaSame: '与昨日持平',
    firstToday: 'first commits today',
    streakFoot: [
      { lt: 1, text: '今日打个卡，开启新连胜' },
      { lt: 3, text: '坚持下去，节奏才刚开始' },
      { lt: 7, text: '已经连写了一阵子' },
      { lt: 30, text: '稳定的产出节奏' },
      { lt: 100, text: '形成肌肉记忆，状态在线' },
      { lt: 200, text: '难以置信的坚持力' },
      { lt: Infinity, text: '你正在创造历史' },
    ],
    netPos: (n) => `net +${n}`,
    netNeg: (n) => `net ${n}`,
    allBranches: 'all branches',
    showingAll: 'showing commits from all authors · all branches',
    authorTitle: (name, emails) => `Filtering commits across all branches by:\n  name: ${name}\n  emails: ${emails.join('\n          ')}`,
    snapTimeTitle: (s) => `快照时间 ${s}`,
    loadingTitle: '正在扫描代码足迹',
    loadingTitleFirst: '正在扫描你的代码足迹',
    errNoWs: '还没有打开工作区',
    errNoWsDesc: '请先在 BitFun 侧边栏选择一个 Git 仓库的工作区。',
    errNotGit: '当前工作区不是 Git 仓库',
    errNotGitDesc: (p) => `${p} · 执行 git init 或切换到 Git 仓库`,
    errNoWsShort: '未检测到工作区',
    errNoWsShortDesc: '请先打开一个工作区',
    errScan: '扫描失败',
    errScanReason: (r) => `原因：${r}`,
    errScanRuntime: '扫描出错',
    styleNight: '凌晨刺客',
    styleMorning: '早起鸟',
    styleAm: '上午型选手',
    stylePm: '下午冲刺者',
    styleEvening: '夜猫子',
  },
  'en-US': {
    title: 'Daily Coding Snapshot',
    subtitle: 'Scans your local Git repo and crystallizes today into one coding portrait.',
    streakLabel: 'Streak',
    streakUnit: 'DAYS',
    todayCommits: 'Commits Today',
    codeChanges: 'Lines',
    filesTouched: 'Files',
    touchedToday: 'touched today',
    languagesUsed: 'Languages',
    donutTitle: 'This Week · Languages',
    commits: 'commits',
    hoursTitle: '24h Activity Rhythm',
    thisWeek: 'this week',
    hoursAria: '24h commit distribution',
    heatmapTitle: 'Coding Heatmap',
    heatmapAria: '52-week commit heatmap',
    less: 'less',
    more: 'more',
    commitsTitle: 'Today Commits',
    brand: 'Daily Coding Snapshot',
    weekShort: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
    just: 'just now',
    noCommitsThisWeek: 'No commits this week',
    noActivityThisWeek: 'No activity this week',
    codingStyle: 'CODING STYLE',
    other: 'Other',
    last52: (total, days) => `Last 52w · ${total} commits · ${days} active days`,
    perCellTitle: (date, count) => `${date} · ${count} commits`,
    hourTitle: (hh, count) => `${hh}:00 — ${count} commits`,
    noCommitsToday: 'No commits yet today — write that first line.',
    showXofY: (shown, total) => `Showing ${shown} / ${total}`,
    totalX: (total) => `${total} commits`,
    greetings: { lateNight: 'Burning the midnight oil', morning: 'Good morning', am: 'Good morning', noon: 'Good noon', pm: 'Good afternoon', evening: 'Good evening' },
    greetWith: (part, name) => name ? `${part}, ${name}` : part,
    subtitleEmpty: 'Nothing yet today — one tiny commit fills the canvas.',
    subtitleSprint: (n) => `${n} commits today — full sprint mode.`,
    subtitleSteady: (n) => `${n} commits today — steady rhythm.`,
    subtitleStart: (n) => `${n} commits today — just getting started.`,
    deltaUp: (n) => `+${n} vs yesterday`,
    deltaDown: (n) => `${n} vs yesterday`,
    deltaSame: 'same as yesterday',
    firstToday: 'first commits today',
    streakFoot: [
      { lt: 1, text: 'Punch in today and start a new streak' },
      { lt: 3, text: 'Keep going — the rhythm is just starting' },
      { lt: 7, text: "You've been at it for a while" },
      { lt: 30, text: 'Solid steady output' },
      { lt: 100, text: 'Muscle memory — fully in the zone' },
      { lt: 200, text: 'Unbelievable persistence' },
      { lt: Infinity, text: "You're making history" },
    ],
    netPos: (n) => `net +${n}`,
    netNeg: (n) => `net ${n}`,
    allBranches: 'all branches',
    showingAll: 'showing commits from all authors · all branches',
    authorTitle: (name, emails) => `Filtering commits across all branches by:\n  name: ${name}\n  emails: ${emails.join('\n          ')}`,
    snapTimeTitle: (s) => `Snapshot time ${s}`,
    loadingTitle: 'Scanning your code footprint',
    loadingTitleFirst: 'Scanning your code footprint',
    errNoWs: 'No workspace open',
    errNoWsDesc: 'Open a Git repo workspace from the BitFun sidebar first.',
    errNotGit: 'Workspace is not a Git repo',
    errNotGitDesc: (p) => `${p} · run \`git init\` or switch to a Git repo`,
    errNoWsShort: 'No workspace detected',
    errNoWsShortDesc: 'Open a workspace first',
    errScan: 'Scan failed',
    errScanReason: (r) => `reason: ${r}`,
    errScanRuntime: 'Scan error',
    styleNight: 'Midnight Hacker',
    styleMorning: 'Early Bird',
    styleAm: 'Morning Brew',
    stylePm: 'Afternoon Sprinter',
    styleEvening: 'Night Owl',
  },
};

function currentLocale() {
  const l = window.app && window.app.locale;
  if (l && I18N[l]) return l;
  if (l && typeof l === 'string') {
    const base = l.split('-')[0];
    for (const k of Object.keys(I18N)) if (k.startsWith(base + '-')) return k;
  }
  return 'en-US';
}
function t(key) {
  const dict = I18N[currentLocale()] || I18N['en-US'];
  return dict[key];
}

function applyStaticI18n() {
  const root = document.getElementById('root');
  if (!root) return;
  root.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    const v = t(key);
    if (typeof v !== 'string') return;
    const attr = el.getAttribute('data-i18n-attr');
    if (attr) el.setAttribute(attr, v);
    else el.textContent = v;
  });
  document.documentElement.setAttribute('lang', currentLocale());
}

function fmtDate(d) {
  const dt = new Date(d);
  const week = t('weekShort') || ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const dayLabel = currentLocale() === 'zh-CN'
    ? `周${week[dt.getDay()]}`
    : week[dt.getDay()];
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')} ${dayLabel}`;
}
function fmtShortDate(d) {
  const dt = new Date(d);
  return `${dt.getMonth() + 1}/${dt.getDate()}`;
}
function relativeTime(iso) {
  const diff = Math.max(0, Date.now() - new Date(iso).getTime());
  const m = Math.floor(diff / 60000);
  if (m < 1) return t('just');
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}
function greetingFor(d, name) {
  const h = d.getHours();
  const g = t('greetings') || {};
  const part = h < 5 ? g.lateNight : h < 9 ? g.morning : h < 12 ? g.am : h < 14 ? g.noon : h < 18 ? g.pm : h < 22 ? g.evening : g.lateNight;
  return t('greetWith')(part, name);
}

function setEmpty(state, title, desc, icon) {
  const el = $('empty');
  el.classList.remove('is-loading', 'is-error');
  if (state === 'loading') el.classList.add('is-loading');
  if (state === 'error') el.classList.add('is-error');
  $('empty-icon').innerHTML = icon || SVG.loader;
  $('empty-title').textContent = title;
  $('empty-desc').textContent = desc || '';
}
function showEmpty(state, title, desc, icon) {
  setEmpty(state, title, desc, icon);
  $('empty').removeAttribute('hidden');
  $('content').setAttribute('hidden', '');
}
function hideEmpty() {
  $('empty').setAttribute('hidden', '');
  $('content').removeAttribute('hidden');
}

function renderDonut(langs) {
  const svg = $('donut');
  svg.innerHTML = '';
  const total = langs.reduce((acc, l) => acc + l.weight, 0);
  if (total <= 0) {
    svg.innerHTML = '<circle cx="60" cy="60" r="46" fill="none" stroke="rgba(148,163,184,0.12)" stroke-width="12" />';
    $('lang-legend').innerHTML = `<li class="cs-legend-row"><span></span><span class="cs-legend-name" style="color:var(--cs-text-muted)">${t('noCommitsThisWeek')}</span><span></span></li>`;
    return;
  }
  const top = langs.slice(0, 5);
  const otherWeight = langs.slice(5).reduce((acc, l) => acc + l.weight, 0);
  const slices = otherWeight > 0
    ? [...top, { name: t('other'), weight: otherWeight, isOther: true }]
    : top;

  const r = 46;
  const c = 2 * Math.PI * r;
  let offset = 0;

  // Track ring
  const track = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
  track.setAttribute('cx', '60');
  track.setAttribute('cy', '60');
  track.setAttribute('r', String(r));
  track.setAttribute('fill', 'none');
  track.setAttribute('stroke', 'var(--cs-card-2)');
  track.setAttribute('stroke-width', '12');
  svg.appendChild(track);

  for (let i = 0; i < slices.length; i++) {
    const s = slices[i];
    const frac = s.weight / total;
    const dash = c * frac;
    const color = s.isOther ? 'rgba(148,163,184,0.35)' : LANG_COLORS[i % LANG_COLORS.length];
    const arc = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
    arc.setAttribute('cx', '60');
    arc.setAttribute('cy', '60');
    arc.setAttribute('r', String(r));
    arc.setAttribute('fill', 'none');
    arc.setAttribute('stroke', color);
    arc.setAttribute('stroke-width', '12');
    arc.setAttribute('stroke-linecap', 'butt');
    arc.setAttribute('stroke-dasharray', `${dash} ${c - dash}`);
    arc.setAttribute('stroke-dashoffset', String(-offset));
    svg.appendChild(arc);
    offset += dash;
  }

  const legend = $('lang-legend');
  legend.innerHTML = '';
  slices.forEach((s, i) => {
    const li = document.createElement('li');
    li.className = 'cs-legend-row';
    const color = s.isOther ? 'rgba(148,163,184,0.35)' : LANG_COLORS[i % LANG_COLORS.length];
    li.innerHTML = `
      <span class="cs-legend-swatch" style="background:${color}"></span>
      <span class="cs-legend-name" title="${s.name}">${s.name}</span>
      <span class="cs-legend-pct">${(s.weight / total * 100).toFixed(1)}%</span>
    `;
    legend.appendChild(li);
  });
}

function renderHours(hours) {
  const wrap = $('hours');
  wrap.innerHTML = '';
  const max = Math.max(1, ...hours);
  let peakHour = 0, peakVal = 0;
  hours.forEach((v, i) => { if (v > peakVal) { peakVal = v; peakHour = i; } });
  hours.forEach((v, i) => {
    const bar = document.createElement('div');
    bar.className = 'cs-hour-bar';
    if (v > 0) bar.classList.add('has');
    if (v > 0 && v === peakVal) bar.classList.add('peak');
    const h = v > 0 ? Math.max(8, Math.round((v / max) * 100)) : 4;
    bar.style.height = `${h}%`;
    bar.title = t('hourTitle')(String(i).padStart(2, '0'), v);
    wrap.appendChild(bar);
  });

  const badge = $('style-badge');
  if (peakVal > 0) {
    const s = styleForHour(peakHour);
    badge.innerHTML = `
      <span class="cs-style-badge-label">${t('codingStyle')}</span>
      <span class="cs-style-badge-value">
        <span style="width:14px;height:14px;display:inline-flex">${s.icon}</span>
        ${t(s.key)} · ${String(peakHour).padStart(2, '0')}:00
      </span>`;
  } else {
    badge.innerHTML = `<span class="cs-style-badge-label">${t('codingStyle')}</span><span class="cs-style-badge-value" style="color:var(--cs-text-muted)">${t('noActivityThisWeek')}</span>`;
  }
}

// Heatmap rendering uses an adaptive multi-band layout: instead of forcing a
// 7×52 strip (which leaves huge empty space in narrow/tall tiles), we split
// the year into N stacked 7-row bands and pick the N that maximizes the
// square cell size for the actual tile dimensions.

let _heatmapCells = []; // flat list incl. leading/trailing pads
let _heatmapMeta = { total: 0, activeDays: 0 };

function renderHeatmap(heatmap) {
  const body = $('heatmap-body');
  if (!body) return;
  body.innerHTML = '';
  _heatmapCells = [];

  if (!heatmap.length) {
    $('heatmap-hint').textContent = '';
    return;
  }

  const counts = heatmap.map((d) => d.count);
  const max = Math.max(1, ...counts);
  const firstDate = new Date(heatmap[0].date + 'T00:00:00');
  const leadingEmpty = firstDate.getDay();

  // Leading pads (so first column starts on Sunday)
  for (let i = 0; i < leadingEmpty; i++) {
    _heatmapCells.push({ empty: true });
  }

  let total = 0, activeDays = 0;
  heatmap.forEach((d) => {
    const ratio = d.count / max;
    let lvl = 0;
    if (d.count > 0) {
      if (ratio < 0.25) lvl = 1;
      else if (ratio < 0.5) lvl = 2;
      else if (ratio < 0.85) lvl = 3;
      else lvl = 4;
    }
    _heatmapCells.push({ date: d.date, count: d.count, lvl });
    total += d.count;
    if (d.count > 0) activeDays += 1;
  });

  // Trailing pads to complete the last column
  const lastDate = new Date(heatmap[heatmap.length - 1].date + 'T00:00:00');
  const trailing = 6 - lastDate.getDay();
  for (let i = 0; i < trailing; i++) _heatmapCells.push({ empty: true });

  _heatmapMeta = { total, activeDays };
  $('heatmap-hint').textContent = t('last52')(total, activeDays);
  fitHeatmap();
}

const HEAT_GAP = 2;
const HEAT_BAND_GAP = 6;

// Adaptive heatmap that ALWAYS fills 100% of the tile. Strategy:
//   1) Try band counts 1..6 (each band is 7 rows × ceil(N/bands) cols).
//   2) For each, the resulting cell is a rectangle (cellW x cellH).
//   3) Score by aspect-ratio closeness to 1 (we prefer near-square cells but
//      we do NOT enforce squareness — that's what was leaving whitespace).
//   4) The chosen layout uses width:100% / height:100% with `1fr` tracks so
//      the grid stretches edge-to-edge regardless of the cell aspect.
function fitHeatmap() {
  const body = $('heatmap-body');
  if (!body || !_heatmapCells.length) return;
  const totalWeeks = _heatmapCells.length / 7;
  const availW = Math.max(0, body.clientWidth);
  const availH = Math.max(0, body.clientHeight);
  if (availW < 8 || availH < 8) return;

  let best = null;
  for (const bands of [1, 2, 3, 4, 5, 6]) {
    const weeksPerBand = Math.ceil(totalWeeks / bands);
    if (weeksPerBand < 3) continue;
    const cellW = (availW - (weeksPerBand - 1) * HEAT_GAP) / weeksPerBand;
    const totalRows = bands * 7;
    const cellH = (availH - bands * 6 * HEAT_GAP - (bands - 1) * HEAT_BAND_GAP) / totalRows;
    if (cellW < 1 || cellH < 1) continue;
    const aspect = Math.max(cellW / cellH, cellH / cellW);
    if (!best || aspect < best.aspect) {
      best = { bands, weeksPerBand, cellW, cellH, aspect };
    }
  }
  if (!best) best = { bands: 1, weeksPerBand: totalWeeks, cellW: 4, cellH: 4, aspect: 1 };

  body.innerHTML = '';
  const cellsPerBand = best.weeksPerBand * 7;
  for (let b = 0; b < best.bands; b++) {
    const grid = document.createElement('div');
    grid.className = 'cs-heatmap';
    const startIdx = b * cellsPerBand;
    const endIdx = Math.min(_heatmapCells.length, startIdx + cellsPerBand);
    const slice = _heatmapCells.slice(startIdx, endIdx);
    while (slice.length < cellsPerBand) slice.push({ empty: true });
    const weeks = slice.length / 7;
    // 1fr tracks fill the band edge-to-edge regardless of cell aspect.
    grid.style.gridTemplateColumns = `repeat(${weeks}, minmax(0, 1fr))`;
    grid.style.gridTemplateRows = `repeat(7, minmax(0, 1fr))`;
    grid.style.gap = `${HEAT_GAP}px`;
    grid.style.gridAutoFlow = 'column';
    grid.style.width = '100%';
    // Each band gets equal vertical share of the body.
    grid.style.flex = '1 1 0';
    grid.style.minHeight = '0';
    slice.forEach((c) => {
      const cell = document.createElement('div');
      if (c.empty) {
        cell.className = 'cs-heat-cell cs-heat-empty';
      } else {
        cell.className = `cs-heat-cell cs-heat-${c.lvl}`;
        cell.title = t('perCellTitle')(c.date, c.count);
      }
      grid.appendChild(cell);
    });
    body.appendChild(grid);
  }
}

let _heatmapRO = null;
function setupHeatmapResize() {
  if (_heatmapRO) return;
  const body = $('heatmap-body');
  if (!body || typeof ResizeObserver === 'undefined') {
    window.addEventListener('resize', fitHeatmap);
    return;
  }
  _heatmapRO = new ResizeObserver(() => fitHeatmap());
  _heatmapRO.observe(body);
}

function renderCommits(commits, totalToday) {
  const list = $('commits-list');
  list.innerHTML = '';
  if (!commits.length) {
    const li = document.createElement('li');
    li.className = 'cs-commits-empty';
    li.textContent = t('noCommitsToday');
    list.appendChild(li);
    return;
  }
  commits.forEach((c) => {
    const li = document.createElement('li');
    li.className = 'cs-commit';
    li.innerHTML = `
      <span class="cs-commit-hash">${c.hash}</span>
      <span class="cs-commit-subject" title="${escapeAttr(c.subject)}">${escapeHtml(c.subject)}</span>
      <span class="cs-commit-time">${relativeTime(c.date)}</span>
      <span class="cs-commit-stat"><span class="cs-add">+${c.added}</span><span class="cs-del">-${c.deleted}</span></span>
    `;
    list.appendChild(li);
  });
  // Visible count is bounded by CSS overflow:hidden in the list — surface a
  // clearer breakdown so "10" never reads as a lonely number on the edge.
  $('commits-hint').textContent = totalToday > commits.length
    ? t('showXofY')(commits.length, totalToday)
    : t('totalX')(totalToday);
}

function renderHero(data) {
  const now = new Date();
  $('hero-greeting').textContent = greetingFor(now, data.author.name);

  // Bottom meta footer (replaces the old in-hero chips)
  const repoEl = $('meta-repo');
  if (repoEl) {
    repoEl.textContent = `${data.repo.name}@${data.repo.branch}`;
    repoEl.parentElement.title = data.repo.path;
  }
  const dateEl = $('meta-date');
  if (dateEl) dateEl.textContent = fmtDate(now);
  const snap = new Date(data.generatedAt || Date.now());
  const timeEl = $('meta-time');
  if (timeEl) {
    timeEl.textContent = `${String(snap.getHours()).padStart(2, '0')}:${String(snap.getMinutes()).padStart(2, '0')}`;
    timeEl.parentElement.title = t('snapTimeTitle')(snap.toLocaleTimeString());
  }

  const td = data.today;
  let subtitle;
  if (td.commitCount === 0) {
    subtitle = t('subtitleEmpty');
  } else if (td.commitCount >= 8) {
    subtitle = t('subtitleSprint')(td.commitCount);
  } else if (td.commitCount >= 4) {
    subtitle = t('subtitleSteady')(td.commitCount);
  } else {
    subtitle = t('subtitleStart')(td.commitCount);
  }
  $('hero-subtitle').textContent = subtitle;

  const emails = (data.author.detectedEmails || []).filter(Boolean);
  const heroAuthor = $('hero-author');
  const allBranches = t('allBranches');
  if (data.author.name && emails.length) {
    const shownEmails = emails.length <= 2
      ? emails.join(', ')
      : `${emails.slice(0, 2).join(', ')} +${emails.length - 2}`;
    heroAuthor.textContent = `${data.author.name} · ${shownEmails} · ${allBranches}`;
    heroAuthor.title = t('authorTitle')(data.author.name, emails);
  } else if (data.author.name) {
    heroAuthor.textContent = `${data.author.name} · ${allBranches}`;
    heroAuthor.title = '';
  } else {
    heroAuthor.textContent = t('showingAll');
    heroAuthor.title = '';
  }
}

function renderStats(data) {
  const td = data.today;
  $('stat-commits').textContent = String(td.commitCount);
  $('stat-add').textContent = `+${td.added}`;
  $('stat-del').textContent = `-${td.deleted}`;
  $('stat-files').textContent = String(td.fileCount);
  $('stat-langs').textContent = String(td.langs.length);

  const net = td.added - td.deleted;
  $('stat-net').textContent = net >= 0 ? t('netPos')(net) : t('netNeg')(net);

  $('stat-langs-list').textContent = td.langs.length
    ? td.langs.slice(0, 3).map((l) => l.name).join(' · ')
    : '—';

  const delta = td.commitCount - data.yesterday.commitCount;
  const dEl = $('stat-commits-delta');
  dEl.classList.remove('up', 'down');
  if (delta > 0) {
    dEl.innerHTML = `${SVG.trendUp} ${t('deltaUp')(delta)}`;
    dEl.classList.add('up');
  } else if (delta < 0) {
    dEl.innerHTML = `${SVG.trendDown} ${t('deltaDown')(delta)}`;
    dEl.classList.add('down');
  } else if (data.yesterday.commitCount > 0) {
    dEl.innerHTML = `${SVG.minus} ${t('deltaSame')}`;
  } else {
    dEl.textContent = t('firstToday');
  }
}

function renderStreak(data) {
  $('streak-num').textContent = String(data.streak);
  const table = t('streakFoot') || [];
  let foot = '';
  for (const row of table) {
    if (data.streak < row.lt) { foot = row.text; break; }
  }
  $('streak-foot').textContent = foot;
}

function render(data) {
  hideEmpty();
  renderHero(data);
  renderStreak(data);
  renderStats(data);
  renderDonut(data.week.langs);
  $('donut-total').textContent = String(data.week.commitCount);
  renderHours(data.week.hours);
  renderHeatmap(data.heatmap);
  setupHeatmapResize();
  renderCommits(data.today.commits, data.today.commitCount);
}

function buildMarkdown(data) {
  const td = data.today;
  const date = fmtDate(new Date());
  const lines = [];
  lines.push(`# ${t('title')} · ${date}`);
  lines.push('');
  lines.push(`> \`${data.repo.name}@${data.repo.branch}\`${data.author.name ? ` · ${data.author.name}` : ''}`);
  lines.push('');
  lines.push('| Metric | Today | Yesterday |');
  lines.push('|---|---|---|');
  lines.push(`| Commits | **${td.commitCount}** | ${data.yesterday.commitCount} |`);
  lines.push(`| Lines  | **+${td.added} / -${td.deleted}** | +${data.yesterday.added} / -${data.yesterday.deleted} |`);
  lines.push(`| Files  | **${td.fileCount}** | ${data.yesterday.fileCount} |`);
  lines.push(`| Streak | **${data.streak} days** | — |`);

  if (td.langs.length) {
    lines.push('');
    lines.push(`**Languages:** ${td.langs.slice(0, 5).map((l) => l.name).join(' · ')}`);
  }

  let peakHour = 0, peakVal = 0;
  data.week.hours.forEach((v, i) => { if (v > peakVal) { peakVal = v; peakHour = i; } });
  if (peakVal > 0) {
    const s = styleForHour(peakHour);
    lines.push(`**Style:** ${t(s.key)} (peak ${String(peakHour).padStart(2, '0')}:00)`);
  }

  if (td.commits.length) {
    lines.push('');
    lines.push('**Today commits:**');
    lines.push('');
    td.commits.forEach((c) => {
      lines.push(`- \`${c.hash}\` ${c.subject} _(+${c.added} -${c.deleted})_`);
    });
  }

  lines.push('');
  lines.push('---');
  lines.push(`_Generated by BitFun · ${t('title')}_`);
  return lines.join('\n');
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}
function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, '&quot;');
}

// ---- Git scan (host-side shell, no Node/Bun worker) -----------------------
// Mirrors the legacy `worker.js` `workspace.scan` output shape so the rest of
// the UI does not need to change. Runs entirely from the iframe via
// `app.shell.exec`, which the host now serves directly with the same
// permission policy (shell.allow=["git"], fs.read=["{workspace}", ...]).

const EXT_TO_LANG = {
  '.ts': 'TypeScript', '.tsx': 'TSX', '.js': 'JavaScript', '.jsx': 'JSX', '.mjs': 'JavaScript',
  '.cjs': 'JavaScript', '.py': 'Python', '.rs': 'Rust', '.go': 'Go', '.java': 'Java',
  '.kt': 'Kotlin', '.swift': 'Swift', '.cpp': 'C++', '.cc': 'C++', '.c': 'C', '.h': 'C/C++',
  '.hpp': 'C++', '.cs': 'C#', '.rb': 'Ruby', '.php': 'PHP', '.scala': 'Scala', '.sh': 'Shell',
  '.bash': 'Shell', '.zsh': 'Shell', '.css': 'CSS', '.scss': 'SCSS', '.sass': 'Sass',
  '.less': 'Less', '.html': 'HTML', '.htm': 'HTML', '.json': 'JSON', '.md': 'Markdown',
  '.mdx': 'MDX', '.yml': 'YAML', '.yaml': 'YAML', '.toml': 'TOML', '.ini': 'INI', '.sql': 'SQL',
  '.vue': 'Vue', '.svelte': 'Svelte', '.lua': 'Lua', '.dart': 'Dart', '.r': 'R', '.proto': 'Protobuf',
  '.gradle': 'Gradle', '.tf': 'Terraform', '.hcl': 'HCL', '.ex': 'Elixir', '.exs': 'Elixir',
  '.erl': 'Erlang', '.elm': 'Elm', '.zig': 'Zig', '.nim': 'Nim', '.jl': 'Julia', '.clj': 'Clojure',
  '.cljs': 'ClojureScript', '.fs': 'F#', '.ml': 'OCaml', '.coffee': 'CoffeeScript', '.xml': 'XML',
};

function langOf(file) {
  const slash = Math.max(file.lastIndexOf('/'), file.lastIndexOf('\\'));
  const base = (slash >= 0 ? file.slice(slash + 1) : file).toLowerCase();
  if (base === 'dockerfile' || base.endsWith('.dockerfile')) return 'Docker';
  if (base === 'makefile') return 'Make';
  if (base === 'cmakelists.txt') return 'CMake';
  const dot = base.lastIndexOf('.');
  if (dot < 0) return null;
  return EXT_TO_LANG[base.slice(dot)] || null;
}

function basenameOf(p) {
  if (!p) return '';
  const segs = p.split(/[\\/]/).filter(Boolean);
  return segs.length ? segs[segs.length - 1] : p;
}

// POSIX single-quote shell escape: wrap with ' and escape inner ' as '\''.
function shq(s) {
  if (s == null) return "''";
  return `'${String(s).replace(/'/g, "'\\''")}'`;
}

async function gitRun(cwd, argv, opts = {}) {
  const cmd = ['git', ...argv.map(shq)].join(' ');
  const res = await window.app.shell.exec(cmd, { cwd, timeout: opts.timeout || 30000 });
  return res.stdout || '';
}

async function gitRunOptional(cwd, argv, fallback = '', opts = {}) {
  try {
    return await gitRun(cwd, argv, opts);
  } catch (_e) {
    return fallback;
  }
}

function dayKey(d) {
  const dt = new Date(d);
  return (
    dt.getFullYear() +
    '-' +
    String(dt.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(dt.getDate()).padStart(2, '0')
  );
}

function summarize(cs) {
  let added = 0, deleted = 0;
  const langs = new Map();
  const files = new Set();
  const hours = new Array(24).fill(0);
  for (const c of cs) {
    added += c.added;
    deleted += c.deleted;
    for (const f of c.files) {
      files.add(f.path);
      if (f.lang) {
        const t = (f.added + f.deleted) || 1;
        langs.set(f.lang, (langs.get(f.lang) || 0) + t);
      }
    }
    const h = new Date(c.date).getHours();
    if (h >= 0 && h < 24) hours[h] += 1;
  }
  const langArr = Array.from(langs.entries())
    .map(([name, weight]) => ({ name, weight }))
    .sort((a, b) => b.weight - a.weight);
  return {
    commitCount: cs.length,
    added,
    deleted,
    fileCount: files.size,
    langs: langArr,
    hours,
    commits: cs.slice(0, 12).map((c) => ({
      hash: c.hash.slice(0, 7),
      date: c.date,
      author: c.author,
      subject: c.subject,
      added: c.added,
      deleted: c.deleted,
    })),
  };
}

async function scanGitWorkspace(cwd) {
  if (!cwd) return { ok: false, reason: 'no-workspace' };

  // 1) Verify it is a git work tree.
  let inside;
  try {
    inside = (await gitRun(cwd, ['rev-parse', '--is-inside-work-tree'], { timeout: 8000 })).trim();
  } catch (_e) {
    return { ok: false, reason: 'not-a-git-repo' };
  }
  if (inside !== 'true') return { ok: false, reason: 'not-a-git-repo' };

  // 2) Repo basics.
  const [topLevelRaw, branchRaw, userNameRaw, userEmailRaw] = await Promise.all([
    gitRunOptional(cwd, ['rev-parse', '--show-toplevel'], cwd, { timeout: 8000 }),
    gitRunOptional(cwd, ['rev-parse', '--abbrev-ref', 'HEAD'], 'HEAD', { timeout: 8000 }),
    gitRunOptional(cwd, ['config', 'user.name'], '', { timeout: 5000 }),
    gitRunOptional(cwd, ['config', 'user.email'], '', { timeout: 5000 }),
  ]);
  const topLevel = (topLevelRaw || cwd).trim() || cwd;
  const branch = (branchRaw || 'HEAD').trim() || 'HEAD';
  const userName = (userNameRaw || '').trim();
  const userEmail = (userEmailRaw || '').trim();
  const repoName = basenameOf(topLevel);

  // 3) Auto-discover every email the current author has committed with in this
  //    repo. Same logic as the old worker — handles squash-merge no-reply emails
  //    and multiple identities for the same name.
  const detectedEmails = new Set();
  if (userEmail) detectedEmails.add(userEmail);
  if (userName) {
    const emailMap = await gitRunOptional(
      topLevel,
      ['log', '--all', '--pretty=format:%aN\t%aE'],
      '',
      { timeout: 12000 },
    );
    for (const line of emailMap.split('\n')) {
      const tab = line.indexOf('\t');
      if (tab < 0) continue;
      const n = line.slice(0, tab).trim();
      const e = line.slice(tab + 1).trim();
      if (n && e && n === userName) detectedEmails.add(e);
    }
  }

  const escRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const patterns = [];
  if (userName) patterns.push(escRe(userName));
  for (const e of detectedEmails) patterns.push(escRe(e));
  const authorPattern = patterns.length ? patterns.join('\\|') : '';

  // 4) Pull last 52 weeks of commits with numstat across ALL branches.
  const SEP = '\x1f';
  const REC = '\x1e';
  const fmt = `${REC}%H${SEP}%aI${SEP}%aN${SEP}%aE${SEP}%s`;
  const args = [
    'log',
    '--all',
    '--since=52.weeks',
    '--no-merges',
    `--pretty=format:${fmt}`,
    '--numstat',
  ];
  if (authorPattern) args.push(`--author=${authorPattern}`);

  let raw;
  try {
    raw = await gitRun(topLevel, args, { timeout: 30000 });
  } catch (e) {
    return { ok: false, reason: 'git-log-failed', message: String(e && e.message ? e.message : e) };
  }

  const commits = [];
  for (const rec of raw.split(REC)) {
    const trimmed = rec.replace(/^\n+/, '');
    if (!trimmed) continue;
    const lines = trimmed.split('\n');
    const parts = (lines[0] || '').split(SEP);
    if (parts.length < 5) continue;
    const [hash, date, author, email, subject] = parts;
    const files = [];
    let added = 0, deleted = 0;
    for (let i = 1; i < lines.length; i++) {
      const ln = lines[i];
      if (!ln) continue;
      const m = ln.match(/^(\d+|-)\t(\d+|-)\t(.+)$/);
      if (!m) continue;
      const a = m[1] === '-' ? 0 : parseInt(m[1], 10);
      const d = m[2] === '-' ? 0 : parseInt(m[2], 10);
      added += a;
      deleted += d;
      files.push({ path: m[3], added: a, deleted: d, lang: langOf(m[3]) });
    }
    commits.push({ hash, date, author, email, subject, added, deleted, files });
  }

  // 5) Aggregate by day (local time).
  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const yesterdayStart = new Date(todayStart);
  yesterdayStart.setDate(todayStart.getDate() - 1);

  const byDay = new Map();
  for (const c of commits) {
    const k = dayKey(c.date);
    if (!byDay.has(k)) byDay.set(k, []);
    byDay.get(k).push(c);
  }

  const todayKey = dayKey(todayStart);
  const yesterdayKey = dayKey(yesterdayStart);
  const today = summarize(byDay.get(todayKey) || []);
  const yesterday = summarize(byDay.get(yesterdayKey) || []);

  const weekCutoff = new Date(todayStart.getTime() - 6 * 86400000);
  const week = summarize(commits.filter((c) => new Date(c.date) >= weekCutoff));
  const month = summarize(commits);

  // 6) Streak: consecutive days backwards with at least one commit. Counts today
  //    if there are commits today; otherwise starts from yesterday.
  let streak = 0;
  const cursor = new Date(todayStart);
  if (!byDay.has(dayKey(cursor))) {
    cursor.setDate(cursor.getDate() - 1);
  }
  while (byDay.has(dayKey(cursor))) {
    streak += 1;
    cursor.setDate(cursor.getDate() - 1);
  }

  // 7) Year heatmap: 7 * 52 = 364 days, oldest → today.
  const HEATMAP_DAYS = 7 * 52;
  const heatmap = [];
  for (let i = HEATMAP_DAYS - 1; i >= 0; i--) {
    const d = new Date(todayStart);
    d.setDate(d.getDate() - i);
    const k = dayKey(d);
    const list = byDay.get(k) || [];
    heatmap.push({ date: k, count: list.length });
  }

  return {
    ok: true,
    repo: { name: repoName, path: topLevel, branch },
    author: {
      name: userName,
      email: userEmail,
      detectedEmails: Array.from(detectedEmails),
      scope: 'all-branches',
    },
    today,
    yesterday,
    week,
    month,
    streak,
    heatmap,
    generatedAt: new Date().toISOString(),
  };
}

let lastData = null;
let scanning = false;

async function scan() {
  if (scanning) return;
  scanning = true;

  const ws = (window.app && window.app.workspaceDir) || '';

  if (!ws) {
    showEmpty('error', t('errNoWs'), t('errNoWsDesc'), SVG.folder);
    lastData = null;
    scanning = false;
    return;
  }

  // Only show the full loading screen on the very first scan. Subsequent
  // re-scans (triggered by `onActivate`) silently update the dashboard.
  if (!lastData) {
    showEmpty('loading', t('loadingTitle'), shortPath(ws), SVG.loader);
  }

  try {
    const result = await scanGitWorkspace(ws);
    if (!result || !result.ok) {
      const reason = (result && result.reason) || 'unknown';
      if (reason === 'not-a-git-repo') {
        showEmpty('error', t('errNotGit'), t('errNotGitDesc')(shortPath(ws)), SVG.gitBranch);
      } else if (reason === 'no-workspace') {
        showEmpty('error', t('errNoWsShort'), t('errNoWsShortDesc'), SVG.folder);
      } else {
        showEmpty('error', t('errScan'), (result && result.message) || t('errScanReason')(reason), SVG.alert);
      }
      lastData = null;
    } else {
      lastData = result;
      render(result);
    }
  } catch (err) {
    showEmpty('error', t('errScanRuntime'), String(err && err.message ? err.message : err), SVG.alert);
    lastData = null;
  } finally {
    scanning = false;
  }
}

function shortPath(p) {
  if (!p) return '';
  const home = (typeof navigator !== 'undefined' && /Mac|Linux/.test(navigator.platform)) ? null : null;
  // Truncate from the left, keeping the last 2 segments visible.
  const segs = p.split(/[\\/]/).filter(Boolean);
  if (segs.length <= 3) return p;
  const tail = segs.slice(-2).join('/');
  return `…/${tail}`;
}

applyStaticI18n();
window.app?.onActivate?.(scan);
window.app?.onLocaleChange?.(() => {
  applyStaticI18n();
  if (lastData) render(lastData);
});
scan();
