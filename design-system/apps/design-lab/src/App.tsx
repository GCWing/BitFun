import {
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { SystemTokenMode } from "@bitfun/design-tokens";
import type { ThemeDataName } from "@bitfun/theme-bitfun";
import {
  AppWindow,
  Blocks,
  BookOpen,
  Braces,
  CircleDashed,
  FileText,
  House,
  Languages,
  Menu,
  Moon,
  MousePointerClick,
  Palette,
  PanelTop,
  Search,
  Settings2,
  Sun,
  ToggleLeft,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  ThemeRoot,
  type ColorScheme,
  type ContrastMode,
  type DensityMode,
} from "@bitfun/ui";
import { componentRegistry } from "@bitfun/ui/registry";
import {
  useI18n,
  type DesignLabLocale,
  type MessageKey,
} from "./i18n";
import {
  getComponentCategoryLabel,
  getComponentDescription,
} from "./i18n/componentMetadata";
import { OverviewPage } from "./pages/OverviewPage";
import { ComponentsPage } from "./pages/ComponentsPage";
import { ComponentDetailPage } from "./pages/ComponentDetailPage";
import { GettingStartedPage } from "./pages/GettingStartedPage";
import { ResourcesPage } from "./pages/ResourcesPage";
import { ColorsPage } from "./pages/ColorsPage";
import {
  colorTokenCatalog,
  editableTokenCatalog,
  nonColorTokenCatalog,
  type EditableToken,
} from "./token-editor/catalog";
import {
  buildActiveTokenOverrides,
  getActiveTokenMode,
  getTokenDraftKey,
  loadTokenDrafts,
  persistTokenDrafts,
  type TokenDrafts,
  type TokenEditorContext,
} from "./token-editor/model";
import { TokenWorkbench } from "./token-editor/TokenWorkbench";
import { TokenEffectPreview } from "./token-editor/TokenEffectPreview";

type LabRoute =
  | { page: "overview" }
  | { page: "getting-started" }
  | { page: "components" }
  | { page: "colors" }
  | { page: "resources" }
  | { page: "tokens" }
  | { componentName: string; page: "component" };

interface SearchDestination {
  detail: string;
  icon: LucideIcon;
  keywords: string;
  label: string;
  route: LabRoute;
}

const componentIcons: Record<string, LucideIcon> = {
  Button: MousePointerClick,
  Modal: AppWindow,
  Switch: ToggleLeft,
  TabGroup: PanelTop,
};

function getThemeDataName(
  colorScheme: ColorScheme,
  contrast: ContrastMode,
): ThemeDataName {
  if (contrast === "high") {
    return colorScheme === "dark" ? "highContrastDark" : "highContrastLight";
  }
  return colorScheme;
}

function parseRoute(hash: string): LabRoute {
  const route = hash.replace(/^#/, "").toLowerCase();
  if (!route || route === "overview") {
    return { page: "overview" };
  }
  if (route === "tokens") {
    return { page: "tokens" };
  }
  if (route === "colors") {
    return { page: "colors" };
  }
  if (route === "getting-started") {
    return { page: "getting-started" };
  }
  if (route === "components") {
    return { page: "components" };
  }
  if (route === "resources") {
    return { page: "resources" };
  }

  const componentSlug = route.startsWith("component/")
    ? route.slice("component/".length)
    : route;
  const component = componentRegistry.find(
    (candidate) => candidate.name.toLowerCase() === componentSlug,
  );
  return component
    ? { componentName: component.name, page: "component" }
    : { page: "overview" };
}

function routeHash(route: LabRoute): string {
  return route.page === "component"
    ? `#component/${route.componentName.toLowerCase()}`
    : `#${route.page}`;
}

export function App() {
  const { locale, setLocale, t } = useI18n();
  const [colorScheme, setColorScheme] = useState<ColorScheme>("light");
  const [contrast, setContrast] = useState<ContrastMode>("standard");
  const [density, setDensity] = useState<DensityMode>("comfortable");
  const [route, setRoute] = useState<LabRoute>(() => parseRoute(window.location.hash));
  const [componentScope, setComponentScope] = useState("all");
  const [drafts, setDrafts] = useState<TokenDrafts>(loadTokenDrafts);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const settingsRef = useRef<HTMLDivElement>(null);
  const searchShortcut = /Mac|iPhone|iPad/i.test(navigator.userAgent) ? "⌘ K" : "Ctrl K";

  const themeName = getThemeDataName(colorScheme, contrast);
  const editorContext = useMemo<TokenEditorContext>(() => ({
    density: density as SystemTokenMode,
    theme: themeName,
  }), [density, themeName]);
  const tokenOverrides = useMemo(
    () => buildActiveTokenOverrides(editorContext, drafts),
    [drafts, editorContext],
  );

  const searchDestinations = useMemo<SearchDestination[]>(() => [
    {
      detail: t("search.overviewDetail"),
      icon: House,
      keywords: `home start overview ${t("nav.overview")}`,
      label: t("nav.overview"),
      route: { page: "overview" },
    },
    {
      detail: t("search.gettingStartedDetail"),
      icon: BookOpen,
      keywords: `install packages quick start ${t("nav.gettingStarted")}`,
      label: t("nav.gettingStarted"),
      route: { page: "getting-started" },
    },
    {
      detail: t("search.tokensDetail", { count: nonColorTokenCatalog.length }),
      icon: Braces,
      keywords: `design tokens spacing typography radius motion theme ${t("nav.designTokens")}`,
      label: t("nav.designTokens"),
      route: { page: "tokens" },
    },
    {
      detail: t("search.colorsDetail", { count: colorTokenCatalog.length }),
      icon: Palette,
      keywords: `colors semantic palette scale reference theme ${t("nav.colors")}`,
      label: t("nav.colors"),
      route: { page: "colors" },
    },
    {
      detail: t("search.componentsDetail", { count: componentRegistry.length }),
      icon: Blocks,
      keywords: `component library catalog ${t("nav.components")}`,
      label: t("nav.components"),
      route: { page: "components" },
    },
    {
      detail: t("search.resourcesDetail"),
      icon: FileText,
      keywords: `readme release policy package documentation ${t("nav.resources")}`,
      label: t("nav.resources"),
      route: { page: "resources" },
    },
    ...componentRegistry.map((component) => {
      const category = getComponentCategoryLabel(component.category, t);
      const description = getComponentDescription(component.name, component.description, t);
      return {
        detail: t("search.componentDetail", { category }),
        icon: componentIcons[component.name] ?? Blocks,
        keywords: `${component.name} ${component.category} ${component.description} ${category} ${description}`,
        label: component.name,
        route: { componentName: component.name, page: "component" } as const,
      };
    }),
  ], [t]);

  const visibleSearchDestinations = useMemo(() => {
    const normalized = searchQuery.trim().toLowerCase();
    if (!normalized) {
      return searchDestinations;
    }
    return searchDestinations.filter((destination) =>
      `${destination.label} ${destination.detail} ${destination.keywords}`
        .toLowerCase()
        .includes(normalized),
    );
  }, [searchDestinations, searchQuery]);

  useEffect(() => {
    persistTokenDrafts(drafts);
  }, [drafts]);

  useEffect(() => {
    function syncRoute() {
      setRoute(parseRoute(window.location.hash));
      setSidebarOpen(false);
      window.scrollTo({ top: 0 });
    }
    window.addEventListener("hashchange", syncRoute);
    return () => window.removeEventListener("hashchange", syncRoute);
  }, []);

  useEffect(() => {
    function handleKeyboard(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
        setSearchOpen(true);
      }
      if (event.key === "Escape") {
        setSearchOpen(false);
        setSettingsOpen(false);
        setSidebarOpen(false);
      }
    }
    window.addEventListener("keydown", handleKeyboard);
    return () => window.removeEventListener("keydown", handleKeyboard);
  }, []);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (
        settingsOpen &&
        settingsRef.current &&
        !settingsRef.current.contains(event.target as Node)
      ) {
        setSettingsOpen(false);
      }
    }
    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, [settingsOpen]);

  function navigate(nextRoute: LabRoute) {
    const nextHash = routeHash(nextRoute);
    setSearchOpen(false);
    setSearchQuery("");
    setSidebarOpen(false);
    if (window.location.hash === nextHash) {
      setRoute(nextRoute);
      window.scrollTo({ top: 0 });
      return;
    }
    window.location.hash = nextHash;
  }

  function changeToken(token: EditableToken, value: string) {
    const mode = getActiveTokenMode(token, editorContext);
    const key = getTokenDraftKey(token.collection, mode, token.name);
    setDrafts((current) => {
      if (value === token.values[mode]) {
        const next = { ...current };
        delete next[key];
        return next;
      }
      return { ...current, [key]: value };
    });
  }

  function resetToken(token: EditableToken) {
    const mode = getActiveTokenMode(token, editorContext);
    const key = getTokenDraftKey(token.collection, mode, token.name);
    setDrafts((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
  }

  function openTokenPage(scope = "all") {
    setComponentScope(scope);
    navigate({ page: "tokens" });
  }

  function changeThemeMode(nextMode: ThemeDataName) {
    switch (nextMode) {
      case "dark":
        setColorScheme("dark");
        setContrast("standard");
        break;
      case "highContrastDark":
        setColorScheme("dark");
        setContrast("high");
        break;
      case "highContrastLight":
        setColorScheme("light");
        setContrast("high");
        break;
      case "light":
        setColorScheme("light");
        setContrast("standard");
        break;
    }
  }

  const activeComponent = route.page === "component"
    ? componentRegistry.find((component) => component.name === route.componentName)
    : undefined;

  return (
    <div
      className="lab-shell"
      data-lab-scheme={colorScheme}
      data-sidebar-open={sidebarOpen || undefined}
    >
      <button
        aria-label={t("app.closeNavigation")}
        className="lab-sidebar-backdrop"
        onClick={() => setSidebarOpen(false)}
        type="button"
      />

      <aside className="lab-sidebar">
        <div className="lab-brand">
          <span className="lab-brand__mark"><CircleDashed aria-hidden="true" size={25} strokeWidth={2.35} /></span>
          <span>
            <strong>BitFun Design</strong>
          </span>
          <button aria-label={t("app.closeNavigation")} onClick={() => setSidebarOpen(false)} type="button">
            <X aria-hidden="true" size={18} />
          </button>
        </div>

        <nav aria-label={t("app.pagesLabel")} className="lab-navigation">
          <a
            aria-current={route.page === "overview" ? "page" : undefined}
            href="#overview"
            onClick={(event) => {
              event.preventDefault();
              navigate({ page: "overview" });
            }}
          >
            <House aria-hidden="true" size={17} />
            <span>{t("nav.overview")}</span>
          </a>
          <a
            aria-current={route.page === "getting-started" ? "page" : undefined}
            href="#getting-started"
            onClick={(event) => {
              event.preventDefault();
              navigate({ page: "getting-started" });
            }}
          >
            <BookOpen aria-hidden="true" size={17} />
            <span>{t("nav.gettingStarted")}</span>
          </a>

          <span className="lab-nav-label">{t("nav.foundations")}</span>
          <a
            aria-current={route.page === "tokens" ? "page" : undefined}
            href="#tokens"
            onClick={(event) => {
              event.preventDefault();
              openTokenPage();
            }}
          >
            <Braces aria-hidden="true" size={17} />
            <span>{t("nav.designTokens")}</span>
            <small>{nonColorTokenCatalog.length}</small>
          </a>
          <a
            aria-current={route.page === "colors" ? "page" : undefined}
            href="#colors"
            onClick={(event) => {
              event.preventDefault();
              navigate({ page: "colors" });
            }}
          >
            <Palette aria-hidden="true" size={17} />
            <span>{t("nav.colors")}</span>
            <small>{colorTokenCatalog.length}</small>
          </a>

          <span className="lab-nav-label">{t("nav.library")}</span>
          <a
            aria-current={route.page === "components" ? "page" : undefined}
            data-expanded={route.page === "component" || route.page === "components" || undefined}
            href="#components"
            onClick={(event) => {
              event.preventDefault();
              navigate({ page: "components" });
            }}
          >
            <Blocks aria-hidden="true" size={17} />
            <span>{t("nav.components")}</span>
          </a>
          <div className="lab-component-links">
            {componentRegistry.map((component) => {
              const Icon = componentIcons[component.name] ?? Blocks;
              const active = route.page === "component" && route.componentName === component.name;
              return (
                <a
                  aria-current={active ? "page" : undefined}
                  href={`#component/${component.name.toLowerCase()}`}
                  key={component.name}
                  onClick={(event) => {
                    event.preventDefault();
                    navigate({ componentName: component.name, page: "component" });
                  }}
                >
                  <Icon aria-hidden="true" size={15} />
                  <span>{component.name}</span>
                </a>
              );
            })}
          </div>
        </nav>

        <div className="lab-sidebar-footer">
          <span>v0.1.0</span>
          <button
            onClick={() => {
              setSettingsOpen(true);
              setSidebarOpen(false);
            }}
            type="button"
          >
            {t(`settings.${density}` as MessageKey)}
            <Settings2 aria-hidden="true" size={13} />
          </button>
        </div>
      </aside>

      <div className="lab-workspace">
        <header className="lab-topbar">
          <button
            aria-label={t("app.openNavigation")}
            className="topbar-menu-button"
            onClick={() => setSidebarOpen(true)}
            type="button"
          >
            <Menu aria-hidden="true" size={19} />
          </button>

          <div className="lab-search">
            <Search aria-hidden="true" size={17} />
            <input
              aria-autocomplete="list"
              aria-controls="lab-search-results"
              aria-expanded={searchOpen}
              aria-label={t("search.label")}
              onBlur={() => window.setTimeout(() => setSearchOpen(false), 120)}
              onChange={(event) => {
                setSearchQuery(event.target.value);
                setSearchOpen(true);
              }}
              onFocus={() => setSearchOpen(true)}
              placeholder={t("search.placeholder")}
              ref={searchInputRef}
              type="search"
              value={searchQuery}
            />
            <kbd>{searchShortcut}</kbd>
            {searchOpen && (
              <div className="lab-search-results" id="lab-search-results" role="listbox">
                {visibleSearchDestinations.map((destination) => {
                  const Icon = destination.icon;
                  return (
                    <button
                      key={`${destination.route.page}-${destination.label}`}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => navigate(destination.route)}
                      role="option"
                      type="button"
                    >
                      <span><Icon aria-hidden="true" size={16} /></span>
                      <span>
                        <strong>{destination.label}</strong>
                        <small>{destination.detail}</small>
                      </span>
                    </button>
                  );
                })}
                {visibleSearchDestinations.length === 0 && (
                  <p>{t("search.noResults", { query: searchQuery })}</p>
                )}
              </div>
            )}
          </div>

          <nav className="topbar-links" aria-label={t("nav.resources")}>
            <a href="https://github.com/GCWing/BitFun/tree/main/design-system" rel="noreferrer" target="_blank">{t("nav.docs")}</a>
            <a
              href="#resources"
              onClick={(event) => {
                event.preventDefault();
                navigate({ page: "resources" });
              }}
            >
              {t("nav.resources")}
            </a>
          </nav>

          <label className="lab-language-control">
            <Languages aria-hidden="true" size={17} />
            <select
              aria-label={t("language.label")}
              onChange={(event) => setLocale(event.target.value as DesignLabLocale)}
              value={locale}
            >
              <option value="zh-CN">简体中文</option>
              <option value="en-US">English</option>
              <option value="zh-TW">繁體中文</option>
            </select>
          </label>

          <button
            aria-label={colorScheme === "light" ? t("theme.switchToDark") : t("theme.switchToLight")}
            className="topbar-icon-button"
            onClick={() => setColorScheme((current) => current === "light" ? "dark" : "light")}
            type="button"
          >
            {colorScheme === "light"
              ? <Sun aria-hidden="true" size={18} />
              : <Moon aria-hidden="true" size={18} />}
          </button>

          <div className="lab-settings" ref={settingsRef}>
            <button
              aria-controls="lab-settings-panel"
              aria-expanded={settingsOpen}
              aria-label={t("settings.label")}
              className="topbar-icon-button"
              onClick={() => setSettingsOpen((current) => !current)}
              type="button"
            >
              <Settings2 aria-hidden="true" size={18} />
            </button>
            {settingsOpen && (
              <div className="lab-settings-panel" id="lab-settings-panel">
                <div className="lab-settings-panel__heading">
                  <div>
                    <strong>{t("settings.title")}</strong>
                    <span>{t("settings.subtitle")}</span>
                  </div>
                  <button aria-label={t("settings.close")} onClick={() => setSettingsOpen(false)} type="button">
                    <X aria-hidden="true" size={16} />
                  </button>
                </div>
                <label>
                  <span>{t("settings.scheme")}</span>
                  <select onChange={(event) => setColorScheme(event.target.value as ColorScheme)} value={colorScheme}>
                    <option value="light">{t("settings.light")}</option>
                    <option value="dark">{t("settings.dark")}</option>
                  </select>
                </label>
                <label>
                  <span>{t("settings.contrast")}</span>
                  <select onChange={(event) => setContrast(event.target.value as ContrastMode)} value={contrast}>
                    <option value="standard">{t("settings.standard")}</option>
                    <option value="high">{t("settings.highContrast")}</option>
                  </select>
                </label>
                <label>
                  <span>{t("settings.density")}</span>
                  <select onChange={(event) => setDensity(event.target.value as DensityMode)} value={density}>
                    <option value="compact">{t("settings.compact")}</option>
                    <option value="comfortable">{t("settings.comfortable")}</option>
                    <option value="touch">{t("settings.touch")}</option>
                  </select>
                </label>
                <button
                  className="settings-reset-button"
                  disabled={Object.keys(drafts).length === 0}
                  onClick={() => setDrafts({})}
                  type="button"
                >
                  {t("settings.resetTokenDrafts")}
                  <span>{Object.keys(drafts).length}</span>
                </button>
              </div>
            )}
          </div>

        </header>

        <div className="lab-content">
          {route.page === "overview" && (
            <OverviewPage onNavigate={(target) => navigate({ page: target })} />
          )}

          {route.page === "getting-started" && (
            <GettingStartedPage
              onNavigate={(target) => navigate({ page: target })}
            />
          )}

          {route.page === "components" && (
            <ComponentsPage
              colorScheme={colorScheme}
              contrast={contrast}
              density={density}
              onInspectTokens={() => openTokenPage()}
              onOpenComponent={(name) => navigate({ componentName: name, page: "component" })}
              tokenOverrides={tokenOverrides}
            />
          )}

          {route.page === "component" && activeComponent && (
            <ComponentDetailPage
              colorScheme={colorScheme}
              component={activeComponent}
              contrast={contrast}
              density={density}
              key={activeComponent.name}
              onBack={() => navigate({ page: "components" })}
              onInspectTokens={openTokenPage}
              tokenOverrides={tokenOverrides}
            />
          )}

          {route.page === "tokens" && (
            <TokenWorkbench
              componentScope={componentScope}
              context={editorContext}
              drafts={drafts}
              onComponentScopeChange={setComponentScope}
              onResetAll={() => setDrafts({})}
              onResetToken={resetToken}
              onTokenChange={changeToken}
              preview={(
                <ThemeRoot
                  className="token-preview-theme-host"
                  colorScheme={colorScheme}
                  contrast={contrast}
                  density={density}
                  tokenOverrides={tokenOverrides}
                >
                  <TokenEffectPreview />
                </ThemeRoot>
              )}
            />
          )}

          {route.page === "colors" && (
            <ColorsPage
              density={density}
              mode={themeName}
              onDensityChange={setDensity}
              onModeChange={changeThemeMode}
            />
          )}

          {route.page === "resources" && <ResourcesPage />}
        </div>
      </div>
    </div>
  );
}
