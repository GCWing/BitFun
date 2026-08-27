import {
  AppWindow,
  ArrowRight,
  Check,
  Command,
  Eye,
  Heading,
  Keyboard,
  List,
  Rows3,
  MessageCircle,
  MousePointerClick,
  PanelTop,
  Search as SearchIcon,
  ToggleLeft,
} from "lucide-react";
import {
  ActionItem,
  Button,
  Field,
  IconButton,
  Input,
  KeyHint,
  PageHeader,
  SearchField,
  Stack,
  Switch,
  TabGroup,
  ThemeRoot,
  type ColorScheme,
  type ContrastMode,
  type DensityMode,
  type TokenOverrides,
} from "@bitfun/ui";
import { componentRegistry, type ComponentMeta } from "@bitfun/ui/registry";
import { useI18n } from "../i18n";
import {
  getComponentCategoryLabel,
  getComponentDescription,
} from "../i18n/componentMetadata";

interface ComponentsPageProps {
  colorScheme: ColorScheme;
  contrast: ContrastMode;
  density: DensityMode;
  onInspectTokens: () => void;
  onOpenComponent: (name: string) => void;
  tokenOverrides: TokenOverrides;
}

const componentIcons = {
  ActionItem: List,
  Button: MousePointerClick,
  Field: Rows3,
  IconButton: List,
  Input: Eye,
  KeyHint: Keyboard,
  Modal: AppWindow,
  PageHeader: Heading,
  SearchField: SearchIcon,
  Switch: ToggleLeft,
  TabGroup: PanelTop,
} as const;

function ComponentCardPreview({ component }: { component: ComponentMeta }) {
  const { t } = useI18n();

  switch (component.name) {
    case "ActionItem":
      return (
        <ActionItem
          leading={<MessageCircle aria-hidden="true" />}
          shortcut={<KeyHint>K</KeyHint>}
        >
          {t("components.preview.assistant")}
        </ActionItem>
      );
    case "Button":
      return (
        <Stack align="center" direction="horizontal" gap="2" wrap>
          <Button variant="fill">{t("components.preview.primary")}</Button>
          <Button>{t("components.preview.button")}</Button>
        </Stack>
      );
    case "Field":
      return (
        <Field
          description={t("components.preview.fieldDescription")}
          label={t("components.preview.notifications")}
          orientation="horizontal"
        >
          <Switch tabIndex={-1} />
        </Field>
      );
    case "IconButton":
      return (
        <Stack align="center" direction="horizontal" gap="2">
          <IconButton
            aria-label={t("components.preview.listView")}
            icon={<List aria-hidden="true" />}
            tabIndex={-1}
          />
          <IconButton
            aria-label={t("components.preview.listView")}
            icon={<List aria-hidden="true" />}
            tabIndex={-1}
            variant="fill"
          />
        </Stack>
      );
    case "Input":
      return (
        <Input
          aria-label={t("components.preview.inputLabel")}
          placeholder={t("components.preview.inputPlaceholder")}
          trailing={<Eye aria-hidden="true" />}
        />
      );
    case "KeyHint":
      return <KeyHint icon={<Command aria-hidden="true" />}>K</KeyHint>;
    case "Modal":
      return (
        <Button
          leadingIcon={<AppWindow aria-hidden="true" />}
          size="sm"
          tabIndex={-1}
        >
          {t("components.preview.openModal")}
        </Button>
      );
    case "PageHeader":
      return (
        <PageHeader
          description={t("components.preview.appearanceDescription")}
          level={2}
          size="sm"
          title={t("components.preview.appearance")}
        />
      );
    case "SearchField":
      return (
        <SearchField
          aria-label={t("components.preview.searchLabel")}
          leadingIcon={<SearchIcon aria-hidden="true" />}
          placeholder={t("components.preview.searchPlaceholder")}
          shortcut={<KeyHint icon={<Command aria-hidden="true" />}>K</KeyHint>}
        />
      );
    case "Switch":
      return (
        <Stack align="center" direction="horizontal" gap="3">
          <Switch
            aria-label={t("components.preview.notifications")}
            tabIndex={-1}
          />
          <Switch
            aria-label={t("components.preview.notifications")}
            defaultChecked
            tabIndex={-1}
          />
        </Stack>
      );
    case "TabGroup":
      return (
        <TabGroup
          aria-label={t("components.preview.tabGroupLabel")}
          defaultValue="welcome"
          items={[
            {
              icon: <MessageCircle aria-hidden="true" />,
              label: t("components.preview.welcome"),
              value: "welcome",
            },
            {
              icon: <MessageCircle aria-hidden="true" />,
              label: t("components.preview.settings"),
              value: "settings",
            },
          ]}
        />
      );
    default:
      return null;
  }
}

export function ComponentsPage({
  colorScheme,
  contrast,
  density,
  onInspectTokens,
  onOpenComponent,
  tokenOverrides,
}: ComponentsPageProps) {
  const { t } = useI18n();

  return (
    <main className="lab-page" id="components">
      <header className="page-heading page-heading--split">
        <div>
          <span className="page-kicker">{t("components.kicker")}</span>
          <h1>{t("components.title")}</h1>
          <p>{t("components.description")}</p>
        </div>
        <button className="lab-button" onClick={onInspectTokens} type="button">
          {t("components.inspectAllTokens")}
        </button>
      </header>

      <div className="component-summary-strip" aria-label={t("components.summaryLabel")}>
        <span><strong>{componentRegistry.length}</strong> {t("components.registeredCount")}</span>
        <span><strong>{componentRegistry.reduce((total, item) => total + item.states.length, 0)}</strong> {t("components.statesCount")}</span>
        <span><Check aria-hidden="true" size={15} /> {t("components.accessibilityContracts")}</span>
      </div>

      <ThemeRoot
        className="component-catalog-grid"
        colorScheme={colorScheme}
        contrast={contrast}
        density={density}
        tokenOverrides={tokenOverrides}
      >
        {componentRegistry.map((component) => {
          const Icon = componentIcons[component.name as keyof typeof componentIcons];
          return (
            <button
              className="component-card"
              key={component.name}
              onClick={() => onOpenComponent(component.name)}
              type="button"
            >
              <span className="component-card__topline">
                <span className="component-card__icon">
                  {Icon ? <Icon aria-hidden="true" size={19} /> : null}
                </span>
              </span>
              <span className="component-card__preview">
                <ComponentCardPreview component={component} />
              </span>
              <span className="component-card__body">
                <span className="component-card__category">{getComponentCategoryLabel(component.category, t)}</span>
                <strong>{component.name}</strong>
                <span>{getComponentDescription(component.name, component.description, t)}</span>
              </span>
              <span className="component-card__footer">
                {t("components.cardStats", {
                  states: component.states.length,
                  tokens: component.tokens.length,
                })}
                <ArrowRight aria-hidden="true" size={16} />
              </span>
            </button>
          );
        })}
      </ThemeRoot>

      <section className="primitive-note">
        <div>
          <span className="page-kicker">{t("components.primitivesKicker")}</span>
          <h2>{t("components.primitivesTitle")}</h2>
        </div>
        <p>{t("components.primitivesDescription")}</p>
      </section>
    </main>
  );
}
