import {
  ArrowClockwise,
  ArrowSquareOut,
  GithubLogo,
  GlobeSimple,
  Moon,
  SignOut,
  Sun,
} from '@phosphor-icons/react';
import { useCallback, useEffect, useState } from 'react';
import {
  sharedMarketAccountApi,
  SharedMarketAccountError,
  sharedMarketLoginUrl,
} from './account';
import { AdminPage } from './AdminPage';
import { CatalogPage } from './CatalogPage';
import { DetailPage } from './DetailPage';
import { useI18n } from './i18n';
import { OPENBITFUN_HOME_URL } from './links';
import { adminPath, parseMarketRoute, submissionsPath } from './router';
import { SubmissionsPage } from './SubmissionsPage';
import { useTheme } from './theme';
import type { SharedMarketAccount } from './types';

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
  const [account, setAccount] = useState<SharedMarketAccount>();
  const [accountResolved, setAccountResolved] = useState(false);
  const [accountBusy, setAccountBusy] = useState(false);
  const [accountError, setAccountError] = useState<Error>();
  const [githubAuthConfigured, setGithubAuthConfigured] = useState<boolean>();

  const refreshAccount = useCallback(async () => {
    setAccountError(undefined);
    try {
      setAccount(await sharedMarketAccountApi.me());
    } catch (error) {
      if (error instanceof SharedMarketAccountError && error.code === 'unauthorized') {
        setAccount(undefined);
      } else {
        setAccountError(error instanceof Error ? error : new Error(String(error)));
      }
    } finally {
      setAccountResolved(true);
    }
  }, []);

  useEffect(() => {
    void sharedMarketAccountApi
      .config()
      .then((config) => setGithubAuthConfigured(config.githubAuthConfigured))
      .catch(() => undefined);
    void refreshAccount();
  }, [refreshAccount]);

  useEffect(() => {
    const refreshWhenActive = () => {
      if (document.visibilityState === 'visible') void refreshAccount();
    };
    window.addEventListener('focus', refreshWhenActive);
    document.addEventListener('visibilitychange', refreshWhenActive);
    return () => {
      window.removeEventListener('focus', refreshWhenActive);
      document.removeEventListener('visibilitychange', refreshWhenActive);
    };
  }, [refreshAccount]);

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
  const followPath = (path: string) => (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    navigate(path);
  };
  const followCatalog = followPath(catalogPath);

  const signOut = async () => {
    setAccountBusy(true);
    setAccountError(undefined);
    try {
      await sharedMarketAccountApi.logout();
      setAccount(undefined);
      if (route.kind === 'submissions' || route.kind === 'admin') navigate(catalogPath);
    } catch (error) {
      setAccountError(error instanceof Error ? error : new Error(String(error)));
    } finally {
      setAccountBusy(false);
    }
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
            <a href={catalogPath} onClick={followCatalog} aria-current={route.kind === 'catalog' || route.kind === 'detail' ? 'page' : undefined}>{t('navBrowse')}</a>
            {account && (
              <a href={submissionsPath()} onClick={followPath(submissionsPath())} aria-current={route.kind === 'submissions' ? 'page' : undefined}>
                {t('navSubmissions')}
              </a>
            )}
            {account?.isAdmin && (
              <a href={adminPath()} onClick={followPath(adminPath())} aria-current={route.kind === 'admin' ? 'page' : undefined}>
                {t('navReview')}
              </a>
            )}
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
            {!accountResolved ? (
              <div className="account-loading" role="status" aria-label={t('accountLoading')}>
                <span className="account-loading__avatar" aria-hidden="true" />
                <span className="account-loading__name" aria-hidden="true" />
                <span className="sr-only">{t('accountLoading')}</span>
              </div>
            ) : account ? (
              <div className="account-profile">
                <img src={account.user.avatarUrl} alt="" width="28" height="28" />
                <span title={`@${account.user.login}`}>@{account.user.login}</span>
                <button
                  type="button"
                  className="account-signout"
                  onClick={() => void signOut()}
                  disabled={accountBusy}
                  aria-label={t('signOut')}
                  title={t('signOut')}
                >
                  <SignOut size={18} weight="regular" aria-hidden="true" />
                </button>
              </div>
            ) : (
              <a
                className={`account-signin${githubAuthConfigured === false ? ' disabled' : ''}`}
                href={sharedMarketLoginUrl()}
                aria-disabled={githubAuthConfigured === false}
                title={githubAuthConfigured === false ? t('githubUnavailable') : t('signInGitHub')}
                onClick={(event) => {
                  if (githubAuthConfigured === false) event.preventDefault();
                }}
              >
                <GithubLogo size={18} weight="bold" aria-hidden="true" />
                <span>{t('signInGitHub')}</span>
              </a>
            )}
          </div>
        </div>
      </header>

      {accountError && (
        <div className="account-alert" role="alert" title={accountError.message}>
          <span>{t('accountError')}</span>
          <button type="button" onClick={() => void refreshAccount()}>
            <ArrowClockwise size={17} weight="bold" aria-hidden="true" />
            {t('retryAccount')}
          </button>
        </div>
      )}

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
          isAdmin={account?.isAdmin === true}
          locale={locale}
          onNavigate={navigate}
          slug={route.slug}
          t={t}
        />
      ) : route.kind === 'submissions' ? (
        <SubmissionsPage account={account} accountResolved={accountResolved} locale={locale} t={t} />
      ) : route.kind === 'admin' ? (
        <AdminPage account={account} accountResolved={accountResolved} locale={locale} t={t} />
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
          <a className="site-footer__link" href={OPENBITFUN_HOME_URL} target="_blank" rel="noreferrer">
            {t('openbitfunHome')}
            <ArrowSquareOut size={16} weight="regular" aria-hidden="true" />
          </a>
        </div>
      </footer>
    </div>
  );
}
