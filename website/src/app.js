import { matchingItems, queryTokens, scoreCapability } from './search.js';

const root = document.querySelector('#app');
const pageData = JSON.parse(document.querySelector('#page-data')?.textContent ?? '{}');
const THEME_STORAGE_KEY = 'bitfun-playbook-theme';
const SIDEBAR_SCROLL_STORAGE_KEY = 'bitfun-playbook-sidebar-scroll';
const THEME_CHOICES = ['system', 'light', 'dark'];

function readPreference(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writePreference(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // The selected option still applies for this page when storage is unavailable.
  }
}

function readSessionPreference(key) {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeSessionPreference(key, value) {
  try {
    sessionStorage.setItem(key, value);
  } catch {
    // Sidebar continuity is a progressive enhancement when storage is unavailable.
  }
}

function storedTheme() {
  const value = readPreference(THEME_STORAGE_KEY);
  return THEME_CHOICES.includes(value) ? value : 'system';
}

const state = {
  catalog: null,
  language: readPreference('bitfun-playbook-language')
    ?? (navigator.language.startsWith('zh') ? 'zh' : 'en'),
  query: new URLSearchParams(location.search).get('q') ?? '',
  kind: 'all',
  category: 'all',
  theme: storedTheme(),
};

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}

function text(zh, en) {
  return state.language === 'zh' ? zh : en;
}

function localized(value, field) {
  return value[`${field}${state.language === 'zh' ? 'Zh' : 'En'}`];
}

function alternate(value, field) {
  return value[`${field}${state.language === 'zh' ? 'En' : 'Zh'}`];
}

function normalize(value) {
  return String(value ?? '').toLocaleLowerCase().trim();
}

function kindLabel(kind) {
  return kind === 'feature' ? text('功能', 'Feature') : text('设置', 'Setting');
}

function themeLabel(theme) {
  if (theme === 'light') return text('浅色', 'Light');
  if (theme === 'dark') return text('深色', 'Dark');
  return text('跟随系统', 'System');
}

function themeIcon(theme) {
  if (theme === 'light') return '☀';
  if (theme === 'dark') return '☾';
  return '◐';
}

function resolvedTheme(theme = state.theme) {
  if (theme !== 'system') return theme;
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function syncThemeColor() {
  const surface = getComputedStyle(document.documentElement)
    .getPropertyValue('--bf-color-surface-canvas')
    .trim();
  if (surface) {
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', surface);
  }
}

function syncThemeControls() {
  document.querySelectorAll('[data-theme-choice]').forEach((button) => {
    const active = button.dataset.themeChoice === state.theme;
    button.setAttribute('aria-pressed', String(active));
    button.classList.toggle('active', active);
  });
}

function applyTheme(theme, persist = true) {
  state.theme = THEME_CHOICES.includes(theme) ? theme : 'system';
  document.documentElement.dataset.theme = state.theme;
  document.documentElement.dataset.colorScheme = resolvedTheme();
  syncThemeColor();
  if (persist) writePreference(THEME_STORAGE_KEY, state.theme);
  syncThemeControls();
}

function themeControl() {
  return `
    <div class="theme-control" role="group" aria-label="${text('颜色主题', 'Color theme')}">
      ${THEME_CHOICES.map((theme) => `
        <button type="button" data-theme-choice="${theme}"
          class="${state.theme === theme ? 'active' : ''}"
          aria-pressed="${state.theme === theme}"
          aria-label="${themeLabel(theme)}" title="${themeLabel(theme)}">
          <span aria-hidden="true">${themeIcon(theme)}</span><span>${themeLabel(theme)}</span>
        </button>`).join('')}
    </div>`;
}

function header() {
  return `
    <header class="site-header">
      <a class="brand" href="/" aria-label="BitFun Playbook home">
        <span class="brand-mark">B</span>
        <span><strong>BitFun</strong><small>PLAYBOOK</small></span>
      </a>
      <nav class="header-actions" aria-label="${text('全局操作', 'Global actions')}">
        <button class="sidebar-toggle" id="sidebar-toggle" type="button" aria-controls="docs-sidebar" aria-expanded="false">
          <span aria-hidden="true">☰</span>${text('目录', 'Contents')}
        </button>
        ${themeControl()}
        <a class="header-link" href="https://github.com/GCWing/BitFun">GitHub</a>
        <button class="language-toggle" id="language-toggle" type="button" aria-label="${text('Switch to English', '切换到中文')}">
          ${state.language === 'zh' ? 'EN' : '中文'}
        </button>
      </nav>
    </header>`;
}

function footer() {
  return `
    <footer class="site-footer">
      <p>${text('说明书、应用搜索和 Agent 控制共享同一份功能与设置目录。', 'The manual, in-app search, and agent control share one features-and-settings catalog.')}</p>
      <code>${state.catalog.digest.slice(0, 12)}</code>
    </footer>`;
}

function categoryTitle(category) {
  return localized(category, 'title');
}

function sidebarSearchText(capability) {
  return escapeHtml(normalize([
    capability.titleZh,
    capability.titleEn,
    ...capability.items.flatMap((item) => [item.titleZh, item.titleEn]),
    ...capability.searchTerms,
  ].join(' ')));
}

function sidebarSection(kind) {
  const capabilities = state.catalog.capabilities.filter((capability) => capability.kind === kind);
  const categories = Object.entries(state.catalog.categories)
    .map(([id, category]) => ({
      id,
      category,
      capabilities: capabilities.filter((capability) => capability.categoryId === id),
    }))
    .filter((group) => group.capabilities.length);
  return `
    <section class="sidebar-section" data-sidebar-section>
      <div class="sidebar-kind-heading">
        <h2>${kind === 'feature' ? text('功能', 'Features') : text('设置', 'Settings')}</h2>
        <span>${capabilities.length}</span>
      </div>
      ${categories.map(({ category, capabilities: entries }) => `
        <div class="sidebar-group" data-sidebar-group>
          <h3>${escapeHtml(categoryTitle(category))}</h3>
          <div class="sidebar-links">
            ${entries.map((capability) => `
              <a href="/capabilities/${encodeURIComponent(capability.id)}/"
                data-sidebar-item data-sidebar-search="${sidebarSearchText(capability)}"
                ${pageData.capabilityId === capability.id ? 'class="active" aria-current="page"' : ''}>
                <span>${escapeHtml(localized(capability, 'title'))}</span>
              </a>`).join('')}
          </div>
        </div>`).join('')}
    </section>`;
}

function sidebar() {
  return `
    <aside class="docs-sidebar" id="docs-sidebar" aria-label="${text('说明书目录', 'Playbook contents')}">
      <div class="sidebar-mobile-head">
        <strong>${text('说明书目录', 'Playbook contents')}</strong>
        <button id="sidebar-close" type="button" aria-label="${text('关闭目录', 'Close contents')}">×</button>
      </div>
      <div class="sidebar-search">
        <span aria-hidden="true">⌕</span>
        <input id="sidebar-search" type="search" autocomplete="off" spellcheck="false"
          placeholder="${text('筛选目录…', 'Filter contents…')}" aria-label="${text('筛选说明书目录', 'Filter playbook contents')}" />
      </div>
      <nav data-sidebar-scroll>
        <a class="sidebar-home ${pageData.kind === 'index' ? 'active' : ''}" href="/" ${pageData.kind === 'index' ? 'aria-current="page"' : ''}>
          <span aria-hidden="true">⌂</span>${text('说明书首页', 'Playbook home')}
        </a>
        ${sidebarSection('feature')}
        ${sidebarSection('setting')}
        <p class="sidebar-empty" id="sidebar-empty" hidden>${text('目录中没有匹配项', 'No matching entry')}</p>
      </nav>
    </aside>`;
}

function pageFrame(content, className = '') {
  return `
    <div class="page-shell ${className}">
      ${header()}
      <div class="docs-shell">
        ${sidebar()}
        <div class="docs-content">${content}</div>
      </div>
      <button class="sidebar-backdrop" id="sidebar-backdrop" type="button" aria-label="${text('关闭目录', 'Close contents')}" tabindex="-1"></button>
    </div>`;
}

function capabilityCard(capability) {
  const category = state.catalog.categories[capability.categoryId];
  const matches = matchingItems(capability, state.query);
  const focus = matches[0]?.id;
  const href = `/capabilities/${encodeURIComponent(capability.id)}/${focus ? `?focus=${encodeURIComponent(focus)}#item-${encodeURIComponent(focus)}` : ''}`;
  return `
    <a class="capability-card kind-${escapeHtml(capability.kind)}" href="${href}">
      <span class="card-topline">
        <span class="kind-badge">${kindLabel(capability.kind)}</span>
        <span class="category-name">${escapeHtml(categoryTitle(category))}</span>
      </span>
      <strong>${escapeHtml(localized(capability, 'title'))}</strong>
      <span class="secondary-title">${escapeHtml(alternate(capability, 'title'))}</span>
      <p>${escapeHtml(localized(capability, 'summary'))}</p>
      ${focus ? `<span class="matched-item"><i>${text('匹配', 'Match')}</i>${escapeHtml(localized(matches[0], 'title'))}</span>` : ''}
      <span class="card-link">${text('查看说明', 'Read guide')} <span aria-hidden="true">↗</span></span>
    </a>`;
}

function filteredCapabilities() {
  return state.catalog.capabilities
    .map((capability) => ({ capability, rank: scoreCapability(capability, state.query) }))
    .filter(({ capability, rank }) => rank > 0
      && (state.kind === 'all' || capability.kind === state.kind)
      && (state.category === 'all' || capability.categoryId === state.category))
    .sort((left, right) => right.rank - left.rank
      || left.capability.kind.localeCompare(right.capability.kind)
      || left.capability.id.localeCompare(right.capability.id))
    .map(({ capability }) => capability);
}

function capabilityResultsMarkup(capabilities) {
  return capabilities.length ? capabilities.map(capabilityCard).join('') : `
    <div class="empty-state">
      <span>⌕</span>
      <h3>${text('没有找到相关功能或设置', 'No matching feature or setting')}</h3>
      <p>${text('试试“终端”“远程控制”“主题”或“MCP”。', 'Try “terminal,” “remote control,” “theme,” or “MCP.”')}</p>
    </div>`;
}

function updateIndexResults() {
  const capabilities = filteredCapabilities();
  const count = document.querySelector('#result-count');
  const grid = document.querySelector('#capability-grid');
  if (count) count.textContent = text(`找到 ${capabilities.length} 项`, `${capabilities.length} results`);
  if (grid) grid.innerHTML = capabilityResultsMarkup(capabilities);
}

function renderIndex() {
  const capabilities = filteredCapabilities();
  const categoryCounts = Object.fromEntries(
    Object.keys(state.catalog.categories).map((id) => [
      id,
      state.catalog.capabilities.filter((capability) => capability.categoryId === id).length,
    ]),
  );
  root.innerHTML = pageFrame(`
      <main>
        <section class="hero">
          <p class="eyebrow">THE OPERATING MANUAL FOR BITFUN</p>
          <h1>${text(
            '<span>会用 BitFun，</span><em>也会让它适应你。</em>',
            '<span>Use BitFun—and</span><em>make it yours.</em>',
          )}</h1>
          <p class="hero-copy">${text(
            '一本真正按“功能 + 设置”组织的使用说明书。搜中文或 English，找到后直接照着用，也可以把同一句话交给 Agent。',
            'A practical manual organized around features and settings. Search in English or 中文, follow the guide, or say the same thing to an agent.',
          )}</p>
          <div class="hero-actions">
            <form class="search-box" id="search-form" role="search">
              <span class="search-glyph" aria-hidden="true">⌕</span>
              <input id="capability-search" type="search" value="${escapeHtml(state.query)}"
                placeholder="${text('搜索功能或设置，例如：终端、深色模式、MCP…', 'Search features or settings: terminal, dark mode, MCP…')}"
                autocomplete="off" spellcheck="false" autofocus />
              <kbd>⌘ K</kbd>
            </form>
            <div class="hero-stat" aria-label="${text('说明书统计', 'Playbook statistics')}">
              <strong>${state.catalog.counts.features}</strong><span>${text('个功能', 'features')}</span>
              <i></i>
              <strong>${state.catalog.counts.settings}</strong><span>${text('个设置页', 'settings pages')}</span>
              <i></i>
              <strong>${state.catalog.counts.documentedItems}</strong><span>${text('项完整说明', 'documented items')}</span>
            </div>
          </div>
        </section>

        <section class="catalog-section" aria-labelledby="catalog-heading">
          <div class="catalog-toolbar">
            <div>
              <p class="section-label">PLAYBOOK INDEX</p>
              <h2 id="catalog-heading">${text('功能与设置', 'Features & settings')}</h2>
            </div>
            <p id="result-count" aria-live="polite">${text(`找到 ${capabilities.length} 项`, `${capabilities.length} results`)}</p>
          </div>

          <div class="kind-switch" role="tablist" aria-label="${text('条目类型', 'Entry type')}">
            <button class="${state.kind === 'all' ? 'active' : ''}" data-kind="all" type="button">${text('全部', 'All')} <span>${state.catalog.counts.userFacing}</span></button>
            <button class="${state.kind === 'feature' ? 'active' : ''}" data-kind="feature" type="button">${text('功能', 'Features')} <span>${state.catalog.counts.features}</span></button>
            <button class="${state.kind === 'setting' ? 'active' : ''}" data-kind="setting" type="button">${text('设置', 'Settings')} <span>${state.catalog.counts.settings}</span></button>
          </div>

          <div class="category-rail" role="tablist" aria-label="${text('内容分类', 'Content categories')}">
            <button class="category-pill ${state.category === 'all' ? 'active' : ''}" data-category="all" type="button">
              ${text('所有领域', 'All areas')}
            </button>
            ${Object.entries(state.catalog.categories).map(([id, category]) => `
              <button class="category-pill ${state.category === id ? 'active' : ''}" data-category="${escapeHtml(id)}" type="button">
                ${escapeHtml(categoryTitle(category))} <span>${categoryCounts[id]}</span>
              </button>`).join('')}
          </div>
          <div class="capability-grid" id="capability-grid">
            ${capabilityResultsMarkup(capabilities)}
          </div>
        </section>
      </main>
      ${footer()}
  `, 'index-page');
  bindCommonEvents();
  document.querySelector('#search-form')?.addEventListener('submit', (event) => event.preventDefault());
  const searchInput = document.querySelector('#capability-search');
  let composing = false;
  const applySearchInput = () => {
    state.query = searchInput.value;
    const url = new URL(location.href);
    if (state.query) url.searchParams.set('q', state.query);
    else url.searchParams.delete('q');
    history.replaceState(null, '', url);
    updateIndexResults();
  };
  searchInput?.addEventListener('compositionstart', () => {
    composing = true;
  });
  searchInput?.addEventListener('compositionend', () => {
    composing = false;
    applySearchInput();
  });
  searchInput?.addEventListener('input', (event) => {
    if (!composing && !event.isComposing) applySearchInput();
  });
  document.querySelectorAll('[data-kind]').forEach((button) => {
    button.addEventListener('click', () => {
      state.kind = button.dataset.kind;
      renderIndex();
    });
  });
  document.querySelectorAll('[data-category]').forEach((button) => {
    button.addEventListener('click', () => {
      state.category = button.dataset.category;
      renderIndex();
    });
  });
}

function destinationLabel(capability) {
  if (capability.destination.kind === 'settings') {
    return text('BitFun 设置中的对应页面', 'The matching page in BitFun Settings');
  }
  return text('BitFun 中的对应功能入口', 'The matching feature entry in BitFun');
}

function schemaLabel(schema) {
  if (schema.enum) return schema.enum.map(String).join(' / ');
  if (schema.type === 'boolean') return text('开 / 关', 'On / Off');
  if (schema.minimum !== undefined || schema.maximum !== undefined) {
    return `${schema.minimum ?? '…'}–${schema.maximum ?? '…'}`;
  }
  return schema.type;
}

function operationCards(capability) {
  if (capability.agentControl) {
    const workflow = localized(capability.agentControl, 'workflow');
    return `<ol class="step-list agent-control-list">${workflow.map((step, index) => `
      <li><span>${index + 1}</span><p><strong>${escapeHtml(capability.agentControl.tool)}</strong> · ${escapeHtml(step)}</p></li>`).join('')}</ol>`;
  }
  if (!capability.operations.length) {
    return `<div class="manual-empty">${text(
      '这个功能通过“打开”进入界面，具体操作在页面里完成。',
      'Use Open to enter this feature, then complete its actions in the interface.',
    )}</div>`;
  }
  return `<div class="operation-list">${capability.operations.map((operation) => `
    <article>
      <h3>${escapeHtml(localized(operation, 'title'))}</h3>
      <p>${escapeHtml(localized(operation, 'description'))}</p>
    </article>`).join('')}</div>`;
}

function optionRows(capability) {
  if (!capability.options.length) {
    return `<div class="manual-empty">${text(
      '这个页面的配置需要在 BitFun 界面中确认和完成。Agent 可以先替你打开到这里。',
      'These controls require confirmation in the BitFun interface. An agent can still open this page for you.',
    )}</div>`;
  }
  return `<div class="option-table">${capability.options.map((option) => `
    <article>
      <div><h3>${escapeHtml(localized(option, 'title'))}</h3></div>
      <p>${escapeHtml(localized(option, 'description'))}</p>
      <span>${escapeHtml(schemaLabel(option.valueSchema))}</span>
    </article>`).join('')}</div>`;
}

function itemControlLabel(item) {
  if (item.control.kind === 'direct') return text('Agent 直接控制', 'Direct Agent control');
  if (item.control.kind === 'delegate') return text('专用工具控制', 'Delegated tool');
  if (item.control.kind === 'open') return text('需界面交互', 'Interaction required');
  return text('不支持', 'Unsupported');
}

function itemControlTools(item, capability) {
  if (item.control.kind !== 'delegate') return '';
  const tools = item.control.tools ?? (capability.agentControl?.tool ? [capability.agentControl.tool] : []);
  return tools.length ? ` · ${tools.join(' / ')}` : '';
}

function renderCapability(capability) {
  const category = state.catalog.categories[capability.categoryId];
  const steps = localized(capability, 'steps');
  const examples = localized(capability, 'agentExamples');
  root.innerHTML = pageFrame(`
      <main>
        <nav class="breadcrumb"><a href="/">${text('功能与设置', 'Features & settings')}</a><span>/</span><span>${escapeHtml(categoryTitle(category))}</span></nav>
        <article class="detail-layout manual-layout">
          <section class="detail-main">
            <div class="detail-category kind-${escapeHtml(capability.kind)}"><span class="kind-badge">${kindLabel(capability.kind)}</span>${escapeHtml(categoryTitle(category))}</div>
            <h1>${escapeHtml(localized(capability, 'title'))}</h1>
            <p class="detail-alt-title">${escapeHtml(alternate(capability, 'title'))}</p>
            <p class="detail-summary">${escapeHtml(localized(capability, 'summary'))}</p>
            <div class="entry-callout"><span>${text('在哪里找到', 'Where to find it')}</span><strong>${destinationLabel(capability)}</strong></div>

            <section class="content-section manual-section">
              <p class="section-label">EVERYTHING INCLUDED · ${capability.items.length}</p>
              <h2>${text('完整功能清单', 'Everything included')}</h2>
              <ul class="highlight-list inventory-list">${capability.items.map((item) => `<li id="item-${escapeHtml(item.id)}" data-inventory-item="${escapeHtml(item.id)}" data-control-kind="${escapeHtml(item.control.kind)}"><span>✓</span><div><em class="control-badge control-${escapeHtml(item.control.kind)}">${itemControlLabel(item)}${escapeHtml(itemControlTools(item, capability))}</em>${escapeHtml(localized(item, 'title'))}<small>${escapeHtml(alternate(item, 'title'))}</small></div></li>`).join('')}</ul>
            </section>

            <section class="content-section manual-section">
              <p class="section-label">HOW TO</p>
              <h2>${text('怎么用', 'How to use it')}</h2>
              <ol class="step-list">${steps.map((step, index) => `<li><span>${index + 1}</span><p>${escapeHtml(step)}</p></li>`).join('')}</ol>
            </section>

            ${(capability.kind === 'feature' || capability.operations.length || capability.agentControl) ? `
              <section class="content-section manual-section">
                <p class="section-label">DIRECT ACTIONS</p>
                <h2>${text('Agent 可以直接执行', 'What an agent can execute')}</h2>
                ${operationCards(capability)}
              </section>` : ''}

            ${capability.kind === 'setting' ? `
              <section class="content-section manual-section">
                <p class="section-label">OPTIONS</p>
                <h2>${text('可配置选项', 'Configurable options')}</h2>
                ${optionRows(capability)}
              </section>` : ''}

            <section class="content-section manual-section">
              <p class="section-label">ASK AN AGENT</p>
              <h2>${text('可以直接对 Agent 说', 'Try saying this to an agent')}</h2>
              <div class="prompt-list">${examples.map((example) => `
                <button type="button" data-copy-prompt="${escapeHtml(example)}"><span>“${escapeHtml(example)}”</span><small>${text('复制', 'Copy')}</small></button>`).join('')}</div>
            </section>
          </section>

          <aside class="detail-aside manual-aside">
            <p class="section-label">AGENT READY</p>
            <h2>${text('不必记住入口', 'No menus to memorize')}</h2>
            <p>${text(
              '和 Agent 说出你想做的事。目录会明确告诉它哪些可直接控制、哪些交给专用工具、哪些仍需你在界面确认。',
              'Tell an agent what you want. The catalog explicitly says what it can control directly, delegate to a specialist tool, or only open for your confirmation.',
            )}</p>
            <ol>
              <li>${text('理解你的意图', 'Understand your intent')}</li>
              <li>${text('搜索或读取对应说明', 'Search or read the matching guide')}</li>
              <li>${text('打开、执行或配置', 'Open, execute, or configure')}</li>
            </ol>
            <button type="button" class="copy-button" data-copy-first>${text('复制一句示例', 'Copy an example')}</button>
            <a class="back-link" href="/?q=${encodeURIComponent(capability.titleZh)}">${text('搜索相关条目', 'Find related entries')} ↗</a>
          </aside>
        </article>
      </main>
      ${footer()}
  `, 'detail-page');
  bindCommonEvents();
  const focusedItemId = new URLSearchParams(location.search).get('focus');
  const focusedItem = focusedItemId
    ? document.querySelector(`[data-inventory-item="${CSS.escape(focusedItemId)}"]`)
    : null;
  if (focusedItem) {
    focusedItem.classList.add('focused');
    requestAnimationFrame(() => focusedItem.scrollIntoView({ block: 'center', behavior: 'smooth' }));
  }
  const copyPrompt = async (button, prompt) => {
    await navigator.clipboard.writeText(prompt);
    const label = button.querySelector('small');
    if (label) label.textContent = text('已复制', 'Copied');
    else button.textContent = text('已复制', 'Copied');
  };
  document.querySelectorAll('[data-copy-prompt]').forEach((button) => {
    button.addEventListener('click', () => void copyPrompt(button, button.dataset.copyPrompt));
  });
  document.querySelector('[data-copy-first]')?.addEventListener('click', (event) => {
    void copyPrompt(event.currentTarget, examples[0]);
  });
}

function bindCommonEvents() {
  const sidebarNav = document.querySelector('[data-sidebar-scroll]');
  const saveSidebarScroll = () => {
    if (sidebarNav) writeSessionPreference(SIDEBAR_SCROLL_STORAGE_KEY, String(sidebarNav.scrollTop));
  };
  sidebarNav?.addEventListener('scroll', saveSidebarScroll, { passive: true });
  document.querySelectorAll('.sidebar-home, [data-sidebar-item]').forEach((link) => {
    link.addEventListener('click', saveSidebarScroll);
  });
  const storedSidebarScroll = readSessionPreference(SIDEBAR_SCROLL_STORAGE_KEY);
  requestAnimationFrame(() => {
    if (!sidebarNav) return;
    const savedScrollTop = Number(storedSidebarScroll);
    if (storedSidebarScroll !== null && Number.isFinite(savedScrollTop) && savedScrollTop >= 0) {
      sidebarNav.scrollTop = savedScrollTop;
    }
    const activeItem = sidebarNav.querySelector('[aria-current="page"]');
    if (!activeItem) return;
    const navRect = sidebarNav.getBoundingClientRect();
    const itemRect = activeItem.getBoundingClientRect();
    if (itemRect.top < navRect.top || itemRect.bottom > navRect.bottom) {
      sidebarNav.scrollTop += itemRect.top - navRect.top - (navRect.height - itemRect.height) / 2;
    }
  });
  const setSidebarOpen = (open) => {
    document.body.classList.toggle('sidebar-open', open);
    document.querySelector('#sidebar-toggle')?.setAttribute('aria-expanded', String(open));
    if (open) document.querySelector('#sidebar-search')?.focus();
  };
  document.querySelector('#sidebar-toggle')?.addEventListener('click', () => setSidebarOpen(true));
  document.querySelector('#sidebar-close')?.addEventListener('click', () => setSidebarOpen(false));
  document.querySelector('#sidebar-backdrop')?.addEventListener('click', () => setSidebarOpen(false));
  document.querySelector('#sidebar-search')?.addEventListener('input', (event) => {
    const needle = normalize(event.target.value);
    const tokens = needle.split(/\s+/u).filter(Boolean);
    document.querySelectorAll('[data-sidebar-item]').forEach((item) => {
      item.hidden = tokens.length > 0 && !tokens.every((token) => item.dataset.sidebarSearch.includes(token));
    });
    document.querySelectorAll('[data-sidebar-group]').forEach((group) => {
      group.hidden = !group.querySelector('[data-sidebar-item]:not([hidden])');
    });
    document.querySelectorAll('[data-sidebar-section]').forEach((section) => {
      section.hidden = !section.querySelector('[data-sidebar-item]:not([hidden])');
    });
    const hasResults = !!document.querySelector('[data-sidebar-item]:not([hidden])');
    const empty = document.querySelector('#sidebar-empty');
    if (empty) empty.hidden = hasResults;
  });
  document.querySelector('#language-toggle')?.addEventListener('click', () => {
    saveSidebarScroll();
    document.body.classList.remove('sidebar-open');
    state.language = state.language === 'zh' ? 'en' : 'zh';
    writePreference('bitfun-playbook-language', state.language);
    render();
  });
  document.querySelectorAll('[data-theme-choice]').forEach((button) => {
    button.addEventListener('click', () => applyTheme(button.dataset.themeChoice));
  });
}

function render() {
  document.documentElement.lang = state.language === 'zh' ? 'zh-CN' : 'en';
  if (pageData.kind === 'capability') {
    const capability = state.catalog.capabilities.find(({ id }) => id === pageData.capabilityId);
    if (capability) renderCapability(capability);
    else renderIndex();
  } else {
    renderIndex();
  }
}

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && document.body.classList.contains('sidebar-open')) {
    document.body.classList.remove('sidebar-open');
    document.querySelector('#sidebar-toggle')?.setAttribute('aria-expanded', 'false');
    document.querySelector('#sidebar-toggle')?.focus();
    return;
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    if (pageData.kind === 'capability') location.href = '/';
    else document.querySelector('#capability-search')?.focus();
  }
});

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (state.theme === 'system') applyTheme('system', false);
});

applyTheme(state.theme, false);

try {
  const response = await fetch('/data/capabilities.json');
  if (!response.ok) throw new Error(`Catalog request failed: ${response.status}`);
  state.catalog = await response.json();
  render();
} catch (error) {
  root.innerHTML = `<div class="fatal-error"><h1>BitFun Playbook</h1><p>${escapeHtml(error.message)}</p></div>`;
}
