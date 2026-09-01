import { AppWindow, Heading, Keyboard, List, Rows3, MousePointerClick, PanelTop, ToggleLeft } from "lucide-react";
import {
  ActionCard,
  ActionItem,
  ActivityItem,
  Alert,
  Avatar,
  AvatarGroup,
  Button,
  Card,
  CardHeader,
  ChangeCount,
  Checkbox,
  Combobox,
  Composer,
  ComposerToolbar,
  Disclosure,
  Empty,
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Icon as CatalogIcon,
  IconButton,
  Input,
  KeyHint,
  Listbox,
  ListboxOption,
  LoadingState,
  Menu,
  MenuItem,
  MenuSection,
  MultiSelect,
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelFooter,
  NavigationPanelItem,
  NavigationPanelSection,
  NumberInput,
  NumberBadge,
  PageHeader,
  Radio,
  ScrollArea,
  SearchField,
  SegmentedControl,
  Select,
  Stack,
  StatusPill,
  Spinner,
  Switch,
  TabGroup,
  Textarea,
  ThemeRoot,
  Toolbar,
  ToolbarBadge,
  ToolbarGroup,
  ToolbarSeparator,
  Tooltip,
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
  ActionCard: <MousePointerClick aria-hidden="true" size={19} />,
  ActionItem: <List aria-hidden="true" size={19} />,
  ActivityItem: <CatalogIcon name="terminal" style={{ width: 19, height: 19 }} />,
  Button: <MousePointerClick aria-hidden="true" size={19} />,
  Card: <Rows3 aria-hidden="true" size={19} />,
  Combobox: <CatalogIcon name="search" style={{ width: 19, height: 19 }} />,
  Composer: <CatalogIcon name="arrow-up" style={{ width: 19, height: 19 }} />,
  Field: <Rows3 aria-hidden="true" size={19} />,
  Icon: <CatalogIcon name="search" style={{ width: 19, height: 19 }} />,
  IconButton: <List aria-hidden="true" size={19} />,
  Input: <CatalogIcon name="eye" style={{ width: 19, height: 19 }} />,
  KeyHint: <Keyboard aria-hidden="true" size={19} />,
  Listbox: <List aria-hidden="true" size={19} />,
  LoadingState: <AppWindow aria-hidden="true" size={19} />,
  Menu: <List aria-hidden="true" size={19} />,
  Dialog: <AppWindow aria-hidden="true" size={19} />,
  Sheet: <AppWindow aria-hidden="true" size={19} />,
  MultiSelect: <List aria-hidden="true" size={19} />,
  NavigationPanel: <CatalogIcon name="sidebar-left" style={{ width: 19, height: 19 }} />,
  PageHeader: <Heading aria-hidden="true" size={19} />,
  ScrollArea: <Rows3 aria-hidden="true" size={19} />,
  SearchField: <CatalogIcon name="search" style={{ width: 19, height: 19 }} />,
  SegmentedControl: <ToggleLeft aria-hidden="true" size={19} />,
  Select: <List aria-hidden="true" size={19} />,
  Spinner: <AppWindow aria-hidden="true" size={19} />,
  StatusPill: <CatalogIcon name="check-line" style={{ width: 19, height: 19 }} />,
  Switch: <ToggleLeft aria-hidden="true" size={19} />,
  Disclosure: <CatalogIcon name="chevron-right" style={{ width: 19, height: 19 }} />,
  TabGroup: <PanelTop aria-hidden="true" size={19} />,
  Toolbar: <PanelTop aria-hidden="true" size={19} />,
  Tooltip: <CatalogIcon name="session" style={{ width: 19, height: 19 }} />,
} as const;

function ComponentCardPreview({ component }: { component: ComponentMeta }) {
  const { t } = useI18n();
  if (component.name === "Combobox") return <Combobox label={t("components.preview.modalProviderName")} defaultValue="openbitfun" options={[{ value: "openbitfun", label: "OpenBitFun" }, { value: "custom", label: t("components.preview.add") }]} />;
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
          leading={<CatalogIcon name="session" aria-hidden="true" />}
          tabIndex={-1}
        >
          {t("components.preview.actionCardTitle")}
        </ActionCard>
      );
    case "ActionItem":
      return (
        <ActionItem
          leading={<CatalogIcon name="session" aria-hidden="true" />}
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
          leading={<CatalogIcon name="terminal" aria-hidden="true" />}
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
            leading={<CatalogIcon name="command-mac" size="lg" aria-hidden="true" />}
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
    case "NumberBadge":
      return <NumberBadge value={18} />;
    case "Icon":
      return (
        <Stack align="center" direction="horizontal" gap="3">
          <CatalogIcon name="search" tone="primary" />
          <CatalogIcon name="folder" tone="secondary" />
          <CatalogIcon name="check-circle" tone="success" />
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
          trailing={<CatalogIcon name="eye" aria-hidden="true" />}
        />
      );
    case "KeyHint":
      return <KeyHint icon={<CatalogIcon name="command-mac" size="lg" aria-hidden="true" />}>K</KeyHint>;
    case "Listbox":
      return (
        <Listbox aria-label={t("components.preview.appearance")}>
          <ListboxOption selected value="ask">Ask</ListboxOption>
          <ListboxOption value="plan">Plan</ListboxOption>
        </Listbox>
      );
    case "Menu":
      return (
        <Menu aria-label={t("components.preview.menuLabel")} scrollbarVisibility="hidden">
          <MenuSection title={t("components.preview.menuSectionTitle")}>
            <MenuItem leading={<CatalogIcon name="session" aria-hidden="true" />} tabIndex={-1}>
              {t("components.preview.menuItemOne")}
            </MenuItem>
            <MenuItem leading={<CatalogIcon name="session" aria-hidden="true" />} tabIndex={-1}>
              {t("components.preview.menuItemTwo")}
            </MenuItem>
          </MenuSection>
        </Menu>
      );
    case "FieldGroup":
      return (
        <FormSection
          headingAs="h3"
          leading={<CatalogIcon name="gear" size="lg" aria-hidden="true" />}
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
    case "Alert":
      return (
        <Alert
          message={t("components.preview.fieldDescription")}
          title={t("components.preview.notifications")}
          tone="info"
        />
      );
    case "Avatar":
      return (
        <AvatarGroup maxCount={3}>
          <Avatar>BF</Avatar>
          <Avatar>UI</Avatar>
          <Avatar>DS</Avatar>
          <Avatar>+1</Avatar>
        </AvatarGroup>
      );
    case "Checkbox":
      return (
        <Checkbox
          defaultChecked
          description={t("components.preview.fieldDescription")}
          label={t("components.preview.notifications")}
          tabIndex={-1}
        />
      );
    case "Combobox":
      return (
        <Combobox
          options={[
            { label: "Ask", value: "ask" },
            { label: "Plan", value: "plan" },
          ]}
          aria-label={t("components.preview.appearance")}
          value="ask"
        />
      );
    case "MultiSelect":
      return (
        <MultiSelect
          aria-label={t("components.preview.appearance")}
          options={[
            { label: "Ask", value: "ask" },
            { label: "Plan", value: "plan" },
          ]}
          value={["ask", "plan"]}
        />
      );
    case "NumberInput":
      return <NumberInput onValueChange={() => undefined} value={8} />;
    case "Radio":
      return (
        <Radio
          defaultChecked
          description={t("components.preview.fieldDescription")}
          label={t("components.preview.fieldValue")}
          name="component-preview-radio"
          tabIndex={-1}
        />
      );
    case "Textarea":
      return (
        <Textarea
          aria-label={t("components.preview.inputLabel")}
          defaultValue={t("components.preview.fieldValue")}
          showCount
        />
      );
    case "Disclosure":
      return (
        <Disclosure defaultOpen summary={t("components.preview.appearance")}>
          <span>{t("components.preview.appearanceDescription")}</span>
        </Disclosure>
      );
    case "Empty":
      return (
        <Empty
          description={t("components.preview.cardDescription")}
          title={t("components.preview.cardTitle")}
        />
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
                  icon={<CatalogIcon name="plus" size="lg" aria-hidden="true" />}
                  size="sm"
                  tabIndex={-1}
                  variant="fill"
                />
              )}
              trailing={(
                <IconButton
                  aria-label={t("components.preview.composerSend")}
                  icon={<CatalogIcon name="arrow-up" aria-hidden="true" />}
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
    case "Dialog":
    case "Sheet":
      return (
        <Button
          leadingIcon={<AppWindow aria-hidden="true" />}
          size="sm"
          tabIndex={-1}
        >
          {t("components.preview.openDialog")}
        </Button>
      );
    case "LoadingState":
      return <LoadingState>{t("detail.loading")}</LoadingState>;
    case "Spinner":
      return <Spinner aria-label={t("detail.loading")} size="sm" />;
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
        >
          <NavigationPanelBody scrollbarVisibility="hidden">
            <NavigationPanelContent>
              <NavigationPanelSection title={t("components.preview.navigationPanelSectionTitle")}>
                <NavigationPanelItem leading={<CatalogIcon name="session" aria-hidden="true" />} selected tabIndex={-1}>
                  {t("components.preview.menuItemOne")}
                </NavigationPanelItem>
                <NavigationPanelItem reserveLeadingSpace tabIndex={-1}>
                  {t("components.preview.menuItemTwo")}
                </NavigationPanelItem>
              </NavigationPanelSection>
            </NavigationPanelContent>
          </NavigationPanelBody>
          <NavigationPanelFooter>
            <span>{t("components.preview.navigationPanelDevice")}</span>
          </NavigationPanelFooter>
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
          leadingIcon={<CatalogIcon name="search" aria-hidden="true" />}
          placeholder={t("components.preview.searchPlaceholder")}
          shortcut={<KeyHint icon={<CatalogIcon name="command-mac" size="lg" aria-hidden="true" />}>K</KeyHint>}
        />
      );
    case "StatusPill":
      return (
        <StatusPill leading={<CatalogIcon name="unselected" />}>
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
              icon: <CatalogIcon name="session" aria-hidden="true" />,
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
              icon: <CatalogIcon name="session" aria-hidden="true" />,
              label: t("components.preview.welcome"),
              value: "welcome",
            },
            {
              icon: <CatalogIcon name="session" aria-hidden="true" />,
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
            <Button size="xs" tabIndex={-1} trailingIcon={<CatalogIcon name="arrow-right" size="lg" aria-hidden="true" />} variant="text">
              {t("components.preview.welcome")}
            </Button>
          )}
          trailing={(
            <ToolbarGroup>
              <ToolbarSeparator />
              <IconButton
                aria-label={t("components.preview.searchLabel")}
                icon={<CatalogIcon name="search" aria-hidden="true" />}
                size="xs"
                tabIndex={-1}
              />
            </ToolbarGroup>
          )}
        />
      );
    case "Tooltip":
      return (
        <Tooltip content={t("components.preview.tooltipContent")} delay={0}>
          <Button size="sm" tabIndex={-1} variant="fill">
            {t("components.preview.tooltipTrigger")}
          </Button>
        </Tooltip>
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
        <span><CatalogIcon name="check-line" aria-hidden="true" style={{ width: 15, height: 15 }} /> {t("components.accessibilityContracts")}</span>
      </div>

      {isFlowChatCategory && (
        <section className="component-catalog-section-heading">
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
          const FlowIcon = getFlowChatPreviewDefinition(component.name)?.icon;
          const glyph = FlowIcon ? <FlowIcon aria-hidden="true" size={19} />
            : componentIcons[component.name as keyof typeof componentIcons];
          return (
            <button
              className="component-card"
              key={component.name}
              onClick={() => onOpenComponent(component.name)}
              type="button"
            >
              <span className="component-card__topline">
                <span className="component-card__icon">
                  {glyph ?? null}
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
                <CatalogIcon name="arrow-right" size="md" aria-hidden="true" />
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
