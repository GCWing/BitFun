import { ChevronRight, Settings } from "lucide-react";
import { Button, Icon, IconButton, Input, SessionIcon, TabGroup } from "@bitfun/ui";
import { useI18n } from "../i18n";

export function IconCompositionPreview() {
  const { t } = useI18n();
  const label = t("components.preview.settings");
  return (
    <section className="component-icon-composition" aria-label={t("detail.iconComposition")}>
      <h3>{t("detail.iconComposition")}</h3>
      <p>{t("detail.iconCompositionHint")}</p>
      {(["xs", "sm", "md", "lg"] as const).map(size => (
        <div className="component-icon-composition__row" key={size}>
          <code>Button / {size}</code>
          <Button size={size} leadingIcon={<Icon name="settings" />} trailingIcon={<Icon name="chevron-right" />}>{label}</Button>
          <Button size={size} leadingIcon={<Settings />} trailingIcon={<ChevronRight />}>{label}</Button>
          <IconButton size={size} aria-label={`Icon / ${size}`} icon={<Icon name="settings" />} />
          <IconButton size={size} aria-label={`SVG / ${size}`} icon={<Settings />} />
        </div>
      ))}
      <div className="component-icon-composition__row">
        <code>TabGroup</code>
        <TabGroup
          aria-label={t("components.preview.tabGroupLabel")}
          defaultValue="session"
          items={[
            { value: "session", label: t("components.preview.session"), icon: <SessionIcon /> },
            { value: "settings", label, icon: <Icon name="settings" /> },
            { value: "assistant", label: t("components.preview.assistant"), icon: <Icon name="user" /> },
          ]}
        />
      </div>
      <div className="component-icon-composition__row">
        <code>Input</code>
        <Input aria-label="Icon" leading={<Icon name="settings" />} trailing={<Icon name="chevron-right" />} defaultValue={label} />
        <Input aria-label="SVG" leading={<Settings />} trailing={<ChevronRight />} defaultValue={label} />
      </div>
    </section>
  );
}
