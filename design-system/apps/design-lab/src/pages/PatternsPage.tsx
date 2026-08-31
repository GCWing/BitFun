import { useMemo, useState, type ReactNode } from "react";
import {
  ActionCard,
  ActivityItem,
  Button,
  Card,
  CardBody,
  CardHeader,
  Disclosure,
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Icon,
  IconButton,
  KeyHint,
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelFooter,
  NavigationPanelHeader,
  NavigationPanelItem,
  NavigationPanelSection,
  PageHeader,
  SearchField,
  SegmentedControl,
  Select,
  StatusPill,
  Switch,
  ThemeRoot,
  type ColorScheme,
  type ContrastMode,
  type DensityMode,
  type IconName,
  type TokenOverrides,
} from "@bitfun/ui";
import { useI18n, type MessageKey } from "../i18n";
import { NestedMenuPattern, ProviderConfigurationPattern, SceneToolbarPattern } from "./ReferencePatterns";

interface PatternsPageProps {
  colorScheme: ColorScheme;
  contrast: ContrastMode;
  density: DensityMode;
  tokenOverrides: TokenOverrides;
}

const quickActions: readonly {
  description: MessageKey;
  icon: IconName;
  title: MessageKey;
}[] = [
  { description: "patterns.actions.newProjectDescription", icon: "plus", title: "patterns.actions.newProject" },
  { description: "patterns.actions.openFilesDescription", icon: "files", title: "patterns.actions.openFiles" },
  { description: "patterns.actions.openBrowserDescription", icon: "browser", title: "patterns.actions.openBrowser" },
  { description: "patterns.actions.openTerminalDescription", icon: "terminal", title: "patterns.actions.openTerminal" },
];

export function PatternsPage({ colorScheme, contrast, density, tokenOverrides }: PatternsPageProps) {
  const { t } = useI18n();
  const [appearance, setAppearance] = useState("system");
  const [fontSize, setFontSize] = useState("medium");
  const [language, setLanguage] = useState("zh-CN");
  const [pointerGlow, setPointerGlow] = useState(true);
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState("all");
  const visibleActions = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return quickActions;
    return quickActions.filter((action) => (
      `${t(action.title)} ${t(action.description)}`.toLowerCase().includes(normalized)
    ));
  }, [query, t]);

  return (
    <ThemeRoot className="patterns-theme-host" colorScheme={colorScheme} contrast={contrast} density={density} tokenOverrides={tokenOverrides}>
      <main className="lab-page lab-page--patterns" id="patterns">
        <header className="page-heading">
          <span className="page-kicker">{t("patterns.kicker")}</span>
          <h1>{t("patterns.title")}</h1>
          <p>{t("patterns.description")}</p>
        </header>

        <PatternSection description={t("patterns.settings.description")} index="01" title={t("patterns.settings.title")}>
          <Card appearance="raised" className="pattern-settings" data-bf-pattern="settings-form" padding="md" radius="md">
            <PageHeader description={t("components.preview.appearanceDescription")} level={3} size="lg" title={t("components.preview.appearance")} />
            <CardBody>
              <FormSection description={t("patterns.settings.description")} headingAs="h4" title={t("components.preview.appearance")}>
                <FieldGroup appearance="subtle" dividers>
                  <FieldRow>
                    <Field controlWidth="fill" description={t("patterns.settings.languageDescription")} label={t("patterns.settings.language")} labelWidth="md" orientation="horizontal">
                      <Select onValueChange={(value) => setLanguage(String(value))} options={[{ label: "简体中文", value: "zh-CN" }, { label: "English", value: "en-US" }, { label: "繁體中文", value: "zh-TW" }]} value={language} />
                    </Field>
                  </FieldRow>
                  <FieldRow>
                    <Field controlWidth="fill" description={t("patterns.settings.themeDescription")} label={t("patterns.settings.theme")} labelWidth="md" orientation="horizontal">
                      <SegmentedControl onValueChange={setAppearance} options={[{ label: t("patterns.settings.system"), value: "system" }, { label: t("settings.light"), value: "light" }, { label: t("settings.dark"), value: "dark" }]} value={appearance} />
                    </Field>
                  </FieldRow>
                  <FieldRow>
                    <Field description={t("patterns.settings.pointerDescription")} label={t("patterns.settings.pointer")} labelWidth="md" orientation="horizontal">
                      <Switch aria-label={t("patterns.settings.pointer")} checked={pointerGlow} onCheckedChange={setPointerGlow} />
                    </Field>
                  </FieldRow>
                </FieldGroup>
              </FormSection>
              <FormSection title={t("patterns.settings.fontSize")}>
                <FieldGroup appearance="subtle">
                  <FieldRow>
                    <Field controlWidth="fill" description={t("patterns.settings.fontSizeDescription")} label={t("patterns.settings.fontSize")} labelWidth="md" orientation="horizontal">
                      <SegmentedControl onValueChange={setFontSize} options={[{ label: t("settings.compact"), value: "small" }, { label: t("settings.comfortable"), value: "medium" }, { label: t("settings.touch"), value: "large" }]} value={fontSize} />
                    </Field>
                  </FieldRow>
                </FieldGroup>
              </FormSection>
            </CardBody>
          </Card>
        </PatternSection>

        <PatternSection description={t("patterns.navigation.description")} index="02" title={t("patterns.navigation.title")}>
          <div className="pattern-navigation-stage" data-bf-pattern="navigation-panel">
            <NavigationPanel
              aria-label={t("patterns.navigation.title")}
            >
              <NavigationPanelHeader>
                <SearchField aria-label={t("patterns.navigation.search")} leadingIcon={<Icon name="search" />} placeholder={t("patterns.navigation.search")} />
              </NavigationPanelHeader>
              <NavigationPanelBody>
                <NavigationPanelContent>
                  <NavigationPanelSection title={t("patterns.navigation.workspace")}>
                    <NavigationPanelItem leading={<Icon name="folder" />} selected>Open-BitFun</NavigationPanelItem>
                    <NavigationPanelItem leading={<Icon name="star" />}>BitFun UI</NavigationPanelItem>
                  </NavigationPanelSection>
                  <Disclosure defaultOpen leading={<Icon name="extension" />} summary={t("patterns.navigation.tools")}>
                    <NavigationPanelItem leading={<Icon name="browser" />}>{t("patterns.actions.openBrowser")}</NavigationPanelItem>
                    <NavigationPanelItem leading={<Icon name="terminal" />}>{t("patterns.actions.openTerminal")}</NavigationPanelItem>
                  </Disclosure>
                  <NavigationPanelSection title={t("patterns.navigation.projects")}>
                    <NavigationPanelItem leading={<Icon name="files" />}>design-system</NavigationPanelItem>
                    <NavigationPanelItem leading={<Icon name="git" />}>fmy/ui-sys</NavigationPanelItem>
                  </NavigationPanelSection>
                </NavigationPanelContent>
              </NavigationPanelBody>
              <NavigationPanelFooter>
                <div className="pattern-navigation-footer"><StatusPill leading={<Icon name="circle" />} tone="success">{t("patterns.device.online")}</StatusPill><IconButton aria-label={t("patterns.device.refresh")} icon={<Icon name="refresh" />} size="xs" variant="quiet" /></div>
              </NavigationPanelFooter>
            </NavigationPanel>
            <div className="pattern-navigation-copy">
              <PageHeader description={t("patterns.navigation.description")} level={3} size="lg" title={t("patterns.navigation.workspace")} />
              <p>{t("patterns.navigation.status")}</p>
            </div>
          </div>
        </PatternSection>

        <PatternSection description={t("patterns.search.description")} index="03" title={t("patterns.search.title")}>
          <Card appearance="raised" className="pattern-command" data-bf-pattern="search-command-surface" gap="md" padding="md" radius="md">
            <CardHeader actions={<SegmentedControl onValueChange={setScope} options={[{ label: t("patterns.search.all"), value: "all" }, { label: t("patterns.search.files"), value: "files" }, { label: t("patterns.search.commands"), value: "commands" }]} value={scope} />} description={t("patterns.search.description")} title={t("patterns.search.title")} />
            <SearchField aria-label={t("patterns.search.searchPlaceholder")} clearLabel={t("components.preview.close")} leadingIcon={<Icon name="search" />} onClear={() => setQuery("")} onValueChange={setQuery} placeholder={t("patterns.search.searchPlaceholder")} shortcut={<KeyHint>Ctrl K</KeyHint>} value={query} />
            <CardBody>
              <div className="pattern-action-grid">
                {visibleActions.map((action) => <ActionCard description={t(action.description)} key={action.title} leading={<Icon name={action.icon} />} size="md">{t(action.title)}</ActionCard>)}
              </div>
              <div className="pattern-recent-list">
                <strong>{t("patterns.search.recent")}</strong>
                <ActivityItem actions={[{ icon: <Icon name="arrow-up-right" />, id: "open-readme", label: t("patterns.actions.openFiles") }]} appearance="surface" label="README.md" leading={<Icon name="files" />}>design-system/README.md</ActivityItem>
                <ActivityItem actions={[{ icon: <Icon name="arrow-up-right" />, id: "open-package", label: t("patterns.actions.openFiles") }]} appearance="surface" label="package.json" leading={<Icon name="files" />}>design-system/packages/ui/package.json</ActivityItem>
              </div>
            </CardBody>
          </Card>
        </PatternSection>

        <PatternSection description={t("patterns.device.description")} index="04" title={t("patterns.device.title")}>
          <Card appearance="subtle" className="pattern-device-card" data-bf-pattern="device-card" gap="md" padding="md" radius="md">
            <CardHeader actions={<IconButton aria-label={t("patterns.device.refresh")} icon={<Icon name="refresh" />} size="sm" variant="quiet" />} description="macOS · 127.0.0.1" leading={<span className="pattern-device-icon"><Icon name="device-mac" size="lg" /></span>} title="MacBook Pro" />
            <CardBody><StatusPill leading={<Icon name="unselected" />} tone="success">{t("patterns.device.online")}</StatusPill></CardBody>
            <Button leadingIcon={<Icon name="link" />} size="sm" variant="fill">{t("patterns.device.connect")}</Button>
          </Card>
        </PatternSection>
        <PatternSection description={t("patterns.provider.description")} index="05" title={t("patterns.provider.title")}>
          <ProviderConfigurationPattern />
        </PatternSection>
        <PatternSection description={t("patterns.toolbar.description")} index="06" title={t("patterns.toolbar.title")}>
          <SceneToolbarPattern />
        </PatternSection>
        <PatternSection description={t("patterns.menu.description")} index="07" title={t("patterns.menu.title")}>
          <NestedMenuPattern />
        </PatternSection>
      </main>
    </ThemeRoot>
  );
}

function PatternSection({ children, description, index, title }: { children: ReactNode; description: string; index: string; title: string }) {
  return (
    <section className="pattern-section">
      <div className="pattern-section__heading"><span>{index}</span><div><h2>{title}</h2><p>{description}</p></div></div>
      {children}
    </section>
  );
}
