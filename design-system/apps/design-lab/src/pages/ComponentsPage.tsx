import {
  AppWindow,
  ArrowUp,
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
  PanelLeft,
  Plus,
  Search as SearchIcon,
  Settings,
  Terminal,
  ToggleLeft,
} from "lucide-react";
import {
  ActionCard,
  ActionItem,
  ActivityItem,
  Button,
  Card,
  CardHeader,
  ChangeCount,
  Composer,
  ComposerToolbar,
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Icon,
  IconButton,
  Input,
  KeyHint,
  Menu,
  MenuItem,
  MenuSection,
  NavigationPanel,
  NavigationPanelItem,
  NavigationPanelSection,
  PageHeader,
  ScrollArea,
  SearchField,
  SegmentedControl,
  Select,
  Stack,
  StatusPill,
  Switch,
  TabGroup,
  ThemeRoot,
  Toolbar,
  ToolbarBadge,
  ToolbarGroup,
  ToolbarSeparator,
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
import {
  FlowChatComponentPreview,
  flowChatPreviewRegistry,
  getFlowChatPreviewDefinition,
} from "../preview/FlowChatPreviewRegistry";
import { FlowChatToolGallery } from "../preview/FlowChatToolGallery";

interface ComponentsPageProps {
  category?: ComponentMeta["category"];
  colorScheme: ColorScheme;
  contrast: ContrastMode;
  density: DensityMode;
  onInspectTokens: () => void;
  onOpenComponent: (name: string) => void;
  tokenOverrides: TokenOverrides;
}

const componentIcons = {
  ActionCard: MousePointerClick,
  ActionItem: List,
  ActivityItem: Terminal,
  Button: MousePointerClick,
  Card: Rows3,
  Composer: ArrowUp,
  Field: Rows3,
  Icon: SearchIcon,
  IconButton: List,
  Input: Eye,
  KeyHint: Keyboard,
  Menu: List,
  Modal: AppWindow,
  NavigationPanel: PanelLeft,
  PageHeader: Heading,
  ScrollArea: Rows3,
  SearchField: SearchIcon,
  SegmentedControl: ToggleLeft,
  Select: List,
  StatusPill: Check,
  Switch: ToggleLeft,
  TabGroup: PanelTop,
  Toolbar: PanelTop,
} as const;

function ComponentCardPreview({ component }: { component: ComponentMeta }) {
  const { t } = useI18n();
  const flowChatPreview = getFlowChatPreviewDefinition(component.name);

  if (flowChatPreview) {
    return (
      <FlowChatComponentPreview
        componentName={component.name}
        interactive={false}
      />
    );
  }

  switch (component.name) {
    case "ActionCard":
      return (
        <ActionCard
          className="component-action-card-card-preview"
          description={t("components.preview.actionCardDescription")}
          leading={<MessageCircle aria-hidden="true" />}
          tabIndex={-1}
        >
          {t("components.preview.actionCardTitle")}
        </ActionCard>
      );
    case "ActionItem":
      return (
        <ActionItem
          leading={<MessageCircle aria-hidden="true" />}
          shortcut={<KeyHint>K</KeyHint>}
        >
          {t("components.preview.assistant")}
        </ActionItem>
      );
    case "ActivityItem":
      return (
        <ActivityItem
          appearance="surface"
          className="component-activity-item-card-preview"
          label={t("components.preview.activityAction")}
          leading={<Terminal aria-hidden="true" />}
          metadata={<ChangeCount additions={6} deletions={0} />}
        >
          {t("components.preview.activityDescription")}
        </ActivityItem>
      );
    case "Button":
      return (
        <Stack align="center" direction="horizontal" gap="2" wrap>
          <Button variant="fill">{t("components.preview.primary")}</Button>
          <Button>{t("components.preview.button")}</Button>
        </Stack>
      );
    case "Card":
      return (
        <Card
          appearance="subtle"
          className="component-card-card-preview"
          gap="sm"
          padding="sm"
          radius="sm"
        >
          <CardHeader
            align="center"
            description={t("components.preview.cardDescription")}
            leading={<Command aria-hidden="true" />}
            title={t("components.preview.cardTitle")}
          />
        </Card>
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
    case "Icon":
      return (
        <Stack align="center" direction="horizontal" gap="3">
          <Icon name="search" tone="primary" />
          <Icon name="folder" tone="secondary" />
          <Icon name="check-circle" tone="success" />
        </Stack>
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
    case "Menu":
      return (
        <Menu aria-label={t("components.preview.menuLabel")} scrollbarVisibility="hidden">
          <MenuSection title={t("components.preview.menuSectionTitle")}>
            <MenuItem leading={<MessageCircle aria-hidden="true" />} tabIndex={-1}>
              {t("components.preview.menuItemOne")}
            </MenuItem>
            <MenuItem leading={<MessageCircle aria-hidden="true" />} tabIndex={-1}>
              {t("components.preview.menuItemTwo")}
            </MenuItem>
          </MenuSection>
        </Menu>
      );
    case "FieldGroup":
      return (
        <FormSection
          headingAs="h3"
          leading={<Settings aria-hidden="true" />}
          title={t("components.preview.modalSectionTitle")}
        >
          <FieldGroup>
            <FieldRow>
              <Field controlWidth="fill" label={t("components.preview.modalProviderName")} labelWidth="sm" orientation="horizontal">
                <Input defaultValue="OpenBitFun" readOnly />
              </Field>
            </FieldRow>
          </FieldGroup>
        </FormSection>
      );
    case "ConfirmDialog":
      return (
        <Button leadingIcon={<AppWindow aria-hidden="true" />} size="sm" variant="fill">
          {t("components.preview.confirmDelete")}
        </Button>
      );
    case "Composer":
      return (
        <Composer
          aria-label={t("components.preview.composerLabel")}
          className="component-composer-card-preview"
          toolbar={(
            <ComposerToolbar
              leading={(
                <IconButton
                  aria-label={t("components.preview.composerAdd")}
                  icon={<Plus aria-hidden="true" />}
                  size="sm"
                  tabIndex={-1}
                  variant="fill"
                />
              )}
              trailing={(
                <IconButton
                  aria-label={t("components.preview.composerSend")}
                  icon={<ArrowUp aria-hidden="true" />}
                  size="sm"
                  tabIndex={-1}
                  variant="primary"
                />
              )}
            />
          )}
        >
          <span className="component-composer-placeholder">
            {t("components.preview.composerPlaceholder")}
          </span>
        </Composer>
      );
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
          leading={<Heading aria-hidden="true" />}
          level={2}
          size="sm"
          title={t("components.preview.appearance")}
        />
      );
    case "NavigationPanel":
      return (
        <NavigationPanel
          aria-label={t("components.preview.navigationPanelLabel")}
          className="component-navigation-panel-card-preview"
          footer={<span>{t("components.preview.navigationPanelDevice")}</span>}
          scrollbarVisibility="hidden"
        >
          <NavigationPanelSection title={t("components.preview.navigationPanelSectionTitle")}>
            <NavigationPanelItem leading={<MessageCircle aria-hidden="true" />} selected tabIndex={-1}>
              {t("components.preview.menuItemOne")}
            </NavigationPanelItem>
            <NavigationPanelItem reserveLeadingSpace tabIndex={-1}>
              {t("components.preview.menuItemTwo")}
            </NavigationPanelItem>
          </NavigationPanelSection>
        </NavigationPanel>
      );
    case "ScrollArea":
      return (
        <ScrollArea
          aria-label={t("components.preview.scrollAreaLabel")}
          className="component-scroll-area-card-preview"
        >
          <div className="component-scroll-area-example__content">
            {Array.from({ length: 5 }, (_, index) => (
              <span className="component-scroll-area-example__item" key={index}>
                {t("components.preview.scrollAreaItem", { index: index + 1 })}
              </span>
            ))}
          </div>
        </ScrollArea>
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
    case "StatusPill":
      return (
        <StatusPill leading={<Icon name="circle" />}>
          Ask
        </StatusPill>
      );
    case "Select":
      return (
        <Select
          aria-label={t("components.preview.appearance")}
          options={[
            { label: "Ask", value: "ask" },
            { label: "Plan", value: "plan" },
          ]}
          value="ask"
        />
      );
    case "SegmentedControl":
      return (
        <SegmentedControl
          aria-label={t("components.preview.segmentedLabel")}
          defaultValue="chat"
          options={[
            {
              icon: <MessageCircle aria-hidden="true" />,
              label: t("components.preview.segmentedChat"),
              value: "chat",
            },
            {
              label: t("components.preview.segmentedAgent"),
              value: "agent",
            },
          ]}
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
    case "Toolbar":
      return (
        <Toolbar
          aria-label={t("components.preview.tabGroupLabel")}
          center={(
            <ToolbarGroup>
              <ToolbarBadge>18</ToolbarBadge>
              <strong>{t("components.preview.session")}</strong>
            </ToolbarGroup>
          )}
          className="component-toolbar-card-preview"
          leading={(
            <Button size="xs" tabIndex={-1} trailingIcon={<ArrowRight aria-hidden="true" />} variant="text">
              {t("components.preview.welcome")}
            </Button>
          )}
          trailing={(
            <ToolbarGroup>
              <ToolbarSeparator />
              <IconButton
                aria-label={t("components.preview.searchLabel")}
                icon={<SearchIcon aria-hidden="true" />}
                size="xs"
                tabIndex={-1}
              />
            </ToolbarGroup>
          )}
        />
      );
    default:
      return null;
  }
}

export function ComponentsPage({
  category,
  colorScheme,
  contrast,
  density,
  onInspectTokens,
  onOpenComponent,
  tokenOverrides,
}: ComponentsPageProps) {
  const { t } = useI18n();
  const isFlowChatCategory = category === "flow-chat";
  const visibleComponents = componentRegistry.filter((component) =>
    isFlowChatCategory
      ? component.category === "flow-chat"
      : component.category !== "flow-chat",
  );
  const catalogComponents = isFlowChatCategory
    ? flowChatPreviewRegistry
      .filter(({ definition }) => definition.section === "framework")
      .map(({ component }) => component)
    : visibleComponents;

  return (
    <main className="lab-page" id={isFlowChatCategory ? "flow-chat" : "components"}>
      <header className="page-heading page-heading--split">
        <div>
          <span className="page-kicker">{t(isFlowChatCategory
            ? "components.flowChat.kicker"
            : "components.kicker")}</span>
          <h1>{t(isFlowChatCategory
            ? "components.flowChat.title"
            : "components.title")}</h1>
          <p>{t(isFlowChatCategory
            ? "components.flowChat.description"
            : "components.description")}</p>
        </div>
        <button className="lab-button" onClick={onInspectTokens} type="button">
          {t("components.inspectAllTokens")}
        </button>
      </header>

      <div className="component-summary-strip" aria-label={t("components.summaryLabel")}>
        <span><strong>{visibleComponents.length}</strong> {t("components.registeredCount")}</span>
        <span><strong>{visibleComponents.reduce((total, item) => total + item.states.length, 0)}</strong> {t("components.statesCount")}</span>
        <span><Check aria-hidden="true" size={15} /> {t("components.accessibilityContracts")}</span>
      </div>

      {isFlowChatCategory && (
        <section className="component-library-section-heading">
          <div>
            <span className="page-kicker">{t("components.flowChat.templatesKicker")}</span>
            <h2>{t("components.flowChat.templatesTitle")}</h2>
          </div>
          <p>{t("components.flowChat.templatesDescription")}</p>
        </section>
      )}

      <ThemeRoot
        className="component-catalog-grid"
        colorScheme={colorScheme}
        contrast={contrast}
        density={density}
        tokenOverrides={tokenOverrides}
      >
        {catalogComponents.map((component) => {
          const Icon = getFlowChatPreviewDefinition(component.name)?.icon
            ?? componentIcons[component.name as keyof typeof componentIcons];
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

      {isFlowChatCategory ? (
        <ThemeRoot
          className="flow-chat-tool-gallery-theme"
          colorScheme={colorScheme}
          contrast={contrast}
          density={density}
          tokenOverrides={tokenOverrides}
        >
          <FlowChatToolGallery onOpenComponent={onOpenComponent} />
        </ThemeRoot>
      ) : (
        <section className="primitive-note">
          <div>
            <span className="page-kicker">{t("components.primitivesKicker")}</span>
            <h2>{t("components.primitivesTitle")}</h2>
          </div>
          <p>{t("components.primitivesDescription")}</p>
        </section>
      )}
    </main>
  );
}
