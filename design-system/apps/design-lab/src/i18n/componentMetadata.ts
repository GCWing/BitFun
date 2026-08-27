import type { MessageKey } from "./messages";
import type { TranslateParams } from "./core.mjs";

type Translate = (key: MessageKey, params?: TranslateParams) => string;

const categoryKeys: Readonly<Record<string, MessageKey>> = {
  action: "meta.category.action",
  feedback: "meta.category.feedback",
  form: "meta.category.form",
  navigation: "meta.category.navigation",
  primitive: "meta.category.primitive",
};

const descriptionKeys: Readonly<Record<string, MessageKey>> = {
  ActionItem: "component.ActionItem.description",
  Button: "component.Button.description",
  Composer: "component.Composer.description",
  Field: "component.Field.description",
  IconButton: "component.IconButton.description",
  Input: "component.Input.description",
  KeyHint: "component.KeyHint.description",
  Menu: "component.Menu.description",
  Modal: "component.Modal.description",
  NavigationPanel: "component.NavigationPanel.description",
  PageHeader: "component.PageHeader.description",
  SearchField: "component.SearchField.description",
  Switch: "component.Switch.description",
  TabGroup: "component.TabGroup.description",
};

export function getComponentCategoryLabel(category: string, t: Translate): string {
  return t(categoryKeys[category] ?? "meta.category.other");
}

export function getComponentDescription(
  name: string,
  fallback: string,
  t: Translate,
): string {
  const key = descriptionKeys[name];
  return key ? t(key) : fallback;
}
