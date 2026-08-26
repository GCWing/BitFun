import {
  ArrowRight,
  Check,
  Keyboard,
  Image as ImageIcon,
  List,
  MessageCircle,
  MousePointerClick,
  MessageSquareText,
  PanelTop,
  PanelLeft,
  Search as SearchIcon,
  ToggleLeft,
  Type,
  User,
} from "lucide-react";
import {
  Button,
  Heading,
  IconButton,
  Input,
  KeyHint,
  ListItem,
  MediaThumbnail,
  NavigationList,
  NavigationListSection,
  PromptComposer,
  Search,
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
import homepagePreviewImage from "../assets/homepage-preview.png";
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
  Button: MousePointerClick,
  Heading: Type,
  IconButton: List,
  Input: Type,
  KeyHint: Keyboard,
  ListItem: User,
  MediaThumbnail: ImageIcon,
  NavigationList: PanelLeft,
  PromptComposer: MessageSquareText,
  Search: SearchIcon,
  Switch: ToggleLeft,
  TabGroup: PanelTop,
} as const;

function ComponentCardPreview({ component }: { component: ComponentMeta }) {
  const { t } = useI18n();

  switch (component.name) {
    case "Button":
      return (
        <Stack align="center" direction="horizontal" gap="2" wrap>
          <Button tone="primary" variant="fill">{t("components.preview.primary")}</Button>
          <Button>{t("components.preview.button")}</Button>
          <Button variant="text">{t("components.preview.button")}</Button>
        </Stack>
      );
    case "IconButton":
      return (
        <IconButton aria-label={t("components.preview.moreActions")}>
          <List aria-hidden="true" />
        </IconButton>
      );
    case "Heading":
      return (
        <Heading
          description="Interface language and visual appearance"
          title="Appearance"
          variant="section"
        />
      );
    case "Input":
      return (
        <Input
          aria-label="Description"
          defaultValue="BitFun is an AI-driven programming environment."
        />
      );
    case "Search":
      return (
        <Search
          aria-label="Search"
          placeholder="Search"
          trailingContent={<KeyHint>⌘ K</KeyHint>}
        />
      );
    case "KeyHint":
      return <KeyHint leadingIcon={<Keyboard aria-hidden="true" />}>K</KeyHint>;
    case "ListItem":
      return (
        <ListItem leadingIcon={<User aria-hidden="true" />} selected>
          AI Assistant
        </ListItem>
      );
    case "MediaThumbnail":
      return (
        <MediaThumbnail
          alt="BitFun homepage"
          presentation="stacked"
          src={homepagePreviewImage}
        />
      );
    case "NavigationList":
      return (
        <NavigationList aria-label="Settings" className="component-card-navigation-preview">
          <NavigationListSection title="General">
            <ListItem>About</ListItem>
            <ListItem selected>Appearance</ListItem>
          </NavigationListSection>
        </NavigationList>
      );
    case "PromptComposer":
      return (
        <PromptComposer
          aria-label="Prompt"
          className="component-card-prompt-preview"
          endControls={<span>Send</span>}
          placeholder="How can I help you..."
          startControls={<span>+</span>}
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
