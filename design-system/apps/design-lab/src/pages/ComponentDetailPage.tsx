import { Fragment, useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Clipboard,
  Command,
  Eye,
  List,
  MessageCircle,
  MoreHorizontal,
  Plus,
  Search as SearchIcon,
  X,
} from "lucide-react";
import {
  ActionItem,
  Button,
  Field,
  IconButton,
  Input,
  KeyHint,
  Modal,
  PageHeader,
  SearchField,
  Switch,
  TabGroup,
  ThemeRoot,
  type ColorScheme,
  type ContrastMode,
  type DensityMode,
  type TokenOverrides,
} from "@bitfun/ui";
import type { ComponentMeta } from "@bitfun/ui/registry";
import { useI18n, type MessageKey } from "../i18n";
import {
  getComponentCategoryLabel,
  getComponentDescription,
} from "../i18n/componentMetadata";

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
const pageHeaderSizes = ["sm", "md", "lg", "display"] as const;

const optionLabelKeys: Readonly<Record<string, MessageKey>> = {
  active: "detail.option.active",
  chevron: "detail.option.chevron",
  center: "detail.option.center",
  default: "detail.option.default",
  disabled: "detail.option.disabled",
  display: "detail.option.display",
  fill: "detail.option.fill",
  "focus-visible": "detail.option.focus-visible",
  hover: "detail.option.hover",
  horizontal: "detail.option.horizontal",
  invalid: "detail.option.invalid",
  left: "detail.option.left",
  lg: "detail.option.lg",
  loading: "detail.option.loading",
  md: "detail.option.md",
  none: "detail.option.none",
  off: "detail.option.off",
  on: "detail.option.on",
  outline: "detail.option.outline",
  primary: "detail.option.primary",
  quiet: "detail.option.quiet",
  right: "detail.option.right",
  selected: "detail.option.selected",
  sm: "detail.option.sm",
  start: "detail.option.start",
  text: "detail.option.text",
  unselected: "detail.option.unselected",
  vertical: "detail.option.vertical",
};

function InspectorSelect({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: readonly string[];
  value: string;
}) {
  const { t } = useI18n();

  return (
    <label className="component-inspector-select">
      <span>{label}</span>
      <select onChange={(event) => onChange(event.target.value)} value={value}>
        {options.map((option) => (
          <option key={option} value={option}>
            {t(optionLabelKeys[option] ?? "detail.option.default")}
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
  const [variant, setVariant] = useState<(typeof buttonVariants)[number]>("fill");
  const [iconButtonVariant, setIconButtonVariant] = useState<(typeof iconButtonVariants)[number]>("quiet");
  const [size, setSize] = useState<PreviewSize>("md");
  const [fieldOrientation, setFieldOrientation] = useState<FieldOrientation>("horizontal");
  const [pageHeaderAlign, setPageHeaderAlign] = useState<PageHeaderAlign>("start");
  const [pageHeaderSize, setPageHeaderSize] = useState<PageHeaderSize>("lg");
  const [previewState, setPreviewState] = useState(
    component.name === "Switch"
      ? "off"
      : component.name === "TabGroup"
        ? "selected"
        : "default",
  );
  const [inspectorDisabled, setInspectorDisabled] = useState(false);
  const [inspectorLoading, setInspectorLoading] = useState(false);
  const [previewIcon, setPreviewIcon] = useState<PreviewIcon>("none");
  const [previewIconPosition, setPreviewIconPosition] = useState<PreviewIconPosition>("left");
  const [copyStatus, setCopyStatus] = useState<CopyStatus>("idle");
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>("properties");
  const [modalOpen, setModalOpen] = useState(false);
  const [modalShowScrollbar, setModalShowScrollbar] = useState(true);

  const states = useMemo(() => {
    switch (component.name) {
      case "ActionItem":
        return ["default", "hover", "active", "disabled"] as const;
      case "Button":
      case "IconButton":
        return ["default", "hover", "active", "disabled"] as const;
      case "Input":
      case "SearchField":
        return ["default", "hover", "focus-visible", "invalid", "disabled"] as const;
      case "Field":
      case "KeyHint":
      case "Modal":
      case "PageHeader":
        return ["default"] as const;
      case "TabGroup":
        return ["selected", "unselected", "hover", "disabled"] as const;
      default:
        return ["off", "on", "focus-visible", "disabled"] as const;
    }
  }, [component.name]);
  const inspectorStates = component.name === "Button" || component.name === "IconButton"
    ? buttonInspectorStates
    : states;

  const codeSample = useMemo(() => {
    if (component.name === "ActionItem") {
      return `import { ActionItem, KeyHint } from "@bitfun/ui";\nimport { MessageCircle, MoreHorizontal, Plus } from "lucide-react";\n\n<ActionItem\n  actions={[\n    { id: "add", icon: <Plus />, label: "${t("components.preview.add")}" },\n    { id: "more", icon: <MoreHorizontal />, label: "${t("components.preview.more")}" },\n  ]}\n  leading={<MessageCircle />}\n  shortcut={<KeyHint>K</KeyHint>}\n>\n  ${t("components.preview.assistant")}\n</ActionItem>`;
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
    if (component.name === "IconButton") {
      const stateProps = `${inspectorDisabled ? " disabled" : ""}${inspectorLoading ? " loading" : ""}`;
      return `import { IconButton } from "@bitfun/ui";\nimport { List } from "lucide-react";\n\n<IconButton\n  aria-label="${t("components.preview.listView")}"\n  icon={<List />}\n  variant="${iconButtonVariant}"${stateProps}\n/>`;
    }
    if (component.name === "Field") {
      return `import { Field, Switch } from "@bitfun/ui";\n\n<Field\n  description="${t("components.preview.fieldDescription")}"\n  label="${t("components.preview.notifications")}"\n  orientation="${fieldOrientation}"\n  required\n>\n  <Switch />\n</Field>`;
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
    if (component.name === "Modal") {
      return `import { Button, Modal } from "@bitfun/ui";\n\n<Modal\n  contentPadding="lg"\n  footer={<>\n    <Button onClick={() => setOpen(false)} variant="fill">${t("components.preview.modalCancel")}</Button>\n    <Button onClick={() => setOpen(false)} variant="primary">${t("components.preview.modalSave")}</Button>\n  </>}\n  isOpen={open}\n  onClose={() => setOpen(false)}\n  showScrollbar={${modalShowScrollbar}}\n  size="xxlarge"\n  title="${t("components.preview.modalTitle")}"\n>\n  <ProviderConfigurationFields />\n</Modal>`;
    }
    if (component.name === "PageHeader") {
      return `import { IconButton, PageHeader } from "@bitfun/ui";\nimport { X } from "lucide-react";\n\n<PageHeader\n  action={<IconButton aria-label="${t("components.preview.close")}" icon={<X />} />}\n  align="${pageHeaderAlign}"\n  description="${t("components.preview.appearanceDescription")}"\n  level={2}\n  size="${pageHeaderSize}"\n  title="${t("components.preview.appearance")}"\n/>`;
    }
    if (component.name === "SearchField") {
      const stateProps = previewState === "disabled"
        ? " disabled"
        : previewState === "invalid"
          ? " invalid"
          : "";
      return `import { KeyHint, SearchField } from "@bitfun/ui";\nimport { Command, Search } from "lucide-react";\n\n<SearchField\n  aria-label="${t("components.preview.searchLabel")}"\n  leadingIcon={<Search />}\n  placeholder="${t("components.preview.searchPlaceholder")}"\n  shortcut={<KeyHint icon={<Command />}>K</KeyHint>}${stateProps}\n/>`;
    }
    if (component.name === "TabGroup") {
      const defaultTab = previewState === "unselected" ? "settings" : "welcome";
      return `import { TabGroup } from "@bitfun/ui";\nimport { MessageCircle } from "lucide-react";\n\nconst items = [\n  { icon: <MessageCircle />, label: "${t("components.preview.welcome")}", value: "welcome" },\n  { icon: <MessageCircle />, label: "${t("components.preview.settings")}", value: "settings" },\n];\n\n<TabGroup\n  aria-label="${t("components.preview.tabGroupLabel")}"\n  defaultValue="${defaultTab}"\n  items={items}\n/>`;
    }
    const stateProps = previewState === "on"
      ? " defaultChecked"
      : previewState === "disabled"
        ? " disabled"
        : "";
    return `import { Switch } from "@bitfun/ui";\n\n<Switch\n  aria-label="${t("components.preview.notifications")}"${stateProps}\n/>`;
  }, [
    component.name,
    fieldOrientation,
    iconButtonVariant,
    inspectorDisabled,
    inspectorLoading,
    modalShowScrollbar,
    pageHeaderAlign,
    pageHeaderSize,
    previewIcon,
    previewIconPosition,
    previewState,
    size,
    t,
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
      <section className="component-modal-example" aria-label={t("components.preview.modalSectionTitle")}>
        <h3>{t("components.preview.modalSectionTitle")}</h3>
        <div className="component-modal-example__panel">
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalProviderName")}
              orientation="horizontal"
              required
            >
              <Input className="component-modal-example__control" defaultValue="OpenBitFun" />
            </Field>
          </div>
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalAuthentication")}
              orientation="horizontal"
              required
            >
              <Input
                className="component-modal-example__control"
                defaultValue="API Key"
                readOnly
                trailing={<ChevronDown aria-hidden="true" />}
              />
            </Field>
          </div>
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalApiKey")}
              orientation="horizontal"
              required
            >
              <Input
                className="component-modal-example__control"
                defaultValue="bitfun-provider-api-key"
                readOnly
                trailing={<Eye aria-hidden="true" />}
                type="password"
              />
            </Field>
          </div>
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalApiUrl")}
              orientation="horizontal"
            >
              <Input
                className="component-modal-example__control"
                defaultValue="https://api.openbitfun.com"
              />
            </Field>
          </div>
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalRequestFormat")}
              orientation="horizontal"
            >
              <Input
                className="component-modal-example__control"
                defaultValue="Anthropic (messages)"
                readOnly
                trailing={<ChevronDown aria-hidden="true" />}
              />
            </Field>
          </div>
          <div className="component-modal-example__row">
            <Field
              className="component-modal-example__field"
              controlClassName="component-modal-example__field-control"
              label={t("components.preview.modalSelectModels")}
              orientation="horizontal"
              required
            >
              <Input
                className="component-modal-example__control"
                defaultValue="k3-256k"
                trailing={<Plus aria-hidden="true" />}
              />
            </Field>
          </div>
        </div>
        <p className="component-modal-example__hint">{t("components.preview.modalPresetModels")}</p>
        <div className="component-modal-example__model-card">
          <strong>k3-256k</strong>
          <span>{t("components.preview.modalModelSummary")}</span>
        </div>
      </section>
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
          shortcut={<KeyHint>K</KeyHint>}
        >
          {t("components.preview.assistant")}
        </ActionItem>
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
          description={t("components.preview.fieldDescription")}
          label={t("components.preview.notifications")}
          orientation={fieldOrientation}
          required
        >
          <Switch />
        </Field>
      );
    }

    if (component.name === "KeyHint") {
      return <KeyHint icon={<Command aria-hidden="true" />}>K</KeyHint>;
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
          level={2}
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
        <button onClick={onBack} type="button">{t("detail.back")}</button>
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
            >
              {component.name === "Modal" ? (
                <div className="component-modal-preview-stage">
                  {renderModalExample(false)}
                  <div className="component-modal-preview-stage__actions">
                    <Button onClick={() => setModalOpen(true)} variant="fill">
                      {t("components.preview.modalInteractionDemo")}
                    </Button>
                  </div>
                  {renderModalExample(true)}
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
                      {t(optionLabelKeys[state] ?? "detail.option.default")}
                    </span>
                  ))}
                  {buttonVariants.map((matrixVariant) => (
                    <Fragment key={matrixVariant}>
                      <span className="component-preview-matrix__row-label">
                        {t(optionLabelKeys[matrixVariant] ?? "detail.option.default")}
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
                      {t(optionLabelKeys[state] ?? "detail.option.default")}
                    </span>
                  ))}
                  {iconButtonVariants.map((matrixVariant) => (
                    <Fragment key={matrixVariant}>
                      <span className="component-preview-matrix__row-label">
                        {t(optionLabelKeys[matrixVariant] ?? "detail.option.default")}
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
              ) : component.name === "ActionItem" || component.name === "Field" || component.name === "Input" || component.name === "KeyHint" || component.name === "PageHeader" || component.name === "SearchField" ? (
                <div
                  className="component-preview-matrix"
                  data-component={component.name === "ActionItem"
                    ? "action-item"
                    : component.name === "Field"
                      ? "field"
                    : component.name === "Input"
                      ? "input"
                    : component.name === "KeyHint"
                      ? "key-hint"
                      : component.name === "PageHeader"
                        ? "page-header"
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
                      {t(optionLabelKeys[state] ?? "detail.option.default")}
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
                      {t(optionLabelKeys[state] ?? "detail.option.default")}
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
                  data-component="switch"
                  data-state-count={states.length}
                >
                  <span className="component-preview-matrix__corner" />
                  {states.map((state, index) => (
                    <span
                      className="component-preview-matrix__column-label"
                      data-last={index === states.length - 1 || undefined}
                      key={state}
                    >
                      {t(optionLabelKeys[state] ?? "detail.option.default")}
                    </span>
                  ))}
                  <span className="component-preview-matrix__row-label">Switch</span>
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
                    value={previewState}
                  />
                  {component.name === "Modal" && (
                    <InspectorToggle
                      checked={modalShowScrollbar}
                      label={t("detail.showScrollbar")}
                      onCheckedChange={setModalShowScrollbar}
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
