import type { MessageKey } from "./messages";
import type { TranslateParams } from "./core.mjs";

type Translate = (key: MessageKey, params?: TranslateParams) => string;

const categoryKeys: Readonly<Record<string, MessageKey>> = {
  action: "meta.category.action",
  feedback: "meta.category.feedback",
  form: "meta.category.form",
  media: "meta.category.media",
  navigation: "meta.category.navigation",
  primitive: "meta.category.primitive",
};

const descriptionKeys: Readonly<Record<string, MessageKey>> = {
  Button: "component.Button.description",
  Heading: "component.Heading.description",
  IconButton: "component.IconButton.description",
  Input: "component.Input.description",
  KeyHint: "component.KeyHint.description",
  ListItem: "component.ListItem.description",
  MediaThumbnail: "component.MediaThumbnail.description",
  NavigationList: "component.NavigationList.description",
  PromptComposer: "component.PromptComposer.description",
  Search: "component.Search.description",
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
