import React, {
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { i18n as I18nApi } from 'i18next';
import { useTranslation } from 'react-i18next';
import {
  Icon,
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelHeader,
  NavigationPanelItem,
  NavigationPanelSection,
  SearchField,
} from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { getInteractionMotion } from '@/shared/utils/motionPreference';
import {
  SETTINGS_CATEGORIES,
  SETTINGS_PAGE_MANIFESTS,
  preloadSettingsPage,
  type SettingsSearchPhrase,
} from './settingsRegistry';
import { useSettingsStore } from './settingsStore';
import type { SettingsDestination, SettingsPageId } from './settingsTypes';
import './SettingsNav.scss';

const SEARCH_DEBOUNCE_MS = 150;
type SettingsT = (key: string, options?: Record<string, unknown>) => unknown;

export interface SettingsSearchRow {
  destination: SettingsDestination;
  categoryLabel: string;
  pageLabel: string;
  viewLabel?: string;
  description: string;
  haystack: string;
}

function translateString(t: SettingsT, key: string, fallback: string): string {
  const value = t(key, { defaultValue: fallback });
  return typeof value === 'string' ? value : fallback;
}

function resolvePhrases(i18n: I18nApi, phrases: readonly SettingsSearchPhrase[]): string {
  const parts: string[] = [];
  for (const { namespace, key } of phrases) {
    const value = i18n.getFixedT(i18n.language, namespace)(key, { defaultValue: '' });
    if (typeof value === 'string' && value.trim() && value !== key) parts.push(value);
  }
  return parts.join(' ');
}

function buildSettingsSearchIndex(t: SettingsT, i18n: I18nApi): SettingsSearchRow[] {
  const categoryLabels = new Map(SETTINGS_CATEGORIES.map((category) => [
    category.id,
    translateString(t, category.labelKey, category.id),
  ]));

  return SETTINGS_PAGE_MANIFESTS.flatMap((page) => {
    const categoryLabel = categoryLabels.get(page.categoryId) ?? page.categoryId;
    const pageLabel = translateString(t, page.labelKey, page.id);
    const description = translateString(t, page.descriptionKey, '');
    const pageContent = resolvePhrases(i18n, page.searchPhrases);
    const base = [categoryLabel, pageLabel, description, page.id, ...page.keywords, pageContent];

    if (!page.views?.length) {
      return [{
        destination: { pageId: page.id },
        categoryLabel,
        pageLabel,
        description,
        haystack: base.join(' ').toLowerCase(),
      }];
    }

    return page.views.map((view) => {
      const viewLabel = translateString(t, view.labelKey, view.id);
      return {
        destination: { pageId: page.id, viewId: view.id },
        categoryLabel,
        pageLabel,
        viewLabel,
        description,
        haystack: [
          ...base,
          viewLabel,
          view.id,
          ...view.keywords,
          resolvePhrases(i18n, view.searchPhrases),
        ].join(' ').toLowerCase(),
      };
    });
  });
}

function highlightFirstMatch(text: string, query: string): React.ReactNode {
  const needle = query.trim();
  if (!needle) return text;
  const index = text.toLowerCase().indexOf(needle.toLowerCase());
  if (index < 0) return text;
  return (
    <>
      {text.slice(0, index)}
      <mark
        className="bitfun-settings-nav__search-highlight"
        data-bf-component="settings-nav"
        data-bf-part="highlight"
      >
        {text.slice(index, index + needle.length)}
      </mark>
      {text.slice(index + needle.length)}
    </>
  );
}

const SettingsNav: React.FC = () => {
  const { t, i18n } = useTranslation('settings');
  const { t: tComponents } = useI18n('components');
  const activePageId = useSettingsStore((state) => state.activePageId);
  const activeViewId = useSettingsStore((state) => state.activeViewId);
  const openDestination = useSettingsStore((state) => state.openDestination);
  const searchQuery = useSettingsStore((state) => state.searchQuery);
  const setSearchQuery = useSettingsStore((state) => state.setSearchQuery);
  const [draftQuery, setDraftQuery] = useState('');
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);
  const activationRequestRef = useRef(0);

  useEffect(() => {
    const timer = window.setTimeout(() => setSearchQuery(draftQuery), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [draftQuery, setSearchQuery]);

  const searchIndex = useMemo(() => buildSettingsSearchIndex(t, i18n), [i18n, t]);
  const results = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    return query ? searchIndex.filter((row) => row.haystack.includes(query)) : [];
  }, [searchIndex, searchQuery]);
  const isSearchMode = draftQuery.trim().length > 0;

  useEffect(() => {
    setHighlightedIndex((current) => {
      if (!results.length) return -1;
      return current >= results.length ? results.length - 1 : current;
    });
  }, [results.length]);

  const clearSearch = useCallback(() => {
    setDraftQuery('');
    setSearchQuery('');
    setHighlightedIndex(-1);
  }, [setSearchQuery]);

  const activate = useCallback((destination: SettingsDestination, clear = false) => {
    const requestId = ++activationRequestRef.current;
    const motion = getInteractionMotion();
    const commit = () => {
      if (requestId !== activationRequestRef.current) return;
      startTransition(() => {
        openDestination(destination, motion);
        if (clear) clearSearch();
      });
    };
    void preloadSettingsPage(destination.pageId).then(commit, commit);
  }, [clearSearch, openDestination]);

  const preload = useCallback((pageId: SettingsPageId) => {
    void preloadSettingsPage(pageId).catch(() => undefined);
  }, []);

  const handleSearchKeyDown = useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      clearSearch();
    } else if (event.key === 'ArrowDown' && results.length > 0) {
      event.preventDefault();
      setHighlightedIndex(0);
      queueMicrotask(() => resultsRef.current?.focus());
    } else if (event.key === 'Enter' && results.length === 1) {
      event.preventDefault();
      activate(results[0].destination, true);
    }
  }, [activate, clearSearch, results]);

  const handleResultsKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!results.length) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      clearSearch();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      setHighlightedIndex((index) => Math.min(index + 1, results.length - 1));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setHighlightedIndex((index) => {
        if (index <= 0) {
          searchInputRef.current?.focus();
          return -1;
        }
        return index - 1;
      });
    } else if (event.key === 'Enter' && highlightedIndex >= 0) {
      event.preventDefault();
      activate(results[highlightedIndex].destination, true);
    }
  }, [activate, clearSearch, highlightedIndex, results]);

  return (
    <NavigationPanel
      className="bitfun-settings-nav"
      data-testid="settings-nav"
      data-bf-component="settings-nav"
      data-bf-part="root"
    >
      <NavigationPanelHeader className="bitfun-settings-nav__panel-header">
        <>
          <div className="bitfun-settings-nav__header" data-bf-component="settings-nav" data-bf-part="header">
            <span className="bitfun-settings-nav__title">{t('shared:features.settings')}</span>
          </div>
          <div className="bitfun-settings-nav__search" data-bf-component="settings-nav" data-bf-part="search">
            <SearchField
              ref={searchInputRef}
              className="bitfun-settings-nav__search-field"
              size="sm"
              value={draftQuery}
              onValueChange={setDraftQuery}
              onClear={draftQuery ? () => {
                clearSearch();
                searchInputRef.current?.focus();
              } : undefined}
              clearLabel={draftQuery ? tComponents('search.clear') : undefined}
              onKeyDown={handleSearchKeyDown}
              leadingIcon={<Icon name="search" size="sm" />}
              placeholder={t('navigation.search.placeholder')}
              aria-label={t('navigation.search.placeholder')}
              aria-controls="settings-nav-results"
              aria-expanded={isSearchMode}
            />
          </div>
        </>
      </NavigationPanelHeader>
      <NavigationPanelBody>
        <NavigationPanelContent className="bitfun-settings-nav__content">
          {isSearchMode ? (
        results.length ? (
          <div
            ref={resultsRef}
            id="settings-nav-results"
            className="bitfun-settings-nav__search-results"
            data-bf-component="settings-nav"
            data-bf-part="searchResults"
            role="listbox"
            tabIndex={results.length ? 0 : undefined}
            onKeyDown={handleResultsKeyDown}
            aria-activedescendant={highlightedIndex >= 0
              ? `settings-nav-result-${highlightedIndex}`
              : undefined}
          >
            {results.map((row, index) => {
              const active = activePageId === row.destination.pageId
                && (!row.destination.viewId || row.destination.viewId === activeViewId);
              const selected = index === highlightedIndex;
              const path = [row.categoryLabel, row.pageLabel, row.viewLabel].filter(Boolean).join(' › ');
              return (
                <NavigationPanelItem
                  key={`${row.destination.pageId}:${row.destination.viewId ?? ''}`}
                  id={`settings-nav-result-${index}`}
                  role="option"
                  aria-selected={active}
                  selected={active}
                  data-bf-component="settings-nav"
                  data-bf-part="searchResult"
                  data-bf-state={[active && 'active', selected && 'selected'].filter(Boolean).join(' ') || undefined}
                  className={[
                    'bitfun-settings-nav__search-result-item',
                    selected && 'is-highlighted',
                    active && 'is-active',
                  ].filter(Boolean).join(' ')}
                  onClick={() => activate(row.destination, true)}
                  onMouseEnter={() => {
                    setHighlightedIndex(index);
                    preload(row.destination.pageId);
                  }}
                  onFocus={() => preload(row.destination.pageId)}
                >
                  <span className="bitfun-settings-nav__search-result-copy">
                    <span className="bitfun-settings-nav__search-result-line">
                      {highlightFirstMatch(path, searchQuery)}
                    </span>
                    <span className="bitfun-settings-nav__search-result-desc">
                      {highlightFirstMatch(row.description, searchQuery)}
                    </span>
                  </span>
                </NavigationPanelItem>
              );
            })}
          </div>
        ) : (
          <div className="bitfun-settings-nav__search-empty" role="status" data-bf-component="settings-nav" data-bf-part="searchEmpty">
            {t('navigation.search.empty')}
          </div>
        )
      ) : SETTINGS_CATEGORIES.map((category) => (
        <NavigationPanelSection
          key={category.id}
          className="bitfun-settings-nav__category"
          data-bf-component="settings-nav"
          data-bf-part="category"
          title={(
            <span
              className="bitfun-settings-nav__category-label"
              data-bf-component="settings-nav"
              data-bf-part="categoryHeader"
            >
              {t(category.labelKey)}
            </span>
          )}
        >
          <div className="bitfun-settings-nav__items" data-bf-component="settings-nav" data-bf-part="items">
          {category.pages.map((page) => (
            <NavigationPanelItem
              key={page.id}
              data-testid="settings-nav-page"
              data-settings-page={page.id}
              data-bf-component="settings-nav"
              data-bf-part="item"
              data-bf-state={activePageId === page.id ? 'active' : undefined}
              className="bitfun-settings-nav__item"
              selected={activePageId === page.id}
              onClick={() => activate({ pageId: page.id })}
              onPointerEnter={() => preload(page.id)}
              onFocus={() => preload(page.id)}
            >
              <span className="bitfun-settings-nav__item-label">{t(page.labelKey)}</span>
            </NavigationPanelItem>
          ))}
          </div>
        </NavigationPanelSection>
          ))}
        </NavigationPanelContent>
      </NavigationPanelBody>
    </NavigationPanel>
  );
};

export default SettingsNav;
