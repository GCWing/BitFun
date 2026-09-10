import { useState } from "react";
import { Button, RollingText, Stack, TabGroup } from "@openbitfun/ui";
import { useI18n } from "../i18n";

/** Manual, event-driven specimens of the public text and tab contracts. */
export function RollingTextPreview({ interactive = false }: { interactive?: boolean }) {
  const { t } = useI18n();
  const [index, setIndex] = useState(0);
  const labels = [
    t("components.preview.welcome"),
    t("components.preview.rollingTextLong"),
    t("components.preview.session"),
  ] as const;
  const label = labels[index % labels.length] ?? labels[0];

  return (
    <Stack gap="3" style={{ inlineSize: 240, maxInlineSize: "100%" }}>
      <RollingText transitionKey={index}>{label}</RollingText>
      {interactive && (
        <>
          <TabGroup
            aria-label={t("components.preview.tabGroupLabel")}
            items={[{ value: "slot", label, labelTransitionKey: index }]}
            size="sm"
            style={{ maxInlineSize: "100%" }}
          />
          <Button onClick={() => setIndex(value => value + 1)} size="sm">
            {t("components.preview.replaceText")}
          </Button>
        </>
      )}
    </Stack>
  );
}
