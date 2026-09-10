import { Button, Icon, IconButton, Input, SessionIcon, TabGroup } from "@openbitfun/ui";
import { useI18n } from "../i18n";

export function IconCompositionPreview() {
  const { t } = useI18n();
  const label = t("components.preview.session");
  return (
    <section className="component-icon-composition" aria-label={t("detail.iconComposition")}>
      <h3>{t("detail.iconComposition")}</h3>
      <p>{t("detail.iconCompositionHint")}</p>
      {(["panel", "tertiary"] as const).map(surface => (
        <div className="component-icon-composition__surface" data-surface={surface} key={surface}>
          {(["outline", "primary"] as const).flatMap(variant => [false, true].map(disabled => (
            <div className="component-icon-composition__row" key={`${variant}-${disabled}`}>
              <code>{variant} / {disabled ? "disabled" : "default"}</code>
              <Button variant={variant} disabled={disabled} leadingIcon={<Icon name="session" />} trailingIcon={<Icon name="chevron-down" />}>{label}</Button>
              <Button variant={variant} disabled={disabled} leadingIcon={<SessionIcon aria-hidden="true" />} trailingIcon={<Icon name="chevron-right" />}>{label}</Button>
            </div>
          )))}
          {(["default", "hover", "active", "disabled"] as const).map(state => (
            <div className="component-icon-composition__row" key={state}>
              <code>IconButton / {state}</code>
              <IconButton aria-label={`22px / ${state}`} icon={<Icon name="session" />} size="xs" disabled={state === "disabled"} data-openbitfun-preview-state={state} />
              <IconButton aria-label={`30px / ${state}`} icon={<Icon name="session" />} size="standard" shape="circle" variant="outline" disabled={state === "disabled"} data-openbitfun-preview-state={state} />
            </div>
          ))}
        </div>
      ))}
      {(["xs", "sm", "md", "lg"] as const).map(size => (
        <div className="component-icon-composition__row" key={size}>
          <code>Button / {size}</code>
          <Button size={size} leadingIcon={<Icon name="session" />} trailingIcon={<Icon name="chevron-right" />}>{label}</Button>
          <Button size={size} leadingIcon={<SessionIcon aria-hidden="true" />} trailingIcon={<Icon name="chevron-right" />}>{label}</Button>
          <IconButton size={size} aria-label={`Icon / ${size}`} icon={<Icon name="session" />} />
          <IconButton size={size} aria-label={`SVG / ${size}`} icon={<SessionIcon aria-hidden="true" />} />
        </div>
      ))}
      <div className="component-icon-composition__row">
        <code>TabGroup</code>
        <TabGroup
          aria-label={t("components.preview.tabGroupLabel")}
          defaultValue="session"
          items={[
            { value: "session", label: t("components.preview.session"), icon: <SessionIcon /> },
            { value: "settings", label: t("components.preview.settings"), icon: <Icon name="settings" /> },
            { value: "assistant", label: t("components.preview.assistant"), icon: <Icon name="user" /> },
          ]}
        />
      </div>
      <div className="component-icon-composition__row">
        <code>Input</code>
        <Input aria-label="Icon" leading={<Icon name="session" />} trailing={<Icon name="chevron-right" />} defaultValue={label} />
        <Input aria-label="SVG" leading={<SessionIcon aria-hidden="true" />} trailing={<Icon name="chevron-right" />} defaultValue={label} />
      </div>
    </section>
  );
}
