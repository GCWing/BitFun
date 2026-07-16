const OPEN_SELECTS = new Set();

function closeSelect(wrap) {
  const menu = wrap.querySelector('.ppt-flat-select__menu');
  const trigger = wrap.querySelector('.ppt-flat-select__trigger');
  if (!menu || !trigger) return;
  menu.hidden = true;
  menu.style.top = '';
  menu.style.bottom = '';
  menu.style.left = '';
  menu.style.width = '';
  trigger.setAttribute('aria-expanded', 'false');
  wrap.classList.remove('is-open');
  OPEN_SELECTS.delete(wrap);
}

function closeAllExcept(exceptWrap) {
  OPEN_SELECTS.forEach((wrap) => {
    if (wrap !== exceptWrap) closeSelect(wrap);
  });
}

function positionMenu(wrap) {
  const menu = wrap.querySelector('.ppt-flat-select__menu');
  const trigger = wrap.querySelector('.ppt-flat-select__trigger');
  if (!menu || !trigger) return;

  const rect = trigger.getBoundingClientRect();
  const menuHeight = Math.min(220, menu.scrollHeight || 220);
  const spaceBelow = window.innerHeight - rect.bottom;
  const openUpward = spaceBelow < menuHeight + 8 && rect.top > spaceBelow;

  // The menu is a fixed-position overlay (see .ppt-flat-select__menu), so it
  // never affects panel layout or scrollbars; only its placement is inline.
  menu.style.left = `${Math.max(8, rect.left)}px`;
  menu.style.width = `${rect.width}px`;

  if (openUpward) {
    menu.style.top = 'auto';
    menu.style.bottom = `${window.innerHeight - rect.top + 4}px`;
  } else {
    menu.style.top = `${rect.bottom + 4}px`;
    menu.style.bottom = 'auto';
  }
}

function openSelect(wrap) {
  const menu = wrap.querySelector('.ppt-flat-select__menu');
  const trigger = wrap.querySelector('.ppt-flat-select__trigger');
  if (!menu || !trigger) return;
  closeAllExcept(null);
  menu.hidden = false;
  positionMenu(wrap);
  trigger.setAttribute('aria-expanded', 'true');
  wrap.classList.add('is-open');
  OPEN_SELECTS.add(wrap);
  const selected = menu.querySelector('.ppt-flat-select__option.is-selected');
  selected?.scrollIntoView({ block: 'nearest' });
}

function moveMenuFocus(wrap, delta) {
  const menu = wrap.querySelector('.ppt-flat-select__menu');
  if (!menu || menu.hidden) return;
  const options = [...menu.querySelectorAll('.ppt-flat-select__option')];
  if (!options.length) return;
  const activeIndex = options.indexOf(document.activeElement);
  const currentIndex = activeIndex >= 0
    ? activeIndex
    : options.findIndex((node) => node.classList.contains('is-selected'));
  const nextIndex = Math.min(options.length - 1, Math.max(0, (currentIndex < 0 ? -delta : currentIndex) + delta));
  options[nextIndex]?.focus();
  options[nextIndex]?.scrollIntoView({ block: 'nearest' });
}

function syncFlatSelect(select) {
  const wrap = select.closest('.ppt-flat-select');
  if (!wrap) return;
  const label = wrap.querySelector('.ppt-flat-select__label');
  const menu = wrap.querySelector('.ppt-flat-select__menu');
  const selected = select.options[select.selectedIndex];
  if (label) label.textContent = selected?.textContent?.trim() || '';
  if (!menu) return;

  const existing = new Map([...menu.querySelectorAll('.ppt-flat-select__option')].map((node) => [node.dataset.value, node]));
  [...select.options].forEach((opt) => {
    let item = existing.get(opt.value);
    if (!item) {
      item = document.createElement('button');
      item.type = 'button';
      item.className = 'ppt-flat-select__option';
      item.setAttribute('role', 'option');
      item.dataset.value = opt.value;
      item.addEventListener('click', () => {
        select.value = opt.value;
        syncFlatSelect(select);
        closeSelect(wrap);
        select.dispatchEvent(new Event('change', { bubbles: true }));
      });
      menu.append(item);
    }
    item.textContent = opt.textContent;
    if (opt.title) item.title = opt.title;
    const isSelected = opt.value === select.value;
    item.classList.toggle('is-selected', isSelected);
    item.setAttribute('aria-selected', isSelected ? 'true' : 'false');
  });

  [...menu.querySelectorAll('.ppt-flat-select__option')].forEach((node) => {
    if (![...select.options].some((opt) => opt.value === node.dataset.value)) node.remove();
  });

  if (wrap.classList.contains('is-open')) positionMenu(wrap);
}

function isScrollInsideOpenMenu(target) {
  if (!(target instanceof Node)) return false;
  for (const wrap of OPEN_SELECTS) {
    const menu = wrap.querySelector('.ppt-flat-select__menu');
    if (menu && (menu === target || menu.contains(target))) return true;
  }
  return false;
}

function handleOutsideScroll(event) {
  if (isScrollInsideOpenMenu(event.target)) return;
  closeAllExcept(null);
}

function handleOutsideClick(event) {
  const target = event.target;
  if (target instanceof Node) {
    for (const wrap of OPEN_SELECTS) {
      if (wrap.contains(target)) return;
    }
  }
  closeAllExcept(null);
}

export function enhanceFlatSelect(select) {
  if (!select || select.dataset.flatSelect === 'true') return;
  select.dataset.flatSelect = 'true';
  select.classList.remove('ppt-flat-select');
  select.classList.add('ppt-flat-select__native');
  select.tabIndex = -1;
  select.setAttribute('aria-hidden', 'true');

  const wrap = document.createElement('div');
  wrap.className = 'ppt-flat-select';

  const trigger = document.createElement('button');
  trigger.type = 'button';
  trigger.className = 'ppt-flat-select__trigger';
  trigger.setAttribute('aria-haspopup', 'listbox');
  trigger.setAttribute('aria-expanded', 'false');

  const label = document.createElement('span');
  label.className = 'ppt-flat-select__label';
  trigger.append(label);

  const menu = document.createElement('div');
  menu.className = 'ppt-flat-select__menu';
  menu.hidden = true;
  menu.setAttribute('role', 'listbox');
  menu.addEventListener('wheel', (event) => event.stopPropagation(), { passive: true });
  menu.addEventListener('mousedown', (event) => event.stopPropagation());

  trigger.addEventListener('click', (event) => {
    event.stopPropagation();
    if (wrap.classList.contains('is-open')) {
      closeSelect(wrap);
      return;
    }
    openSelect(wrap);
  });

  wrap.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      if (!wrap.classList.contains('is-open')) openSelect(wrap);
      moveMenuFocus(wrap, event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'Escape' && wrap.classList.contains('is-open')) {
      event.stopPropagation();
      closeSelect(wrap);
      trigger.focus();
    }
  });

  const parent = select.parentNode;
  parent.insertBefore(wrap, select);
  wrap.append(trigger, menu, select);
  syncFlatSelect(select);
}

export function enhanceFlatSelects(root = document) {
  root.querySelectorAll('select#stylePresetSelect, select#themeInput, select#slideTargetInput').forEach((select) => {
    enhanceFlatSelect(select);
  });
}

export function refreshFlatSelect(select) {
  syncFlatSelect(select);
}

export function refreshFlatSelectLabels() {
  document.querySelectorAll('select[data-flat-select="true"]').forEach((select) => {
    syncFlatSelect(select);
  });
}

if (typeof window !== 'undefined' && typeof document !== 'undefined' && !window.__pptLiveFlatSelectBound) {
  window.__pptLiveFlatSelectBound = true;
  document.addEventListener('click', handleOutsideClick);
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeAllExcept(null);
  });
  window.addEventListener('resize', () => {
    OPEN_SELECTS.forEach((wrap) => positionMenu(wrap));
  });
  document.addEventListener('scroll', handleOutsideScroll, true);
}
