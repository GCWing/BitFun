import {
  GlobeSimple,
  Moon,
  Sun,
} from '@phosphor-icons/react';
import { useCallback, useEffect, useState } from 'react';
import { CatalogPage } from './CatalogPage';
import { DetailPage } from './DetailPage';
import { useI18n } from './i18n';
import { parseMarketRoute } from './router';
import { useTheme } from './theme';

function currentRoute() {
  return parseMarketRoute(window.location.pathname);
}

export default function App() {
  const { locale, setLocale, t } = useI18n();
  const { theme, toggleTheme } = useTheme();
  const [route, setRoute] = useState(currentRoute);
  const [catalogSearch, setCatalogSearch] = useState(
    currentRoute().kind === 'catalog' ? window.location.search : '',
  );

  useEffect(() => {
    const handlePopState = () => {
      const nextRoute = currentRoute();
      setRoute(nextRoute);
      if (nextRoute.kind === 'catalog') setCatalogSearch(window.location.search);
      window.scrollTo({ top: 0, behavior: 'auto' });
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  const navigate = useCallback((path: string) => {
    window.history.pushState({}, '', path);
    setRoute(currentRoute());
    window.scrollTo({ top: 0, behavior: 'auto' });
  }, []);

  const catalogPath = `/skin/${catalogSearch}`;
  const followCatalog = (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    navigate(catalogPath);
  };

  return (
    <div className="app-frame">
      <a className="skip-link" href="#main-content">{t('navBrowse')}</a>
      <header className="site-header">
        <div className="site-header__inner shell">
          <a className="brand" href={catalogPath} onClick={followCatalog} aria-label={`${t('brand')} ${t('market')}`}>
            <img src="/skin/favicon.svg" alt="" width="30" height="30" />
            <span>{t('brand')}</span>
            <span className="brand__divider" aria-hidden="true" />
            <span className="brand__market">{t('market')}</span>
          </a>
          <nav className="site-nav" aria-label={t('market')}>
            <a href={catalogPath} onClick={followCatalog}>{t('navBrowse')}</a>
          </nav>
          <div className="header-actions">
            <button
              type="button"
              className="icon-button language-button"
              onClick={() => setLocale(locale === 'zh-CN' ? 'en-US' : 'zh-CN')}
              aria-label={locale === 'zh-CN' ? t('useEnglish') : t('useChinese')}
              title={locale === 'zh-CN' ? t('useEnglish') : t('useChinese')}
            >
              <GlobeSimple size={19} weight="regular" aria-hidden="true" />
              <span>{locale === 'zh-CN' ? 'EN' : '中'}</span>
            </button>
            <button
              type="button"
              className="icon-button"
              onClick={toggleTheme}
              aria-label={theme === 'dark' ? t('switchToLight') : t('switchToDark')}
              title={theme === 'dark' ? t('switchToLight') : t('switchToDark')}
            >
              {theme === 'dark'
                ? <Sun size={20} weight="regular" aria-hidden="true" />
                : <Moon size={20} weight="regular" aria-hidden="true" />}
            </button>
          </div>
        </div>
      </header>

      {route.kind === 'catalog' ? (
        <CatalogPage
          initialSearch={catalogSearch}
          locale={locale}
          onNavigate={navigate}
          onSearchChange={setCatalogSearch}
          t={t}
        />
      ) : route.kind === 'detail' && route.slug ? (
        <DetailPage
          catalogSearch={catalogSearch}
          locale={locale}
          onNavigate={navigate}
          slug={route.slug}
          t={t}
        />
      ) : (
        <main id="main-content" className="shell detail-state">
          <div className="state-panel">
            <h1>{t('notFoundTitle')}</h1>
            <p>{t('notFoundBody')}</p>
            <a className="primary-button" href={catalogPath} onClick={followCatalog}>{t('backToCatalog')}</a>
          </div>
        </main>
      )}

      <footer className="site-footer">
        <div className="shell site-footer__inner">
          <span>{t('brand')} {t('market')}</span>
          <p>{t('footerNote')}</p>
        </div>
      </footer>
    </div>
  );
}
