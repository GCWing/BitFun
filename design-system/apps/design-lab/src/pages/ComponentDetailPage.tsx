import { Fragment, useMemo, useState } from "react";
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  Clipboard,
  Command,
  Copy,
  Download,
  Eye,
  ExternalLink,
  Info,
  List,
  MessageCircle,
  Mic,
  Monitor,
  MoreHorizontal,
  Plus,
  Search as SearchIcon,
  Settings,
  Terminal,
  X,
} from "lucide-react";
import {
  ActionCard,
  ActionItem,
  ActivityItem,
  Alert,
  Avatar,
  AvatarGroup,
  Button,
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  CardMedia,
  ChangeCount,
  Checkbox,
  Composer,
  Combobox,
  ComposerContextBar,
  ComposerDivider,
  ComposerToolbar,
  ConfirmDialog,
  Disclosure,
  Empty,
  Field,
  FieldGroup,
  FieldRow,
  FormSection,
  Icon,
  IconButton,
  iconNames,
  Input,
  KeyHint,
  Menu,
  MenuItem,
  MenuSection,
  MenuSeparator,
  Modal,
  NavigationPanel,
  NavigationPanelItem,
  NavigationPanelSection,
  NavigationPanelSeparator,
  NumberInput,
  PageHeader,
  Radio,
  ScrollArea,
  SearchField,
  SegmentedControl,
  Select,
  StatusPill,
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
  type ConfirmDialogType,
  type ContrastMode,
  type DensityMode,
  type IconName,
  type IconSize,
  type IconTone,
  type ActivityItemAppearance,
  type ActionCardSize,
  type CardContentAlignment,
  type ScrollAreaOrientation,
  type ScrollbarVisibility,
  type StatusPillTone,
  type ToolbarSize,
  type TokenOverrides,
} from "@bitfun/ui";
import type { ComponentMeta } from "@bitfun/ui/registry";
import previewImage from "../assets/design-system-hero.webp";
import { IconCompositionPreview } from "../preview/IconCompositionPreview";
import { NestedMenuPattern } from "./ReferencePatterns";
import { useI18n, type MessageKey } from "../i18n";
import {
  getComponentCategoryLabel,
  getComponentDescription,
} from "../i18n/componentMetadata";
import {
  FlowChatComponentPreview,
  getFlowChatPreviewDefinition,
} from "../preview/FlowChatPreviewRegistry";

interface ComponentDetailPageProps {
  colorScheme: ColorScheme;
  component: ComponentMeta;
  contrast: ContrastMode;
  density: DensityMode;
  onBack: () => void;
  onInspectTokens: (name: string) => void;
  tokenOverrides: TokenOverrides;
}

type CopyStatus = "idle" | "copied" | "unavailable";
type InspectorTab = "properties" | "styles" | "tokens";
type PreviewIcon = "chevron" | "none";
type PreviewIconPosition = "left" | "right";
type PreviewSize = "sm" | "md" | "lg";
type FieldOrientation = "horizontal" | "vertical";
type PageHeaderAlign = "center" | "start";
type PageHeaderSize = "display" | "lg" | "md" | "sm";

const buttonVariants = ["outline", "fill", "primary", "text"] as const;
const iconButtonVariants = ["quiet", "fill", "primary"] as const;
const buttonInspectorStates = ["default", "hover", "active"] as const;
const fieldOrientations = ["vertical", "horizontal"] as const;
const pageHeaderAlignments = ["start", "center"] as const;
const cardContentAlignments = ["start", "center", "end"] as const;
const pageHeaderSizes = ["sm", "md", "lg", "display"] as const;
const scrollAreaOrientations = ["vertical", "horizontal", "both"] as const;
const activityItemAppearances = ["inline", "surface"] as const;
const actionCardSizes = ["sm", "md"] as const;
const iconSizes = ["2xs", "xs", "sm", "md", "lg"] as const;
const iconTones = ["inherit", "primary", "secondary", "muted", "disabled", "info", "success", "warning", "danger"] as const;

const optionLabelKeys: Readonly<Record<string, MessageKey>> = {
  active: "detail.option.active",
  always: "detail.option.always",
  asking: "detail.option.asking",
  auto: "detail.option.auto",
  both: "detail.option.both",
  chevron: "detail.option.chevron",
  center: "detail.option.center",
  default: "detail.option.default",
  disabled: "detail.option.disabled",
  display: "detail.option.display",
  error: "detail.option.error",
  expanded: "detail.option.expanded",
  fill: "detail.option.fill",
  "focus-visible": "detail.option.focus-visible",
  hover: "detail.option.hover",
  info: "detail.option.info",
  horizontal: "detail.option.horizontal",
  hidden: "detail.option.hidden",
  inline: "detail.option.inline",
  scrolling: "detail.option.scrolling",
  "focus-within": "detail.option.focus-within",
  "with-context": "detail.option.with-context",
  "with-center": "detail.option.with-center",
  overflow: "detail.option.overflow",
  "disabled-item": "detail.option.disabled-item",
  "selected-item": "detail.option.selected-item",
  "checked-item": "detail.option.checked-item",
  invalid: "detail.option.invalid",
  left: "detail.option.left",
  lg: "detail.option.lg",
  loading: "detail.option.loading",
  multiple: "detail.option.multiple",
  searching: "detail.option.searching",
  custom: "detail.option.custom",
  empty: "detail.option.empty",
  pending: "detail.option.pending",
  plain: "detail.option.plain",
  divided: "detail.option.divided",
  md: "detail.option.md",
  none: "detail.option.none",
  off: "detail.option.off",
  on: "detail.option.on",
  outline: "detail.option.outline",
  primary: "detail.option.primary",
  quiet: "detail.option.quiet",
  raised: "detail.option.raised",
  right: "detail.option.right",
  confirmation: "detail.option.confirmation",
  completed: "detail.option.completed",
  selected: "detail.option.selected",
  sm: "detail.option.sm",
  start: "detail.option.start",
  surface: "detail.option.surface",
  subtle: "detail.option.subtle",
  submitting: "detail.option.submitting",
  success: "detail.option.success",
  text: "detail.option.text",
  media: "detail.option.media",
  unselected: "detail.option.unselected",
  vertical: "detail.option.vertical",
  warning: "detail.option.warning",
};

function InspectorSelect({
  label,
  onChange,
  options,
  translateOptions = true,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: readonly string[];
  translateOptions?: boolean;
  value: string;
}) {
  const { t } = useI18n();

  return (
    <label className="component-inspector-select">
      <span>{label}</span>
      <select onChange={(event) => onChange(event.target.value)} value={value}>
        {options.map((option) => (
          <option key={option} value={option}>
            {translateOptions && optionLabelKeys[option] ? t(optionLabelKeys[option]) : option}
          </option>
        ))}
      </select>
    </label>
  );
}

function InspectorToggle({
  checked,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  label: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <label className="component-inspector-toggle">
      <span>{label}</span>
      <Switch
        aria-label={label}
        checked={checked}
        onCheckedChange={onCheckedChange}
      />
    </label>
  );
}

function NumberInputPreview({ state }: { state: string }) {
  const { t } = useI18n();
  const [value, setValue] = useState(8);
  return (
    <NumberInput
      aria-label={t("components.preview.inputLabel")}
      className={`component-number-input-example lab-state-${state}`}
      disabled={state === "disabled"}
      onChange={setValue}
      value={value}
    />
  );
}

export function ComponentDetailPage({
  colorScheme,
  component,
  contrast,
  density,
  onBack,
  onInspectTokens,
  tokenOverrides,
}: ComponentDetailPageProps) {
  const { t } = useI18n();
  const stateLabel = (state: string) => optionLabelKeys[state] ? t(optionLabelKeys[state]) : state;
  const [variant, setVariant] = useState<(typeof buttonVariants)[number]>("fill");
  const [iconButtonVariant, setIconButtonVariant] = useState<(typeof iconButtonVariants)[number]>("quiet");
  const [iconName, setIconName] = useState<IconName>("search");
  const [iconSize, setIconSize] = useState<IconSize>("lg");
  const [iconTone, setIconTone] = useState<IconTone>("inherit");
  const [selectValue, setSelectValue] = useState<string>("ask");
  const [size, setSize] = useState<PreviewSize>("md");
  const [fieldOrientation, setFieldOrientation] = useState<FieldOrientation>("horizontal");
  const [fieldShowLabelAction, setFieldShowLabelAction] = useState(false);
  const [fieldShowControlLeading, setFieldShowControlLeading] = useState(false);
  const [fieldShowControlTrailing, setFieldShowControlTrailing] = useState(false);
  const [pageHeaderAlign, setPageHeaderAlign] = useState<PageHeaderAlign>("start");
  const [cardContentAlign, setCardContentAlign] = useState<CardContentAlignment>("start");
  const [pageHeaderSize, setPageHeaderSize] = useState<PageHeaderSize>("lg");
  const [scrollAreaOrientation, setScrollAreaOrientation] = useState<ScrollAreaOrientation>("vertical");
  const [activityItemAppearance, setActivityItemAppearance] = useState<ActivityItemAppearance>("surface");
  const [activityShowDetail, setActivityShowDetail] = useState(false);
  const [pageHeaderRequired, setPageHeaderRequired] = useState(false);
  const [actionItemShowMetadata, setActionItemShowMetadata] = useState(false);
  const [actionCardSize, setActionCardSize] = useState<ActionCardSize>("sm");
  const [toolbarSize, setToolbarSize] = useState<ToolbarSize>("sm");
  const [previewState, setPreviewState] = useState(
    component.name === "Card"
      ? "raised"
      : component.name === "Disclosure"
        ? "open"
      : component.name === "Composer"
      ? "with-context"
      : component.name === "Switch"
        ? "off"
      : component.name === "StatusPill"
        ? "success"
      : component.name === "Toolbar"
        ? "with-center"
      : component.name === "TabGroup" || component.name === "SegmentedControl"
        ? "selected"
        : component.name === "ScrollArea"
          ? "auto"
        : component.name === "Tooltip"
          ? "top"
        : component.states[0] ?? "default",
  );
  const [inspectorDisabled, setInspectorDisabled] = useState(false);
  const [inspectorLoading, setInspectorLoading] = useState(false);
  const [previewIcon, setPreviewIcon] = useState<PreviewIcon>("none");
  const [previewIconPosition, setPreviewIconPosition] = useState<PreviewIconPosition>("left");
  const [copyStatus, setCopyStatus] = useState<CopyStatus>("idle");
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>("properties");
  const [modalOpen, setModalOpen] = useState(false);
  const [modalShowScrollbar, setModalShowScrollbar] = useState(true);
  const [menuShowScrollbar, setMenuShowScrollbar] = useState(true);
  const [navigationPanelShowScrollbar, setNavigationPanelShowScrollbar] = useState(true);
  const [composerShowContext, setComposerShowContext] = useState(false);
  const [composerShowToolbar, setComposerShowToolbar] = useState(true);
  const flowChatPreview = getFlowChatPreviewDefinition(component.name);
  const isFlowChatComponent = Boolean(flowChatPreview);

  const states = useMemo(() => {
    if (flowChatPreview) {
      return component.states;
    }

    switch (component.name) {
      case "ActionCard":
        return ["default", "hover", "active", "focus-visible", "selected", "disabled"] as const;
      case "ActionItem":
        return ["default", "hover", "active", "disabled"] as const;
      case "ActivityItem":
        return ["default", "hover", "active", "focus-visible", "disabled"] as const;
      case "Button":
      case "IconButton":
        return ["default", "hover", "active", "disabled"] as const;
      case "Composer":
        return ["default", "focus-within", "with-context", "invalid", "disabled"] as const;
      case "ConfirmDialog":
        return ["info", "warning", "error", "success", "pending"] as const;
      case "Card":
        return ["raised", "subtle", "media"] as const;
      case "Input":
      case "SearchField":
      case "Select":
        return ["default", "hover", "focus-visible", "invalid", "disabled"] as const;
      case "Field":
      case "Icon":
      case "KeyHint":
      case "Modal":
      case "PageHeader":
        return ["default"] as const;
      case "FieldGroup":
        return ["subtle", "plain", "divided"] as const;
      case "Disclosure":
        return ["closed", "open", "hover", "focus-visible", "disabled"] as const;
      case "Menu":
        return ["default", "scrolling", "focus-within", "disabled-item", "checked-item"] as const;
      case "NavigationPanel":
        return ["default", "selected-item", "disabled-item", "scrolling"] as const;
      case "ScrollArea":
        return ["auto", "always", "hidden"] as const;
      case "StatusPill":
        return ["neutral", "info", "success", "warning", "danger"] as const;
      case "SegmentedControl":
      case "TabGroup":
        return ["selected", "unselected", "hover", "disabled"] as const;
      case "Toolbar":
        return ["default", "with-center", "overflow"] as const;
      case "Tooltip":
        return ["top", "bottom", "left", "right"] as const;
      case "Combobox":
        return component.states;
      case "Switch":
        return ["off", "on", "focus-visible", "disabled"] as const;
      default:
        return component.states;
    }
  }, [component.name, component.states, flowChatPreview]);
  const inspectorStates = component.name === "Button" || component.name === "IconButton"
    ? buttonInspectorStates
    : states;

  const codeSample = useMemo(() => {
    if (component.name === "Textarea") return `import { Textarea } from "@bitfun/ui";\n\n<Textarea\n  label="${t("components.preview.inputLabel")}"\n  defaultValue="${t("components.preview.fieldValue")}"\n  hint="${t("components.preview.fieldDescription")}"\n  maxLength={200}\n  rows={3}\n  showCount\n/>`;
    if (component.name === "Alert") return `import { Alert } from "@bitfun/ui";\n\n<Alert tone="info" title="${t("components.preview.notifications")}" message="${t("components.preview.fieldDescription")}" />`;
    if (component.name === "Avatar") return 'import { Avatar } from "@bitfun/ui";\n\n<Avatar>BF</Avatar>';
    if (component.name === "Checkbox" || component.name === "Radio") return `import { ${component.name} } from "@bitfun/ui";\n\n<${component.name} label="${t("components.preview.notifications")}" defaultChecked />`;
    if (component.name === "NumberInput") return 'import { useState } from "react";\nimport { NumberInput } from "@bitfun/ui";\n\nfunction Example() {\n  const [value, setValue] = useState(8);\n  return <NumberInput value={value} onChange={setValue} />;\n}';
    if (component.name === "Empty") return `import { Empty } from "@bitfun/ui";\n\n<Empty title="${t("components.preview.cardTitle")}" description="${t("components.preview.cardDescription")}" />`;
    if (component.name === "Combobox") return 'import { Combobox } from "@bitfun/ui";\n\n<Combobox label="Models" multiple searchable allowCustomValue options={[{ label: "OpenBitFun", value: "openbitfun" }]} />';
    if (flowChatPreview) {
      return flowChatPreview.codeSample(t);
    }

    if (component.name === "ActionCard") {
      return `import { ActionCard } from "@bitfun/ui";\nimport { MessageCircle, MoreHorizontal } from "lucide-react";\n\n<ActionCard\n  actions={[\n    { id: "more", icon: <MoreHorizontal />, label: "${t("components.preview.more")}" },\n  ]}\n  description="${t("components.preview.actionCardDescription")}"\n  leading={<MessageCircle />}\n  size="${actionCardSize}"\n>\n  ${t("components.preview.actionCardTitle")}\n</ActionCard>`;
    }
    if (component.name === "ActionItem") {
      const metadataProp = actionItemShowMetadata ? `\n  metadata="12"` : "";
      return `import { ActionItem, KeyHint } from "@bitfun/ui";\nimport { MessageCircle, MoreHorizontal, Plus } from "lucide-react";\n\n<ActionItem\n  actions={[\n    { id: "add", icon: <Plus />, label: "${t("components.preview.add")}" },\n    { id: "more", icon: <MoreHorizontal />, label: "${t("components.preview.more")}" },\n  ]}\n  leading={<MessageCircle />}${metadataProp}\n  shortcut={<KeyHint>K</KeyHint>}\n>\n  ${t("components.preview.assistant")}\n</ActionItem>`;
    }
    if (component.name === "ActivityItem") {
      if (activityItemAppearance === "inline") {
        return `import { ActivityItem } from "@bitfun/ui";\nimport { Check } from "lucide-react";\n\n<ActivityItem\n  appearance="inline"\n  leading={<Check />}\n>\n  ${t("components.preview.activityStatus")}\n</ActivityItem>`;
      }
      const detailProp = activityShowDetail
        ? `\n  detail={<code>${t("components.preview.activityDetail")}</code>}`
        : "";
      return `import { ActivityItem, ChangeCount } from "@bitfun/ui";\nimport { Copy, Download, ExternalLink, Terminal } from "lucide-react";\n\n<ActivityItem\n  actions={[\n    { id: "copy", icon: <Copy />, label: "${t("components.preview.activityCopy")}" },\n    { id: "download", icon: <Download />, label: "${t("components.preview.activityDownload")}" },\n    { id: "open", icon: <ExternalLink />, label: "${t("components.preview.activityOpen")}" },\n  ]}\n  appearance="surface"${detailProp}\n  label="${t("components.preview.activityAction")}"\n  leading={<Terminal />}\n  metadata={<ChangeCount additions={6} deletions={0} />}\n  onActivate={() => openActivity()}\n>\n  ${t("components.preview.activityDescription")}\n</ActivityItem>`;
    }
    if (component.name === "Button") {
      const stateProps = `${inspectorDisabled ? " disabled" : ""}${inspectorLoading ? " loading" : ""}`;
      const iconImport = previewIcon === "chevron"
        ? "\nimport { ChevronRight } from \"lucide-react\";"
        : "";
      const iconProp = previewIcon === "chevron"
        ? ` ${previewIconPosition === "left" ? "leadingIcon" : "trailingIcon"}={<ChevronRight />}`
        : "";
      return `import { Button } from "@bitfun/ui";${iconImport}\n\n<Button variant="${variant}" size="${size}"${stateProps}${iconProp}>\n  ${t("components.preview.session")}\n</Button>`;
    }
    if (component.name === "Card") {
      if (previewState === "media") {
        return `import { Card, CardBody, CardHeader, CardMedia } from "@bitfun/ui";\n\n<Card appearance="neutral" clip radius="md">\n  <CardMedia>\n    <ProductArtwork />\n  </CardMedia>\n  <CardBody align="center" padding="sm">\n    <CardHeader\n      contentAlign="center"\n      title="${t("components.preview.cardMediaTitle")}"\n      description="${t("components.preview.cardMediaDescription")}"\n    />\n  </CardBody>\n</Card>`;
      }
      return `import { Card, CardBody, CardFooter, CardHeader } from "@bitfun/ui";\n\n<Card appearance="${previewState}" gap="md" padding="md" radius="lg">\n  <CardHeader\n    contentAlign="${cardContentAlign}"\n    title="${t("components.preview.cardTitle")}"\n    description="${t("components.preview.cardDescription")}"\n  />\n  <CardBody>\n    <CommandGrid />\n  </CardBody>\n  <CardFooter align="end">\n    <Button>${t("components.preview.settings")}</Button>\n  </CardFooter>\n</Card>`;
    }
    if (component.name === "Composer") {
      const stateProps = `${previewState === "disabled" ? " disabled" : ""}${previewState === "invalid" ? " invalid" : ""}`;
      const contextProp = composerShowContext || previewState === "with-context"
        ? `\n  contextBar={<ComposerContextBar\n    leading={<><span>${t("components.preview.composerDevice")}</span><ComposerDivider /><span>${t("components.preview.composerWorkspace")}</span></>}\n    trailing={<span>${t("components.preview.composerMode")}</span>}\n  />}`
        : "";
      const toolbarProp = composerShowToolbar
        ? `\n  toolbar={<ComposerToolbar\n    leading={<IconButton aria-label="${t("components.preview.composerAdd")}" icon={<Plus />} />}\n    trailing={<><Button variant="text">${t("components.preview.composerModel")}</Button><IconButton aria-label="${t("components.preview.composerSend")}" icon={<ArrowUp />} variant="primary" /></>}\n  />}`
        : "";
      return `import { Button, Composer, ComposerContextBar, ComposerDivider, ComposerToolbar, IconButton } from "@bitfun/ui";\nimport { ArrowUp, Plus } from "lucide-react";\n\n<Composer\n  aria-label="${t("components.preview.composerLabel")}"${contextProp}${toolbarProp}${stateProps}\n>\n  <textarea\n    aria-label="${t("components.preview.composerEditorLabel")}"\n    placeholder="${t("components.preview.composerPlaceholder")}"\n  />\n</Composer>`;
    }
    if (component.name === "ConfirmDialog") {
      return `import { ConfirmDialog } from "@bitfun/ui";\n\n<ConfirmDialog\n  cancelText="${t("components.preview.modalCancel")}"\n  confirmDanger\n  confirmText="${t("components.preview.confirmDelete")}"\n  isOpen={open}\n  message="${t("components.preview.confirmMessage")}"\n  onClose={() => setOpen(false)}\n  onConfirm={() => deleteItem()}\n  preview="/workspace/project"\n  title="${t("components.preview.confirmTitle")}"\n  type="error"\n/>`;
    }
    if (component.name === "Icon") {
      return `import { Icon } from "@bitfun/ui";\n\n<Icon name="${iconName}" size="${iconSize}" tone="${iconTone}" />`;
    }
    if (component.name === "IconButton") {
      const stateProps = `${inspectorDisabled ? " disabled" : ""}${inspectorLoading ? " loading" : ""}`;
      return `import { IconButton } from "@bitfun/ui";\nimport { List } from "lucide-react";\n\n<IconButton\n  aria-label="${t("components.preview.listView")}"\n  icon={<List />}\n  variant="${iconButtonVariant}"${stateProps}\n/>`;
    }
    if (component.name === "Field") {
      const labelAction = fieldShowLabelAction
        ? `\n  labelAction={<IconButton aria-label="${t("components.preview.fieldHelp")}" icon={<Info />} size="xs" />}`
        : "";
      const controlLeading = fieldShowControlLeading
        ? `\n  controlLeading={<Switch aria-label="${t("components.preview.notifications")}" />}`
        : "";
      const controlTrailing = fieldShowControlTrailing
        ? `\n  controlTrailing={<IconButton aria-label="${t("components.preview.more")}" icon={<MoreHorizontal />} size="xs" />}`
        : "";
      return `import { Field, IconButton, Input, Switch } from "@bitfun/ui";\nimport { ChevronDown, Info, MoreHorizontal } from "lucide-react";\n\n<Field\n  description="${t("components.preview.fieldDescription")}"\n  label="${t("components.preview.appearance")}"${labelAction}${controlLeading}${controlTrailing}\n  orientation="${fieldOrientation}"\n  required\n>\n  <Input defaultValue="${t("components.preview.fieldValue")}" trailing={<ChevronDown />} />\n</Field>`;
    }
    if (component.name === "Input") {
      const stateProps = previewState === "disabled"
        ? " disabled"
        : previewState === "invalid"
          ? " invalid"
          : "";
      return `import { Input } from "@bitfun/ui";\nimport { Eye } from "lucide-react";\n\n<Input\n  aria-label="${t("components.preview.inputLabel")}"\n  placeholder="${t("components.preview.inputPlaceholder")}"\n  trailing={<Eye />}${stateProps}\n/>`;
    }
    if (component.name === "KeyHint") {
      return `import { KeyHint } from "@bitfun/ui";\nimport { Command } from "lucide-react";\n\n<KeyHint icon={<Command />}>K</KeyHint>`;
    }
    if (component.name === "FieldGroup") {
      return `import { Field, FieldGroup, FieldRow, FormSection, Input } from "@bitfun/ui";\nimport { Settings } from "lucide-react";\n\n<FormSection\n  description="${t("components.preview.fieldDescription")}"\n  headingAs="h3"\n  leading={<Settings />}\n  title="${t("components.preview.modalSectionTitle")}"\n>\n  <FieldGroup appearance="subtle" dividers>\n    <FieldRow>\n      <Field controlWidth="fill" label="${t("components.preview.modalProviderName")}" labelWidth="md" orientation="horizontal" required>\n        <Input defaultValue="OpenBitFun" />\n      </Field>\n    </FieldRow>\n    <FieldRow>\n      <Field controlWidth="fill" label="${t("components.preview.modalApiUrl")}" labelWidth="md" orientation="horizontal">\n        <Input defaultValue="https://api.openbitfun.com" />\n      </Field>\n    </FieldRow>\n  </FieldGroup>\n</FormSection>`;
    }
    if (component.name === "Tooltip") {
      return `import { Tooltip } from "@bitfun/ui";\n\n<Tooltip\n  content="${t("components.preview.tooltipContent")}"\n  placement="${previewState}"\n>\n  <Button>${t("components.preview.tooltipTrigger")}</Button>\n</Tooltip>`;
    }
    if (component.name === "Menu") {
      return `import { Menu, MenuItem, MenuSection, MenuSeparator } from "@bitfun/ui";\nimport { MessageCircle } from "lucide-react";\n\n<Menu\n  aria-label="${t("components.preview.menuLabel")}"\n  scrollbarVisibility="${menuShowScrollbar ? "auto" : "hidden"}"\n>\n  <MenuSection title="${t("components.preview.menuSectionTitle")}">\n    <MenuItem leading={<MessageCircle />}>${t("components.preview.menuItemOne")}</MenuItem>\n    <MenuItem leading={<MessageCircle />}>${t("components.preview.menuItemTwo")}</MenuItem>\n  </MenuSection>\n  <MenuSeparator />\n  <MenuSection aria-label="${t("components.preview.menuMoreSection")}">\n    <MenuItem disabled>${t("components.preview.menuDisabledItem")}</MenuItem>\n  </MenuSection>\n</Menu>`;
    }
    if (component.name === "Modal") {
      return `import { Button, Modal } from "@bitfun/ui";\n\n<Modal\n  contentPadding="lg"\n  footer={<>\n    <Button onClick={() => setOpen(false)} variant="fill">${t("components.preview.modalCancel")}</Button>\n    <Button onClick={() => setOpen(false)} variant="primary">${t("components.preview.modalSave")}</Button>\n  </>}\n  isOpen={open}\n  onClose={() => setOpen(false)}\n  showScrollbar={${modalShowScrollbar}}\n  size="xxlarge"\n  title="${t("components.preview.modalTitle")}"\n>\n  <ProviderConfigurationFields />\n</Modal>`;
    }
    if (component.name === "PageHeader") {
      const requiredProp = pageHeaderRequired ? "\n  required" : "";
      return `import { IconButton, PageHeader } from "@bitfun/ui";\nimport { Settings, X } from "lucide-react";\n\n<PageHeader\n  action={<IconButton aria-label="${t("components.preview.close")}" icon={<X />} />}\n  align="${pageHeaderAlign}"\n  description="${t("components.preview.appearanceDescription")}"\n  leading={<Settings />}\n  level={2}${requiredProp}\n  size="${pageHeaderSize}"\n  title="${t("components.preview.appearance")}"\n/>`;
    }
    if (component.name === "SearchField") {
      const stateProps = previewState === "disabled"
        ? " disabled"
        : previewState === "invalid"
          ? " invalid"
          : "";
      return `import { KeyHint, SearchField } from "@bitfun/ui";\nimport { Command, Search } from "lucide-react";\n\n<SearchField\n  aria-label="${t("components.preview.searchLabel")}"\n  leadingIcon={<Search />}\n  placeholder="${t("components.preview.searchPlaceholder")}"\n  shortcut={<KeyHint icon={<Command />}>K</KeyHint>}${stateProps}\n/>`;
    }
    if (component.name === "Select") {
      return `import { Icon, Select } from "@bitfun/ui";\n\n<Select\n  aria-label="Mode"\n  leading={<Icon name="circle" />}\n  onValueChange={setMode}\n  options={[\n    { label: "Ask", value: "ask" },\n    { label: "Plan", value: "plan" },\n    { disabled: true, label: "Agent", value: "agent" },\n  ]}\n  value="${selectValue}"\n/>`;
    }
    if (component.name === "SegmentedControl") {
      const defaultMode = previewState === "unselected" ? "agent" : "chat";
      return `import { SegmentedControl } from "@bitfun/ui";\nimport { MessageCircle } from "lucide-react";\n\n<SegmentedControl\n  aria-label="${t("components.preview.segmentedLabel")}"\n  defaultValue="${defaultMode}"\n  onValueChange={setMode}\n  options={[\n    { icon: <MessageCircle />, label: "${t("components.preview.segmentedChat")}", value: "chat" },\n    { label: "${t("components.preview.segmentedAgent")}", value: "agent" },\n  ]}\n/>`;
    }
    if (component.name === "StatusPill") {
      return `import { Icon, StatusPill } from "@bitfun/ui";\n\n<StatusPill leading={<Icon name="circle" />} tone="${previewState}">\n  Ask\n</StatusPill>`;
    }
    if (component.name === "Disclosure") {
      const stateProps = previewState === "open" ? " defaultOpen" : previewState === "disabled" ? " disabled" : "";
      return `import { Disclosure } from "@bitfun/ui";\n\n<Disclosure summary="${t("components.preview.appearance")}"${stateProps}>\n  ${t("components.preview.appearanceDescription")}\n</Disclosure>`;
    }
    if (component.name === "NavigationPanel") {
      return `import { IconButton, NavigationPanel, NavigationPanelItem, NavigationPanelSection, SearchField } from "@bitfun/ui";\nimport { Monitor, Search, Settings } from "lucide-react";\n\n<NavigationPanel\n  aria-label="${t("components.preview.navigationPanelLabel")}"\n  footer={<>\n    <NavigationPanelItem leading={<Monitor />}>${t("components.preview.navigationPanelDevice")}</NavigationPanelItem>\n    <IconButton aria-label="${t("components.preview.settings")}" icon={<Settings />} />\n  </>}\n  header={<SearchField aria-label="${t("components.preview.searchLabel")}" leadingIcon={<Search />} />}\n  scrollbarVisibility="${navigationPanelShowScrollbar ? "auto" : "hidden"}"\n>\n  <NavigationPanelSection title="${t("components.preview.navigationPanelSectionTitle")}">\n    <NavigationPanelItem selected>${t("components.preview.menuItemOne")}</NavigationPanelItem>\n    <NavigationPanelItem>${t("components.preview.menuItemTwo")}</NavigationPanelItem>\n  </NavigationPanelSection>\n</NavigationPanel>`;
    }
    if (component.name === "ScrollArea") {
      return `import { ScrollArea } from "@bitfun/ui";\n\n<ScrollArea\n  aria-label="${t("components.preview.scrollAreaLabel")}"\n  className="activity-scroll-area"\n  orientation="${scrollAreaOrientation}"\n  scrollbarVisibility="${previewState}"\n>\n  {items.map((item) => <div key={item.id}>{item.label}</div>)}\n</ScrollArea>`;
    }
    if (component.name === "TabGroup") {
      const defaultTab = previewState === "unselected" ? "settings" : "welcome";
      return `import { TabGroup } from "@bitfun/ui";\nimport { MessageCircle } from "lucide-react";\n\nconst items = [\n  { icon: <MessageCircle />, label: "${t("components.preview.welcome")}", value: "welcome" },\n  { icon: <MessageCircle />, label: "${t("components.preview.settings")}", value: "settings" },\n];\n\n<TabGroup\n  aria-label="${t("components.preview.tabGroupLabel")}"\n  defaultValue="${defaultTab}"\n  items={items}\n/>`;
    }
    if (component.name === "Toolbar") {
      return `import { ChangeCount, IconButton, TabGroup, Toolbar, ToolbarBadge, ToolbarGroup, ToolbarSeparator } from "@bitfun/ui";\nimport { MoreHorizontal, Search } from "lucide-react";\n\nconst items = [\n  { label: "${t("components.preview.welcome")}", value: "welcome" },\n  { label: "${t("components.preview.settings")}", value: "settings" },\n];\n\n<Toolbar\n  aria-label="${t("components.preview.tabGroupLabel")}"\n  center={<ToolbarGroup>\n    <ToolbarBadge>18</ToolbarBadge>\n    <strong>${t("components.preview.session")}</strong>\n  </ToolbarGroup>}\n  leading={<TabGroup defaultValue="welcome" items={items} />}\n  size="${toolbarSize}"\n  trailing={<ToolbarGroup>\n    <ChangeCount additions={6} deletions={0} />\n    <ToolbarSeparator />\n    <IconButton aria-label="${t("components.preview.searchLabel")}" icon={<Search />} size="xs" />\n    <IconButton aria-label="${t("components.preview.more")}" icon={<MoreHorizontal />} size="xs" />\n  </ToolbarGroup>}\n/>`;
    }
    if (component.name !== "Switch") return `// ${t("detail.previewUnavailable")}: ${component.name}`;
    const stateProps = previewState === "on"
      ? " defaultChecked"
      : previewState === "disabled"
        ? " disabled"
        : "";
    return `import { Switch } from "@bitfun/ui";\n\n<Switch\n  aria-label="${t("components.preview.notifications")}"${stateProps}\n/>`;
  }, [
    actionItemShowMetadata,
    activityItemAppearance,
    activityShowDetail,
    component.name,
    composerShowContext,
    composerShowToolbar,
    fieldOrientation,
    fieldShowControlLeading,
    fieldShowControlTrailing,
    fieldShowLabelAction,
    flowChatPreview,
    iconButtonVariant,
    iconName,
    iconSize,
    iconTone,
    inspectorDisabled,
    inspectorLoading,
    menuShowScrollbar,
    modalShowScrollbar,
    navigationPanelShowScrollbar,
    pageHeaderAlign,
    pageHeaderRequired,
    pageHeaderSize,
    previewIcon,
    previewIconPosition,
    previewState,
    scrollAreaOrientation,
    selectValue,
    size,
    t,
    toolbarSize,
    variant,
  ]);

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(codeSample);
      setCopyStatus("copied");
      window.setTimeout(() => setCopyStatus("idle"), 1400);
    } catch {
      setCopyStatus("unavailable");
    }
  }

  function renderModalConfigurationContent() {
    return (
      <FormSection
        aria-label={t("components.preview.modalSectionTitle")}
        className="component-modal-example"
        headingAs="h3"
        title={t("components.preview.modalSectionTitle")}
      >
        <FieldGroup>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalProviderName")}
              labelWidth="md"
              orientation="horizontal"
              required
            >
              <Input defaultValue="OpenBitFun" />
            </Field>
          </FieldRow>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalAuthentication")}
              labelWidth="md"
              orientation="horizontal"
              required
            >
              <Input
                defaultValue="API Key"
                readOnly
                trailing={<ChevronDown aria-hidden="true" />}
              />
            </Field>
          </FieldRow>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalApiKey")}
              labelWidth="md"
              orientation="horizontal"
              required
            >
              <Input
                defaultValue="bitfun-provider-api-key"
                readOnly
                trailing={<Eye aria-hidden="true" />}
                type="password"
              />
            </Field>
          </FieldRow>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalApiUrl")}
              labelWidth="md"
              orientation="horizontal"
            >
              <Input
                defaultValue="https://api.openbitfun.com"
              />
            </Field>
          </FieldRow>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalRequestFormat")}
              labelWidth="md"
              orientation="horizontal"
            >
              <Input
                defaultValue="Anthropic (messages)"
                readOnly
                trailing={<ChevronDown aria-hidden="true" />}
              />
            </Field>
          </FieldRow>
          <FieldRow>
            <Field
              controlWidth="fill"
              horizontalGap="lg"
              label={t("components.preview.modalSelectModels")}
              labelWidth="md"
              orientation="horizontal"
              required
            >
              <Input
                defaultValue="k3-256k"
                trailing={<Plus aria-hidden="true" />}
              />
            </Field>
          </FieldRow>
        </FieldGroup>
        <p className="component-modal-example__hint">{t("components.preview.modalPresetModels")}</p>
        <div className="component-modal-example__model-card">
          <strong>k3-256k</strong>
          <span>{t("components.preview.modalModelSummary")}</span>
        </div>
      </FormSection>
    );
  }

  function renderModalExample(interactive: boolean) {
    const closePreview = () => {
      if (interactive) setModalOpen(false);
    };

    return (
      <Modal
        autoFocus={interactive}
        closeOnEscape={interactive}
        closeOnOverlayClick={interactive}
        contentPadding="lg"
        dialogClassName={interactive ? undefined : "component-modal-preview-dialog"}
        footer={(
          <>
            <Button onClick={closePreview} variant="fill">
              {t("components.preview.modalCancel")}
            </Button>
            <Button onClick={closePreview} variant="primary">
              {t("components.preview.modalSave")}
            </Button>
          </>
        )}
        isOpen={interactive ? modalOpen : true}
        onClose={closePreview}
        overlayClassName={interactive ? undefined : "component-modal-preview-overlay"}
        portalled={interactive}
        preventScroll={interactive}
        showScrollbar={modalShowScrollbar}
        size="xxlarge"
        title={t("components.preview.modalTitle")}
        trapFocus={interactive}
      >
        {renderModalConfigurationContent()}
      </Modal>
    );
  }

  function renderIconButtonPreview(
    state = previewState,
    previewVariant = iconButtonVariant,
    applyInspectorControls = false,
  ) {
    return (
      <IconButton
        aria-label={t("components.preview.listView")}
        className={state === "focus-visible" ? "lab-force-focus" : undefined}
        data-bf-preview-state={state === "hover" || state === "active" ? state : undefined}
        disabled={state === "disabled" || applyInspectorControls && inspectorDisabled}
        icon={<List aria-hidden="true" />}
        loading={state === "loading" || applyInspectorControls && inspectorLoading}
        size={size}
        variant={previewVariant}
      />
    );
  }

  function renderPreview(
    state = previewState,
    previewVariant = variant,
    applyInspectorControls = false,
  ) {
    if (isFlowChatComponent) {
      return (
        <FlowChatComponentPreview
          componentName={component.name}
          key={`${component.name}-${state}`}
          state={state}
        />
      );
    }

    if (component.name === "Icon") {
      return <Icon name={iconName} size={iconSize} tone={iconTone} />;
    }

    if (component.name === "Textarea") {
      return (
        <Textarea
          className={`component-textarea-example lab-state-${state}`}
          defaultValue={t("components.preview.fieldValue")}
          disabled={state === "disabled"}
          errorMessage={t("components.preview.inputError")}
          hint={t("components.preview.fieldDescription")}
          invalid={state === "invalid"}
          key={state}
          label={t("components.preview.inputLabel")}
          maxLength={200}
          rows={3}
          showCount
        />
      );
    }

    if (component.name === "Alert") {
      const tone = state === "success" || state === "warning" || state === "error" ? state : "info";
      return (
        <Alert
          message={t("components.preview.fieldDescription")}
          title={t("components.preview.notifications")}
          tone={tone}
        />
      );
    }
    if (component.name === "Avatar") {
      return state === "grouped"
        ? <AvatarGroup><Avatar>BF</Avatar><Avatar>UI</Avatar><Avatar>DS</Avatar></AvatarGroup>
        : <Avatar alt="BitFun" key={state} src={state === "image" ? previewImage : undefined}>BF</Avatar>;
    }
    if (component.name === "Checkbox" || component.name === "Radio") {
      const Control = component.name === "Checkbox" ? Checkbox : Radio;
      return (
        <Control
          className={`component-choice-example lab-state-${state}`}
          defaultChecked={state === "checked"}
          disabled={state === "disabled"}
          invalid={state === "invalid"}
          key={state}
          label={t("components.preview.notifications")}
          {...(component.name === "Checkbox" ? { indeterminate: state === "indeterminate" } : {})}
        />
      );
    }
    if (component.name === "NumberInput") return <NumberInputPreview key={state} state={state} />;
    if (component.name === "Empty") {
      return (
        <Empty
          actions={state === "with-actions" ? <Button>{t("components.preview.add")}</Button> : undefined}
          description={t("components.preview.cardDescription")}
          title={state === "with-title" ? t("components.preview.cardTitle") : undefined}
        />
      );
    }

    if (component.name === "Disclosure") {
      return (
        <Disclosure
          data-bf-preview-state={state === "hover" || state === "focus-visible" ? state : undefined}
          defaultOpen={state === "open"}
          disabled={state === "disabled"}
          key={state}
          summary={t("components.preview.appearance")}
        >
          {t("components.preview.appearanceDescription")}
        </Disclosure>
      );
    }

    if (component.name === "ActionCard") {
      return (
        <ActionCard
          actions={[
            {
              icon: <MoreHorizontal aria-hidden="true" />,
              id: "more",
              label: t("components.preview.more"),
            },
          ]}
          className={state === "focus-visible" ? "lab-force-focus" : undefined}
          data-bf-preview-state={state === "hover" || state === "active" ? state : undefined}
          description={t("components.preview.actionCardDescription")}
          disabled={state === "disabled"}
          leading={<MessageCircle aria-hidden="true" />}
          selected={state === "selected"}
          size={actionCardSize}
          tabIndex={-1}
        >
          {t("components.preview.actionCardTitle")}
        </ActionCard>
      );
    }

    if (component.name === "StatusPill") {
      return (
        <StatusPill
          leading={<Icon name="circle" />}
          tone={state as StatusPillTone}
        >
          Ask
        </StatusPill>
      );
    }

    if (component.name === "Combobox") {
      return <Combobox key={state} label={t("components.preview.modalSelectModels")} multiple={state === "multiple"} defaultValue={state === "multiple" ? ["glm-5.2", "openbitfun"] : undefined} defaultOpen={state === "open" || state === "searching"} defaultSearchValue={state === "searching" ? "glm" : ""} allowCustomValue={state === "custom"} disabled={state === "disabled"} error={state === "invalid"} loading={state === "loading"} options={state === "empty" || state === "loading" ? [] : [{ value: "glm-5.2", label: "GLM 5.2", group: "API" }, { value: "openbitfun", label: "OpenBitFun", group: "API" }]} />;
    }
    if (component.name === "Select") {
      return (
        <Select
          aria-label="Mode"
          data-bf-preview-state={state === "hover" || state === "focus-visible" ? state : undefined}
          disabled={state === "disabled"}
          invalid={state === "invalid"}
          leading={<Icon name="circle" />}
          onValueChange={(value) => setSelectValue(String(value))}
          options={[
            { label: "Ask", value: "ask" },
            { label: "Plan", value: "plan" },
            { disabled: true, label: "Agent", value: "agent" },
          ]}
          value={selectValue}
        />
      );
    }

    if (component.name === "ActionItem") {
      return (
        <ActionItem
          actions={[
            {
              icon: <Plus aria-hidden="true" />,
              id: "add",
              label: t("components.preview.add"),
            },
            {
              icon: <MoreHorizontal aria-hidden="true" />,
              id: "more",
              label: t("components.preview.more"),
            },
          ]}
          data-bf-preview-state={state === "hover" || state === "active" ? state : undefined}
          disabled={state === "disabled"}
          leading={<MessageCircle aria-hidden="true" />}
          metadata={actionItemShowMetadata ? "12" : undefined}
          shortcut={<KeyHint>K</KeyHint>}
        >
          {t("components.preview.assistant")}
        </ActionItem>
      );
    }

    if (component.name === "ActivityItem") {
      const surface = activityItemAppearance === "surface";
      return (
        <ActivityItem
          actions={surface ? [
            {
              icon: <Copy aria-hidden="true" />,
              id: "copy",
              label: t("components.preview.activityCopy"),
            },
            {
              icon: <Download aria-hidden="true" />,
              id: "download",
              label: t("components.preview.activityDownload"),
            },
            {
              icon: <ExternalLink aria-hidden="true" />,
              id: "open",
              label: t("components.preview.activityOpen"),
            },
          ] : []}
          appearance={activityItemAppearance}
          className={state === "focus-visible"
            ? "component-activity-item-example lab-force-focus"
            : "component-activity-item-example"}
          data-bf-preview-state={state === "hover" || state === "active" ? state : undefined}
          detail={surface && activityShowDetail
            ? <code>{t("components.preview.activityDetail")}</code>
            : undefined}
          disabled={state === "disabled"}
          label={surface ? t("components.preview.activityAction") : undefined}
          leading={surface
            ? <Terminal aria-hidden="true" />
            : <Check aria-hidden="true" />}
          metadata={surface ? <ChangeCount additions={6} deletions={0} /> : undefined}
          onActivate={surface ? () => undefined : undefined}
          triggerProps={{ tabIndex: -1 }}
        >
          {surface
            ? t("components.preview.activityDescription")
            : t("components.preview.activityStatus")}
        </ActivityItem>
      );
    }

    if (component.name === "Button") {
      const inspectorIcon = applyInspectorControls && previewIcon === "chevron"
        ? <ChevronRight aria-hidden="true" size={14} />
        : undefined;
      const leadingIcon = applyInspectorControls
        ? previewIconPosition === "left" ? inspectorIcon : undefined
        : <MessageCircle aria-hidden="true" size={14} strokeWidth={1.75} />;
      const trailingIcon = applyInspectorControls
        ? previewIconPosition === "right" ? inspectorIcon : undefined
        : <ChevronDown aria-hidden="true" size={12} strokeWidth={1.75} />;
      return (
        <Button
          className={state === "focus-visible" ? "lab-force-focus" : undefined}
          data-bf-preview-state={state === "hover" || state === "active" ? state : undefined}
          disabled={state === "disabled" || applyInspectorControls && inspectorDisabled}
          leadingIcon={leadingIcon}
          loading={state === "loading" || applyInspectorControls && inspectorLoading}
          size={size}
          trailingIcon={trailingIcon}
          variant={previewVariant}
        >
          {t("components.preview.session")}
        </Button>
      );
    }

    if (component.name === "Input") {
      const previewClassName = state === "hover"
        ? "lab-force-hover"
        : state === "focus-visible"
          ? "lab-force-focus"
          : undefined;
      return (
        <Input
          aria-label={t("components.preview.inputLabel")}
          className={previewClassName}
          disabled={state === "disabled"}
          invalid={state === "invalid"}
          placeholder={t("components.preview.inputPlaceholder")}
          trailing={<Eye aria-hidden="true" />}
        />
      );
    }

    if (component.name === "Field") {
      return (
        <Field
          className="component-field-example"
          controlLeading={fieldShowControlLeading ? (
            <Switch aria-label={t("components.preview.notifications")} />
          ) : undefined}
          controlTrailing={fieldShowControlTrailing ? (
            <IconButton
              aria-label={t("components.preview.more")}
              icon={<MoreHorizontal aria-hidden="true" />}
              size="xs"
            />
          ) : undefined}
          description={t("components.preview.fieldDescription")}
          label={t("components.preview.appearance")}
          labelAction={fieldShowLabelAction ? (
            <IconButton
              aria-label={t("components.preview.fieldHelp")}
              icon={<Info aria-hidden="true" />}
              size="xs"
            />
          ) : undefined}
          orientation={fieldOrientation}
          required
        >
          <Input
            aria-label={t("components.preview.appearance")}
            defaultValue={t("components.preview.fieldValue")}
            trailing={<ChevronDown aria-hidden="true" />}
          />
        </Field>
      );
    }

    if (component.name === "KeyHint") {
      return <KeyHint icon={<Command aria-hidden="true" />}>K</KeyHint>;
    }

    if (component.name === "FieldGroup") {
      const plain = state === "plain";
      return (
        <FormSection
          className="component-field-group-example"
          description={t("components.preview.fieldDescription")}
          headingAs="h3"
          leading={<Settings aria-hidden="true" />}
          title={t("components.preview.modalSectionTitle")}
        >
          <FieldGroup appearance={plain ? "plain" : "subtle"} dividers={state === "divided"}>
            <FieldRow>
              <Field
                controlWidth="fill"
                label={t("components.preview.modalProviderName")}
                labelWidth="md"
                orientation="horizontal"
                required
              >
                <Input defaultValue="OpenBitFun" />
              </Field>
            </FieldRow>
            <FieldRow>
              <Field
                controlWidth="fill"
                label={t("components.preview.modalApiUrl")}
                labelWidth="md"
                orientation="horizontal"
              >
                <Input defaultValue="https://api.openbitfun.com" />
              </Field>
            </FieldRow>
          </FieldGroup>
        </FormSection>
      );
    }

    if (component.name === "Card") {
      if (state === "media") {
        return (
          <Card
            appearance="neutral"
            className="component-card-example component-card-example--media"
            clip
            radius="md"
          >
            <CardMedia>
              <div className="component-card-media-visual">
                <Monitor aria-hidden="true" />
              </div>
            </CardMedia>
            <CardBody align={cardContentAlign} padding="sm">
              <CardHeader
                contentAlign={cardContentAlign}
                description={t("components.preview.cardMediaDescription")}
                title={t("components.preview.cardMediaTitle")}
              />
            </CardBody>
          </Card>
        );
      }

      if (state === "subtle") {
        return (
          <Card
            appearance="subtle"
            className="component-card-example component-card-example--compact"
            gap="sm"
            padding="sm"
            radius="sm"
          >
            <CardHeader
              actions={(
                <IconButton
                  aria-label={t("components.preview.more")}
                  icon={<MoreHorizontal aria-hidden="true" />}
                  size="xs"
                />
              )}
              align="center"
              contentAlign={cardContentAlign}
              description={t("components.preview.activityDescription")}
              leading={<Terminal aria-hidden="true" />}
              title={t("components.preview.session")}
            />
          </Card>
        );
      }

      return (
        <Card
          appearance="raised"
          className="component-card-example"
          gap="md"
          padding="md"
          radius="lg"
        >
          <CardHeader
            contentAlign={cardContentAlign}
            description={t("components.preview.cardDescription")}
            title={t("components.preview.cardTitle")}
          />
          <CardBody>
            <div className="component-card-command-grid">
              {["components.preview.menuItemOne", "components.preview.menuItemTwo", "components.preview.settings"].map((key) => (
                <Card appearance="subtle" key={key} padding="sm" radius="sm">
                  <CardHeader
                    align="center"
                    leading={<Command aria-hidden="true" />}
                    title={t(key as MessageKey)}
                  />
                </Card>
              ))}
            </div>
          </CardBody>
          <CardFooter align="end">
            <Button size="sm">{t("components.preview.settings")}</Button>
          </CardFooter>
        </Card>
      );
    }

    if (component.name === "Tooltip") {
      return (
        <Tooltip
          content={t("components.preview.tooltipContent")}
          delay={0}
          placement={state as "top" | "bottom" | "left" | "right"}
        >
          <Button size="sm" variant="fill">
            {t("components.preview.tooltipTrigger")}
          </Button>
        </Tooltip>
      );
    }

    if (component.name === "Menu") {
      const itemCount = state === "scrolling" ? 12 : 3;
      return (
        <Menu
          aria-label={t("components.preview.menuLabel")}
          scrollbarVisibility={menuShowScrollbar ? "auto" : "hidden"}
        >
          <MenuSection
            actions={[{
              icon: <Plus aria-hidden="true" />,
              id: "add",
              label: t("components.preview.add"),
            }]}
            title={t("components.preview.menuSectionTitle")}
          >
            {Array.from({ length: itemCount }, (_, index) => (
              <MenuItem
                checked={state === "checked-item" && index === 0}
                className={state === "focus-within" && index === 0 ? "lab-force-focus" : undefined}
                disabled={state === "disabled-item" && index === 1}
                key={index}
                leading={<MessageCircle aria-hidden="true" />}
                role={state === "checked-item" && index === 0 ? "menuitemcheckbox" : "menuitem"}
              >
                {index === 0
                  ? t("components.preview.menuItemOne")
                  : index === 1
                    ? t("components.preview.menuItemTwo")
                    : t("components.preview.menuItem", { index: index + 1 })}
              </MenuItem>
            ))}
          </MenuSection>
          <MenuSeparator />
          <MenuSection aria-label={t("components.preview.menuMoreSection")}>
            <MenuItem>{t("components.preview.menuMoreItem")}</MenuItem>
          </MenuSection>
        </Menu>
      );
    }

    if (component.name === "Composer") {
      const showContext = composerShowContext || state === "with-context";
      return (
        <Composer
          aria-label={t("components.preview.composerLabel")}
          className={state === "focus-within" ? "component-composer-example lab-force-focus" : "component-composer-example"}
          contextBar={showContext ? (
            <ComposerContextBar
              leading={(
                <>
                  <span className="component-composer-context-label">
                    <Monitor aria-hidden="true" />
                    {t("components.preview.composerDevice")}
                    <ChevronDown aria-hidden="true" />
                  </span>
                  <ComposerDivider />
                  <span className="component-composer-context-label">
                    {t("components.preview.composerWorkspace")}
                    <ChevronDown aria-hidden="true" />
                  </span>
                </>
              )}
              trailing={(
                <span className="component-composer-mode">
                  {t("components.preview.composerMode")}
                </span>
              )}
            />
          ) : undefined}
          disabled={state === "disabled"}
          invalid={state === "invalid"}
          toolbar={composerShowToolbar ? (
            <ComposerToolbar
              leading={(
                <>
                  <IconButton
                    aria-label={t("components.preview.composerAdd")}
                    icon={<Plus aria-hidden="true" />}
                    shape="circle"
                    size="sm"
                    variant="fill"
                  />
                  <Button size="sm" variant="text">
                    {t("components.preview.composerStandard")}
                  </Button>
                </>
              )}
              trailing={(
                <>
                  <Button
                    size="sm"
                    trailingIcon={<ChevronDown aria-hidden="true" />}
                    variant="text"
                  >
                    {t("components.preview.composerModel")}
                  </Button>
                  <IconButton
                    aria-label={t("components.preview.composerVoice")}
                    icon={<Mic aria-hidden="true" />}
                    shape="circle"
                    size="sm"
                    variant="quiet"
                  />
                  <IconButton
                    aria-label={t("components.preview.composerSend")}
                    icon={<ArrowUp aria-hidden="true" />}
                    shape="circle"
                    size="sm"
                    variant="primary"
                  />
                </>
              )}
            />
          ) : undefined}
        >
          <textarea
            aria-label={t("components.preview.composerEditorLabel")}
            disabled={state === "disabled"}
            placeholder={t("components.preview.composerPlaceholder")}
            readOnly
          />
        </Composer>
      );
    }

    if (component.name === "ConfirmDialog") {
      const confirmType = state === "pending" ? "warning" : state as ConfirmDialogType;
      return (
        <ConfirmDialog
          cancelText={t("components.preview.modalCancel")}
          confirmDanger={confirmType === "error"}
          confirmText={confirmType === "error"
            ? t("components.preview.confirmDelete")
            : t("components.preview.modalSave")}
          dialogClassName="component-confirm-dialog-preview-dialog"
          isOpen
          message={t("components.preview.confirmMessage")}
          onClose={() => undefined}
          onConfirm={() => undefined}
          overlayClassName="component-confirm-dialog-preview-overlay"
          pendingAction={state === "pending" ? "confirm" : null}
          portalled={false}
          preventScroll={false}
          preview="/workspace/project"
          title={t("components.preview.confirmTitle")}
          type={confirmType}
        />
      );
    }

    if (component.name === "Modal") {
      return (
        <Button onClick={() => setModalOpen(true)} variant="fill">
          {t("components.preview.modalInteractionDemo")}
        </Button>
      );
    }

    if (component.name === "PageHeader") {
      return (
        <PageHeader
          action={(
            <IconButton
              aria-label={t("components.preview.close")}
              icon={<X aria-hidden="true" />}
            />
          )}
          align={pageHeaderAlign}
          description={t("components.preview.appearanceDescription")}
          leading={<Settings aria-hidden="true" />}
          level={2}
          required={pageHeaderRequired}
          size={pageHeaderSize}
          title={t("components.preview.appearance")}
        />
      );
    }

    if (component.name === "SearchField") {
      const previewClassName = state === "hover"
        ? "lab-force-hover"
        : state === "focus-visible"
          ? "lab-force-focus"
          : undefined;
      return (
        <SearchField
          aria-label={t("components.preview.searchLabel")}
          className={previewClassName}
          disabled={state === "disabled"}
          invalid={state === "invalid"}
          leadingIcon={<SearchIcon aria-hidden="true" />}
          placeholder={t("components.preview.searchPlaceholder")}
          shortcut={<KeyHint icon={<Command aria-hidden="true" />}>K</KeyHint>}
        />
      );
    }

    if (component.name === "NavigationPanel") {
      const itemCount = state === "scrolling" ? 14 : 5;
      return (
        <NavigationPanel
          aria-label={t("components.preview.navigationPanelLabel")}
          className="component-navigation-panel-example"
          footer={(
            <>
              <NavigationPanelItem
                className="component-navigation-panel-example__device"
                leading={<Monitor aria-hidden="true" />}
              >
                {t("components.preview.navigationPanelDevice")}
              </NavigationPanelItem>
              <IconButton
                aria-label={t("components.preview.settings")}
                icon={<Settings aria-hidden="true" />}
                size="sm"
                variant="quiet"
              />
            </>
          )}
          header={(
            <SearchField
              aria-label={t("components.preview.searchLabel")}
              leadingIcon={<SearchIcon aria-hidden="true" />}
              placeholder={t("components.preview.searchPlaceholder")}
            />
          )}
          scrollbarVisibility={navigationPanelShowScrollbar ? "auto" : "hidden"}
        >
          <NavigationPanelSection title={t("components.preview.navigationPanelSectionTitle")}>
            {Array.from({ length: itemCount }, (_, index) => (
              <NavigationPanelItem
                disabled={state === "disabled-item" && index === 1}
                key={index}
                leading={index % 3 === 0 ? <MessageCircle aria-hidden="true" /> : undefined}
                reserveLeadingSpace
                selected={state === "selected-item" && index === 0}
              >
                {index === 0
                  ? t("components.preview.menuItemOne")
                  : index === 1
                    ? t("components.preview.menuItemTwo")
                    : t("components.preview.menuItem", { index: index + 1 })}
              </NavigationPanelItem>
            ))}
          </NavigationPanelSection>
          <NavigationPanelSeparator />
          <NavigationPanelSection title={t("components.preview.navigationPanelMoreSection")}>
            <NavigationPanelItem reserveLeadingSpace>
              {t("components.preview.navigationPanelMoreItem")}
            </NavigationPanelItem>
          </NavigationPanelSection>
        </NavigationPanel>
      );
    }

    if (component.name === "ScrollArea") {
      return (
        <ScrollArea
          aria-label={t("components.preview.scrollAreaLabel")}
          className="component-scroll-area-example"
          orientation={scrollAreaOrientation}
          scrollbarVisibility={state as ScrollbarVisibility}
        >
          <div className="component-scroll-area-example__content">
            {Array.from({ length: 7 }, (_, index) => (
              <span className="component-scroll-area-example__item" key={index}>
                {t("components.preview.scrollAreaItem", { index: index + 1 })}
              </span>
            ))}
          </div>
        </ScrollArea>
      );
    }

    if (component.name === "SegmentedControl") {
      const defaultMode = state === "unselected" ? "agent" : "chat";
      return (
        <SegmentedControl
          aria-label={t("components.preview.segmentedLabel")}
          data-bf-preview-state={state === "hover" ? "hover" : undefined}
          defaultValue={defaultMode}
          disabled={state === "disabled"}
          key={state}
          options={[
            {
              icon: <MessageCircle aria-hidden="true" size={12} strokeWidth={1.75} />,
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
    }

    if (component.name === "TabGroup") {
      const defaultTab = state === "unselected" ? "settings" : "welcome";
      return (
        <TabGroup
          aria-label={t("components.preview.tabGroupLabel")}
          data-bf-preview-state={state === "hover" ? "hover" : undefined}
          defaultValue={defaultTab}
          items={[
            {
              icon: <MessageCircle aria-hidden="true" size={14} strokeWidth={1.75} />,
              label: t("components.preview.welcome"),
              value: "welcome",
            },
            {
              disabled: state === "disabled",
              icon: <MessageCircle aria-hidden="true" size={14} strokeWidth={1.75} />,
              label: t("components.preview.settings"),
              value: "settings",
            },
          ]}
          key={state}
        />
      );
    }

    if (component.name === "Toolbar") {
      const tabItems = Array.from({ length: state === "overflow" ? 9 : 2 }, (_, index) => ({
        icon: <MessageCircle aria-hidden="true" size={14} strokeWidth={1.75} />,
        label: index === 0
          ? t("components.preview.welcome")
          : index === 1
            ? t("components.preview.settings")
            : t("components.preview.menuItem", { index: index + 1 }),
        value: `tab-${index + 1}`,
      }));

      return (
        <Toolbar
          aria-label={t("components.preview.tabGroupLabel")}
          center={state === "with-center" ? (
            <ToolbarGroup>
              <ToolbarBadge>18</ToolbarBadge>
              <strong>{t("components.preview.session")}</strong>
            </ToolbarGroup>
          ) : undefined}
          className="component-toolbar-example"
          leading={state === "with-center" ? (
            <ToolbarGroup>
              <Button size="xs" trailingIcon={<ChevronDown aria-hidden="true" />} variant="text">
                {t("components.preview.welcome")}
              </Button>
              <ChangeCount additions={6} deletions={0} />
            </ToolbarGroup>
          ) : (
            <TabGroup
              aria-label={t("components.preview.tabGroupLabel")}
              defaultValue="tab-1"
              items={tabItems}
            />
          )}
          leadingOverflow={state === "overflow" ? "scroll" : "visible"}
          size={toolbarSize}
          trailing={(
            <ToolbarGroup>
              {state !== "with-center" && <ChangeCount additions={6} deletions={2} />}
              <ToolbarSeparator />
              <IconButton
                aria-label={t("components.preview.searchLabel")}
                icon={<SearchIcon aria-hidden="true" />}
                size="xs"
              />
              <IconButton
                aria-label={t("components.preview.more")}
                icon={<MoreHorizontal aria-hidden="true" />}
                size="xs"
              />
            </ToolbarGroup>
          )}
        />
      );
    }

    if (component.name !== "Switch") return <p role="status">{t("detail.previewUnavailable")}: {component.name}</p>;
    return (
      <Switch
        aria-label={t("components.preview.notifications")}
        className={state === "focus-visible" ? "lab-force-focus" : undefined}
        defaultChecked={state === "on"}
        disabled={state === "disabled"}
        key={state}
      />
    );
  }

  const copyLabel = copyStatus === "copied"
    ? t("detail.copied")
    : copyStatus === "unavailable"
      ? t("detail.copyUnavailable")
      : t("detail.copy");
  const schemeLabel = colorScheme === "dark" ? t("settings.dark") : t("settings.light");
  const contrastLabel = contrast === "high" ? t("settings.highContrast") : t("settings.standard");
  const densityLabel = density === "compact"
    ? t("settings.compact")
    : density === "touch"
      ? t("settings.touch")
      : t("settings.comfortable");

  return (
    <main className="lab-page lab-page--component-detail">
      <nav className="component-breadcrumb" aria-label={t("detail.breadcrumbLabel")}>
        <button onClick={onBack} type="button">
          {t(isFlowChatComponent ? "detail.backFlowChat" : "detail.back")}
        </button>
        <ChevronRight aria-hidden="true" size={13} />
        <span aria-current="page">{component.name}</span>
      </nav>

      <div className="component-preview-layout">
        <div className="component-preview-main">
          <header className="component-detail-heading">
            <div>
              <h1>{component.name}</h1>
              <p>{getComponentDescription(component.name, component.description, t)}</p>
            </div>
          </header>

          <section className="component-preview-panel" id="component-workbench">
            <header className="component-panel-heading">
              <h2>{t("detail.preview")}</h2>
            </header>

            <ThemeRoot
              className="component-preview-canvas"
              colorScheme={colorScheme}
              contrast={contrast}
              density={density}
              tokenOverrides={tokenOverrides}
              tabIndex={0}
              role="region"
              aria-label={t("detail.preview")}
            >
              {component.name === "ConfirmDialog" ? (
                <div className="component-confirm-dialog-preview-stage">
                  <span className="component-confirm-dialog-preview-stage__label">
                    {stateLabel(previewState)}
                  </span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "Modal" ? (
                <div className="component-modal-preview-stage">
                  {renderModalExample(false)}
                  <div className="component-modal-preview-stage__actions">
                    <Button onClick={() => setModalOpen(true)} variant="fill">
                      {t("components.preview.modalInteractionDemo")}
                    </Button>
                  </div>
                  {renderModalExample(true)}
                </div>
              ) : component.name === "Card" ? (
                <div className="component-card-preview-stage">
                  <span className="component-card-preview-stage__label">
                    {stateLabel(previewState)}
                  </span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "ActivityItem" ? (
                <div className="component-activity-item-preview-stage">
                  <span className="component-activity-item-preview-stage__label">
                    {stateLabel(activityItemAppearance)} · {stateLabel(previewState)}
                  </span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "Composer" ? (
                <div className="component-composer-preview-stage">
                  <span className="component-composer-preview-stage__label">
                    {stateLabel(previewState)}
                  </span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "Toolbar" ? (
                <div className="component-toolbar-preview-stage">
                  <span className="component-toolbar-preview-stage__label">
                    {stateLabel(previewState)}
                  </span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "Combobox" ? (
                <div className="component-combobox-preview" data-component="combobox">
                  <span>{stateLabel(previewState)}</span>
                  {renderPreview(previewState)}
                </div>
              ) : component.name === "Menu" || component.name === "NavigationPanel" || component.name === "FieldGroup" ? (
                <div className="component-surface-state-list" data-component={component.name}>
                  {states.map((state) => (
                    <section className="component-surface-state-list__item" key={state}>
                      <header className="flow-chat-state-list__heading">
                        <strong>{stateLabel(state)}</strong>
                        <code>{state}</code>
                      </header>
                      <div className="component-surface-state-list__preview">{renderPreview(state)}</div>
                    </section>
                  ))}
                </div>
              ) : isFlowChatComponent ? (
                <div
                  className="flow-chat-state-list"
                  data-component="flow-chat-tool-card"
                >
                  {states.map((state) => (
                    <section
                      className="flow-chat-state-list__item"
                      data-active={state === previewState || undefined}
                      key={state}
                    >
                      <header className="flow-chat-state-list__heading">
                        <strong>{stateLabel(state)}</strong>
                        <code>{state}</code>
                      </header>
                      <div
                        className="flow-chat-state-list__preview"
                        data-component-name={component.name}
                      >
                        {renderPreview(state)}
                      </div>
                    </section>
                  ))}
                </div>
              ) : component.name === "Button" ? (
                <div
                  className="component-preview-matrix"
                  data-component="button"
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {stateLabel(state)}
                    </span>
                  ))}
                  {buttonVariants.map((matrixVariant) => (
                    <Fragment key={matrixVariant}>
                      <span className="component-preview-matrix__row-label">
                        {stateLabel(matrixVariant)}
                      </span>
                      {states.map((state) => (
                        <div
                          className="component-preview-matrix__cell"
                          data-active={matrixVariant === variant && state === previewState || undefined}
                          key={`${matrixVariant}-${state}`}
                        >
                          {renderPreview(state, matrixVariant)}
                        </div>
                      ))}
                    </Fragment>
                  ))}
                </div>
              ) : component.name === "Icon" ? (
                <>
                  <IconCompositionPreview />
                  <div className="component-icon-catalog">
                    {iconNames.map((name) => (
                      <div className="component-icon-catalog__item" key={name}>
                        <Icon name={name} size="lg" />
                        <code>{name}</code>
                      </div>
                    ))}
                  </div>
                </>
              ) : component.name === "IconButton" ? (
                <div
                  className="component-preview-matrix"
                  data-component="icon-button"
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {stateLabel(state)}
                    </span>
                  ))}
                  {iconButtonVariants.map((matrixVariant) => (
                    <Fragment key={matrixVariant}>
                      <span className="component-preview-matrix__row-label">
                        {stateLabel(matrixVariant)}
                      </span>
                      {states.map((state) => (
                        <div
                          className="component-preview-matrix__cell"
                          data-active={matrixVariant === iconButtonVariant && state === previewState || undefined}
                          key={`${matrixVariant}-${state}`}
                        >
                          {renderIconButtonPreview(state, matrixVariant)}
                        </div>
                      ))}
                    </Fragment>
                  ))}
                </div>
              ) : component.name === "ActionCard" || component.name === "ActionItem" || component.name === "Field" || component.name === "FieldGroup" || component.name === "Input" || component.name === "KeyHint" || component.name === "Menu" || component.name === "NavigationPanel" || component.name === "PageHeader" || component.name === "ScrollArea" || component.name === "SearchField" || component.name === "SegmentedControl" || component.name === "Select" || component.name === "StatusPill" || component.name === "Tooltip" ? (
                <div
                  className="component-preview-matrix"
                  data-component={component.name === "ActionCard"
                    ? "action-card"
                    : component.name === "ActionItem"
                      ? "action-item"
                    : component.name === "Field"
                      ? "field"
                    : component.name === "StatusPill"
                      ? "status-pill"
                    : component.name === "SegmentedControl"
                      ? "segmented-control"
                    : component.name === "Select"
                      ? "select"
                    : component.name === "FieldGroup"
                      ? "field-group"
                    : component.name === "Input"
                      ? "input"
                    : component.name === "KeyHint"
                      ? "key-hint"
                      : component.name === "PageHeader"
                        ? "page-header"
                      : component.name === "Menu"
                        ? "menu"
                      : component.name === "NavigationPanel"
                        ? "navigation-panel"
                      : component.name === "ScrollArea"
                        ? "scroll-area"
                      : component.name === "Tooltip"
                        ? "tooltip"
                      : "search-field"}
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {stateLabel(state)}
                    </span>
                  ))}
                  <span className="component-preview-matrix__row-label">{component.name}</span>
                  {states.map((state) => (
                    <div
                      className="component-preview-matrix__cell"
                      data-active={state === previewState || undefined}
                      key={state}
                    >
                      {renderPreview(state)}
                    </div>
                  ))}
                </div>
              ) : component.name === "TabGroup" ? (
                <div
                  className="component-preview-matrix"
                  data-component="tab-group"
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {stateLabel(state)}
                    </span>
                  ))}
                  <span className="component-preview-matrix__row-label">TabGroup</span>
                  {states.map((state) => (
                    <div
                      className="component-preview-matrix__cell"
                      data-active={state === previewState || undefined}
                      key={state}
                    >
                      {renderPreview(state)}
                    </div>
                  ))}
                </div>
              ) : (
                <div
                  className="component-preview-matrix"
                  data-component={component.name.replace(/([a-z])([A-Z])/g, "$1-$2").toLowerCase()}
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {stateLabel(state)}
                    </span>
                  ))}
                  <span className="component-preview-matrix__row-label">{component.name}</span>
                  {states.map((state) => (
                    <div
                      className="component-preview-matrix__cell"
                      data-active={state === previewState || undefined}
                      key={state}
                    >
                      {renderPreview(state)}
                    </div>
                  ))}
                </div>
              )}
              {component.name === "Menu" && <div className="component-menu-interaction"><NestedMenuPattern /></div>}
            </ThemeRoot>
          </section>

          <section className="component-code-panel component-code-panel--standalone">
            <header className="component-panel-heading component-code-heading">
              <h2>{t("detail.code")}</h2>
              <div>
                <span>React · TypeScript</span>
                <button onClick={copyCode} type="button">
                  <Clipboard aria-hidden="true" size={14} />
                  {copyLabel}
                </button>
              </div>
            </header>
            <pre><code>{codeSample}</code></pre>
          </section>
        </div>

        <aside className="component-inspector">
          <div className="component-inspector-tabs" role="tablist" aria-label={t("detail.inspector.label")}>
            {(["properties", "styles", "tokens"] as const).map((tab) => (
              <button
                aria-selected={inspectorTab === tab}
                key={tab}
                onClick={() => setInspectorTab(tab)}
                role="tab"
                type="button"
              >
                {t(`detail.inspector.${tab}` as MessageKey)}
              </button>
            ))}
          </div>

          {inspectorTab === "properties" && (
            <div className="component-inspector-content" role="tabpanel">
              <section>
                <h2>{t("detail.inspector.basicProperties")}</h2>
                <div className="component-inspector-controls">
                  {component.name === "Button" && (
                    <InspectorSelect
                      label={t("detail.variant")}
                      onChange={(value) => setVariant(value as (typeof buttonVariants)[number])}
                      options={buttonVariants}
                      value={variant}
                    />
                  )}
                  {component.name === "IconButton" && (
                    <InspectorSelect
                      label={t("detail.variant")}
                      onChange={(value) => setIconButtonVariant(value as (typeof iconButtonVariants)[number])}
                      options={iconButtonVariants}
                      value={iconButtonVariant}
                    />
                  )}
                  {component.name === "Field" && (
                    <InspectorSelect
                      label={t("detail.orientation")}
                      onChange={(value) => setFieldOrientation(value as FieldOrientation)}
                      options={fieldOrientations}
                      value={fieldOrientation}
                    />
                  )}
                  {component.name === "PageHeader" && (
                    <InspectorSelect
                      label={t("detail.size")}
                      onChange={(value) => setPageHeaderSize(value as PageHeaderSize)}
                      options={pageHeaderSizes}
                      value={pageHeaderSize}
                    />
                  )}
                  {component.name === "PageHeader" && (
                    <InspectorSelect
                      label={t("detail.alignment")}
                      onChange={(value) => setPageHeaderAlign(value as PageHeaderAlign)}
                      options={pageHeaderAlignments}
                      value={pageHeaderAlign}
                    />
                  )}
                  {component.name === "Icon" && (
                    <InspectorSelect
                      label={t("detail.name")}
                      onChange={(value) => setIconName(value as IconName)}
                      options={iconNames}
                      translateOptions={false}
                      value={iconName}
                    />
                  )}
                  {component.name === "Icon" && (
                    <InspectorSelect
                      label={t("detail.size")}
                      onChange={(value) => setIconSize(value as IconSize)}
                      options={iconSizes}
                      translateOptions={false}
                      value={iconSize}
                    />
                  )}
                  {component.name === "Icon" && (
                    <InspectorSelect
                      label={t("detail.variant")}
                      onChange={(value) => setIconTone(value as IconTone)}
                      options={iconTones}
                      translateOptions={false}
                      value={iconTone}
                    />
                  )}
                  {component.name === "Card" && (
                    <InspectorSelect
                      label={t("detail.alignment")}
                      onChange={(value) => setCardContentAlign(value as CardContentAlignment)}
                      options={cardContentAlignments}
                      value={cardContentAlign}
                    />
                  )}
                  {component.name === "Field" && (
                    <InspectorToggle
                      checked={fieldShowLabelAction}
                      label={t("detail.showLabelAction")}
                      onCheckedChange={setFieldShowLabelAction}
                    />
                  )}
                  {component.name === "Field" && (
                    <InspectorToggle
                      checked={fieldShowControlLeading}
                      label={t("detail.showLeadingControl")}
                      onCheckedChange={setFieldShowControlLeading}
                    />
                  )}
                  {component.name === "Field" && (
                    <InspectorToggle
                      checked={fieldShowControlTrailing}
                      label={t("detail.showTrailingAction")}
                      onCheckedChange={setFieldShowControlTrailing}
                    />
                  )}
                  {component.name === "ScrollArea" && (
                    <InspectorSelect
                      label={t("detail.orientation")}
                      onChange={(value) => setScrollAreaOrientation(value as ScrollAreaOrientation)}
                      options={scrollAreaOrientations}
                      value={scrollAreaOrientation}
                    />
                  )}
                  {(component.name === "Button" || component.name === "IconButton") && (
                    <InspectorSelect
                      label={t("detail.size")}
                      onChange={(value) => setSize(value as PreviewSize)}
                      options={["sm", "md", "lg"]}
                      value={size}
                    />
                  )}
                  <InspectorSelect
                    label={t("detail.state")}
                    onChange={setPreviewState}
                    options={inspectorStates}
                    translateOptions={component.name !== "StatusPill"}
                    value={previewState}
                  />
                  {component.name === "ActivityItem" && (
                    <InspectorSelect
                      label={t("detail.variant")}
                      onChange={(value) => setActivityItemAppearance(value as ActivityItemAppearance)}
                      options={activityItemAppearances}
                      value={activityItemAppearance}
                    />
                  )}
                  {component.name === "ActivityItem" && (
                    <InspectorToggle
                      checked={activityShowDetail}
                      label={t("detail.showDetailArea")}
                      onCheckedChange={setActivityShowDetail}
                    />
                  )}
                  {component.name === "PageHeader" && (
                    <InspectorToggle
                      checked={pageHeaderRequired}
                      label={t("detail.showAsterisk")}
                      onCheckedChange={setPageHeaderRequired}
                    />
                  )}
                  {component.name === "ActionItem" && (
                    <InspectorToggle
                      checked={actionItemShowMetadata}
                      label={t("detail.showMetadata")}
                      onCheckedChange={setActionItemShowMetadata}
                    />
                  )}
                  {component.name === "ActionCard" && (
                    <InspectorSelect
                      label={t("detail.size")}
                      onChange={(value) => setActionCardSize(value as ActionCardSize)}
                      options={actionCardSizes}
                      value={actionCardSize}
                    />
                  )}
                  {component.name === "Toolbar" && (
                    <InspectorSelect
                      label={t("detail.size")}
                      onChange={(value) => setToolbarSize(value as ToolbarSize)}
                      options={["sm", "md"]}
                      value={toolbarSize}
                    />
                  )}
                  {component.name === "Composer" && (
                    <InspectorToggle
                      checked={composerShowContext}
                      label={t("detail.showContextBar")}
                      onCheckedChange={setComposerShowContext}
                    />
                  )}
                  {component.name === "Composer" && (
                    <InspectorToggle
                      checked={composerShowToolbar}
                      label={t("detail.showToolbar")}
                      onCheckedChange={setComposerShowToolbar}
                    />
                  )}
                  {component.name === "Modal" && (
                    <InspectorToggle
                      checked={modalShowScrollbar}
                      label={t("detail.showScrollbar")}
                      onCheckedChange={setModalShowScrollbar}
                    />
                  )}
                  {component.name === "Menu" && (
                    <InspectorToggle
                      checked={menuShowScrollbar}
                      label={t("detail.showScrollbar")}
                      onCheckedChange={setMenuShowScrollbar}
                    />
                  )}
                  {component.name === "NavigationPanel" && (
                    <InspectorToggle
                      checked={navigationPanelShowScrollbar}
                      label={t("detail.showScrollbar")}
                      onCheckedChange={setNavigationPanelShowScrollbar}
                    />
                  )}
                  {(component.name === "Button" || component.name === "IconButton") && (
                    <InspectorToggle
                      checked={inspectorDisabled}
                      label={t("detail.disabled")}
                      onCheckedChange={setInspectorDisabled}
                    />
                  )}
                  {(component.name === "Button" || component.name === "IconButton") && (
                    <InspectorToggle
                      checked={inspectorLoading}
                      label={t("detail.loading")}
                      onCheckedChange={setInspectorLoading}
                    />
                  )}
                  {component.name === "Button" && (
                    <InspectorSelect
                      label={t("detail.icon")}
                      onChange={(value) => setPreviewIcon(value as PreviewIcon)}
                      options={["none", "chevron"]}
                      value={previewIcon}
                    />
                  )}
                  {component.name === "Button" && (
                    <InspectorSelect
                      label={t("detail.iconPosition")}
                      onChange={(value) => setPreviewIconPosition(value as PreviewIconPosition)}
                      options={["left", "right"]}
                      value={previewIconPosition}
                    />
                  )}
                </div>
              </section>

              <section>
                <h2>{t("detail.inspector.selectedPreview")}</h2>
                <ThemeRoot
                  className="component-inspector-preview"
                  colorScheme={colorScheme}
                  contrast={contrast}
                  density={density}
                  tokenOverrides={tokenOverrides}
                >
                  {component.name === "IconButton"
                    ? renderIconButtonPreview(previewState, iconButtonVariant, true)
                    : renderPreview(previewState, variant, true)}
                </ThemeRoot>
              </section>

              <section>
                <h2>{t("detail.inspector.publicApi")}</h2>
                <div className="component-inspector-props">
                  {component.props.map((prop) => (
                    <div key={prop.name}>
                      <code>{prop.name}</code>
                      <span>{prop.type}</span>
                      <small>{prop.defaultValue ?? "—"}</small>
                    </div>
                  ))}
                </div>
              </section>
            </div>
          )}

          {inspectorTab === "styles" && (
            <div className="component-inspector-content" role="tabpanel">
              <section>
                <h2>{t("detail.inspector.themeContext")}</h2>
                <dl className="component-inspector-facts">
                  <div><dt>{t("settings.scheme")}</dt><dd>{schemeLabel}</dd></div>
                  <div><dt>{t("settings.contrast")}</dt><dd>{contrastLabel}</dd></div>
                  <div><dt>{t("settings.density")}</dt><dd>{densityLabel}</dd></div>
                  <div><dt>{t("detail.inspector.category")}</dt><dd>{getComponentCategoryLabel(component.category, t)}</dd></div>
                </dl>
              </section>
              <section>
                <h2>{t("detail.inspector.supportedStates")}</h2>
                <div className="component-inspector-chip-list">
                  {component.states.map((state) => <span key={state}>{state}</span>)}
                </div>
              </section>
            </div>
          )}

          {inspectorTab === "tokens" && (
            <div className="component-inspector-content" role="tabpanel">
              <section>
                <h2>{t("detail.ownedTokens")}</h2>
                <div className="component-inspector-token-list">
                  {component.tokens.map((token) => <code key={token}>{token}</code>)}
                </div>
                <button className="component-inspector-action" onClick={() => onInspectTokens(component.name)} type="button">
                  {t("detail.openWorkbench")}
                </button>
              </section>
            </div>
          )}
        </aside>
      </div>
    </main>
  );
}
