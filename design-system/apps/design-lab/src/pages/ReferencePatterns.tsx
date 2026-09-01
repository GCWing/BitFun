import { useRef, useState } from "react";
import {
  Button,
  Card,
  CardFooter,
  ChangeCount,
  Disclosure,
  Field,
  FieldGroup,
  FieldRow,
  Icon,
  IconButton,
  Input,
  KeyHint,
  MenuPopover,
  MultiSelect,
  NumberInput,
  PageHeader,
  SearchField,
  Select,
  StatusPill,
  Switch,
  TabGroup,
  Textarea,
  Toolbar,
  ToolbarGroup,
  ToolbarSeparator,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from "@bitfun/ui";
import { useI18n } from "../i18n";

export function ProviderConfigurationPattern() {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  const [saved, setSaved] = useState(false);
  const footer = (close: () => void) => <CardFooter align="center">
    <Button variant="outline" onClick={close}>{t("components.preview.modalCancel")}</Button>
    <Button variant="fill" onClick={() => { setSaved(true); setOpen(false); }}>{t("components.preview.modalSave")}</Button>
  </CardFooter>;

  return <div className="pattern-provider" data-bf-pattern="provider-configuration">
    <div className="pattern-demo-actions">
      <Button size="sm" onClick={() => setOpen(true)}>{t("components.preview.modalInteractionDemo")}</Button>
    </div>
    <>
      <Card appearance="raised" padding="md" gap="lg" radius="lg">
        <PageHeader level={3} size="md" title={t("components.preview.modalTitle")} />
        <ProviderFields key={revision} />
        {footer(() => { setRevision(value => value + 1); setSaved(false); })}
      </Card>
      <Dialog
        open={open}
        onOpenChange={(nextOpen) => { if (!nextOpen) (() => setOpen(false))(); }}
        size="md"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t("components.preview.modalTitle")}</DialogTitle>
          </DialogHeading>
          <DialogClose aria-label={t("components.preview.close")} />
        </DialogHeader>
        <DialogBody inset="none">
          <div className="pattern-provider-modal">
        <ProviderFields />
                </div>
                </DialogBody>
        <DialogFooter>{footer(() => setOpen(false))}</DialogFooter>
      </Dialog>
    </>
    <p className="pattern-feedback" role="status">{t(saved ? "patterns.provider.saved" : "patterns.provider.previewOnly")}</p>
  </div>;
}

function ProviderFields() {
  const { t } = useI18n();
  const [models, setModels] = useState(["glm-5.2", "glm-4.7"]);
  const [custom, setCustom] = useState("");
  const [contextWindows, setContextWindows] = useState<Record<string, number>>({});
  const add = () => { const model = custom.trim(); if (model && !models.includes(model)) setModels([...models, model]); setCustom(""); };
  return <div className="pattern-provider-fields">
    <FieldGroup appearance="subtle" dividers>
      <FieldRow><Field label={t("components.preview.modalProviderName")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><Input defaultValue="Z.ai" /></Field></FieldRow>
      <FieldRow><Field label={t("components.preview.modalApiKey")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><Input type="password" placeholder="sk-…" autoComplete="off" /></Field></FieldRow>
      <FieldRow><Field label={t("components.preview.modalApiUrl")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><Input type="url" defaultValue="https://api.example.com/v1" /></Field></FieldRow>
      <FieldRow><Field label={t("components.preview.modalRequestFormat")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><Select defaultValue="openai" options={[{ label: "OpenAI", value: "openai" }, { label: "Anthropic", value: "anthropic" }]} /></Field></FieldRow>
      <FieldRow><Field label={t("components.preview.modalSelectModels")} orientation="horizontal" labelWidth="sm" controlWidth="fill" required>
        <MultiSelect clearable value={models} onValueChange={value => setModels(value.map(String))} onCreateValue={value => value} options={[
          { value: "glm-5.2", label: "GLM 5.2", group: t("patterns.provider.presets") },
          { value: "glm-4.7", label: "GLM 4.7", group: t("patterns.provider.presets") },
          { value: "glm-4.6v", label: "GLM 4.6V", group: t("patterns.provider.vision") },
        ]} />
      </Field></FieldRow>
    </FieldGroup>
    <div className="pattern-model-entry">
      <Input aria-label={t("patterns.provider.custom")} placeholder={t("patterns.provider.custom")} value={custom} onChange={event => setCustom(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && !event.nativeEvent.isComposing && event.nativeEvent.keyCode !== 229) { event.preventDefault(); add(); } }} />
      <Button variant="outline" disabled={!custom.trim()} onClick={add}>{t("patterns.provider.add")}</Button>
    </div>
    {models.map((model, index) => <Card key={model} appearance="subtle" padding="sm">
      <Disclosure summary={model} defaultOpen={index === 0} description={t("components.preview.modalModelSummary")} actions={<IconButton aria-label={`${t("patterns.provider.remove")}: ${model}`} icon={<Icon name="xmark" />} size="sm" variant="quiet" onClick={() => setModels(models.filter(value => value !== model))} />}>
        <FieldGroup appearance="plain" dividers>
          <FieldRow><Field label={t("patterns.provider.category")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><Select defaultValue="general" options={[{ label: t("patterns.provider.general"), value: "general" }, { label: t("patterns.provider.vision"), value: "vision" }]} /></Field></FieldRow>
          <FieldRow><Field label={t("patterns.provider.context")} orientation="horizontal" labelWidth="sm" controlWidth="fill"><NumberInput value={contextWindows[model] ?? 131072} onValueChange={value => setContextWindows(previous => ({ ...previous, [model]: value }))} min={1024} step={1024} disableWheel incrementLabel={t("patterns.provider.increase")} decrementLabel={t("patterns.provider.decrease")} /></Field></FieldRow>
          <FieldRow><Field label={t("patterns.provider.reasoning")} orientation="horizontal" labelWidth="sm"><Switch defaultChecked aria-label={t("patterns.provider.reasoning")} /></Field></FieldRow>
        </FieldGroup>
        <Card appearance="raised" padding="sm" gap="sm"><PageHeader level={4} size="sm" title={t("patterns.provider.preset")} description={t("patterns.provider.presetDescription")} /><Select aria-label={t("patterns.provider.preset")} defaultValue="auto" options={[{ label: t("patterns.provider.auto"), value: "auto" }, { label: t("patterns.provider.high"), value: "high" }]} /></Card>
      </Disclosure>
    </Card>)}
    <Disclosure summary={t("patterns.provider.advanced")}>
      <Field label={t("patterns.provider.headers")} controlWidth="fill"><Textarea defaultValue={'{\n  "X-Client": "BitFun"\n}'} rows={3} /></Field>
    </Disclosure>
  </div>;
}

export function SceneToolbarPattern() {
  const { t } = useI18n();
  const [tabs, setTabs] = useState(["README.md", "models.ts", "design-system"]);
  const [active, setActive] = useState("README.md");
  const [search, setSearch] = useState(false);
  const [details, setDetails] = useState(false);
  const nextTab = useRef(1);
  const close = (value: string) => { const next = tabs.filter(tab => tab !== value); setTabs(next); if (active === value) setActive(next[0] ?? ""); };
  return <Card appearance="raised" data-bf-pattern="scene-toolbar" className="pattern-scene-toolbar">
    <Toolbar leadingOverflow="scroll" leading={<TabGroup aria-label={t("patterns.toolbar.tabs")} value={active} onValueChange={setActive} items={tabs.map(value => ({ value, label: value, id: `pattern-tab-${value}`, panelId: `pattern-panel-${value}`, icon: <Icon name="files" size="sm" />, endAction: <IconButton aria-label={`${t("components.preview.close")}: ${value}`} icon={<Icon name="xmark" />} variant="quiet" size="xs" onClick={() => close(value)} /> }))} />}
      trailing={<ToolbarGroup>
        <ChangeCount additions={12} deletions={3} />
        <ToolbarSeparator />
        <IconButton aria-label={t("components.preview.add")} icon={<Icon name="plus" />} variant="quiet" onClick={() => { const name = `file-${nextTab.current++}.ts`; setTabs([...tabs, name]); setActive(name); }} />
        <IconButton aria-label={t("patterns.toolbar.search")} aria-pressed={search} icon={<Icon name="search" />} variant="quiet" onClick={() => setSearch(!search)} />
        <IconButton aria-label={t("patterns.toolbar.details")} aria-pressed={details} icon={<Icon name="settings" />} variant="quiet" onClick={() => setDetails(!details)} />
      </ToolbarGroup>} />
    <div className="pattern-scene-content">
      {search && <SearchField aria-label={t("patterns.toolbar.search")} placeholder={t("patterns.toolbar.search")} />}
      {active ? <div role="tabpanel" tabIndex={0} id={`pattern-panel-${active}`} aria-labelledby={`pattern-tab-${active}`}><PageHeader level={3} size="sm" title={active} description={t("patterns.toolbar.panel")} /></div> : <p>{t("patterns.toolbar.empty")}</p>}
      {details && <StatusPill tone="info">{t("patterns.toolbar.detailsVisible")}</StatusPill>}
    </div>
  </Card>;
}

export function NestedMenuPattern() {
  const { t } = useI18n();
  const anchorRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | undefined>();
  const [lastAction, setLastAction] = useState("");
  const [pinned, setPinned] = useState(false);
  return <Card appearance="subtle" padding="md" gap="md" data-bf-pattern="nested-menu" onContextMenu={event => { event.preventDefault(); setPosition({ x: event.clientX, y: event.clientY }); setOpen(true); }}>
    <PageHeader level={3} size="sm" title={t("patterns.menu.title")} description={t("patterns.menu.hint")} />
    <Button ref={anchorRef} aria-haspopup="menu" aria-expanded={open} leadingIcon={<Icon name="more" />} onClick={() => { setPosition(undefined); setOpen(!open); }}>{t("patterns.menu.open")}</Button>
    <MenuPopover aria-label={t("patterns.menu.title")} open={open} onClose={() => setOpen(false)} anchorRef={position ? undefined : anchorRef} position={position} items={[
      { id: "open", label: t("patterns.actions.openFiles"), icon: <Icon name="files" />, shortcut: <KeyHint>Ctrl O</KeyHint>, onSelect: () => setLastAction(t("patterns.actions.openFiles")) },
      { id: "tools", label: t("patterns.navigation.tools"), icon: <Icon name="extension" />, submenu: [
        { id: "browser", label: t("patterns.actions.openBrowser"), icon: <Icon name="browser" />, onSelect: () => setLastAction(t("patterns.actions.openBrowser")) },
        { id: "terminal", label: t("patterns.actions.openTerminal"), icon: <Icon name="terminal" />, submenu: [
          { id: "local", label: t("patterns.menu.local"), onSelect: () => setLastAction(t("patterns.menu.local")) },
          { id: "remote", label: t("patterns.menu.remote"), disabled: true },
        ] },
      ] },
      { id: "separator", label: "", separator: true },
      { id: "pin", label: t("patterns.menu.pin"), role: "menuitemcheckbox", checked: pinned, icon: <Icon name={pinned ? "check-line" : "pin"} />, onSelect: () => { setPinned(!pinned); setLastAction(t("patterns.menu.pin")); } },
    ]} />
    <p className="pattern-feedback" role="status">{lastAction ? t("patterns.menu.lastAction", { action: lastAction }) : t("patterns.menu.keyboard")}</p>
  </Card>;
}
