import {
  ArrowRight,
  Eye,
} from "lucide-react";
import { useI18n } from "../i18n";
import { getComponentDescription } from "../i18n/componentMetadata";
import {
  FlowChatComponentPreview,
  flowChatPreviewRegistry,
  type FlowChatToolSpecimen,
} from "./FlowChatPreviewRegistry";
import "./FlowChatToolGallery.css";

interface FlowChatToolGalleryProps {
  onOpenComponent: (name: string) => void;
}

const productOwnedToolExamples = [
  "CreatePlan",
  "submit_code_review",
  "MCP",
  "InitMiniApp",
  "GenerativeUI",
  "ComputerUse",
  "CreateCanvas",
  "ReadCanvas",
  "UpdateCanvas",
  "PatchCanvas",
] as const;

function ToolSpecimenCard({
  componentName,
  specimen,
}: {
  componentName: string;
  specimen: FlowChatToolSpecimen;
}) {
  return (
    <article className="flow-chat-tool-specimen">
      <header className="flow-chat-tool-specimen__heading">
        <code>{specimen.tool}</code>
        <span>{componentName}</span>
      </header>
      <FlowChatComponentPreview
        componentName={componentName}
        specimen={specimen}
      />
    </article>
  );
}

export function FlowChatToolGallery({
  onOpenComponent,
}: FlowChatToolGalleryProps) {
  const { t } = useI18n();
  const toolCardEntries = flowChatPreviewRegistry.filter(
    ({ definition }) => definition.section === "tool-card",
  );

  return (
    <section className="flow-chat-tool-gallery">
      <header className="component-catalog-section-heading">
        <div>
          <span className="page-kicker">{t("components.flowChat.toolsKicker")}</span>
          <h2>{t("components.flowChat.toolsTitle")}</h2>
        </div>
        <p>{t("components.flowChat.toolsDescription")}</p>
      </header>

      {toolCardEntries.map(({ component, definition }) => (
        <section className="flow-chat-tool-group" key={component.name}>
          <header className="flow-chat-tool-group__heading">
            <div>
              <span className="flow-chat-tool-group__attention">
                {t(`components.flowChat.attention.${definition.attention}`)}
              </span>
              <h3>{component.name}</h3>
              <p>{getComponentDescription(component.name, component.description, t)}</p>
            </div>
            <button
              aria-label={t("components.flowChat.openComponent", { name: component.name })}
              onClick={() => onOpenComponent(component.name)}
              type="button"
            >
              <span>{t("components.flowChat.toolCount", {
                count: definition.specimens.length,
              })}</span>
              <ArrowRight aria-hidden="true" />
            </button>
          </header>
          <div className="flow-chat-tool-grid">
            {definition.specimens.map((specimen) => (
              <ToolSpecimenCard
                componentName={component.name}
                key={specimen.tool}
                specimen={specimen}
              />
            ))}
          </div>
        </section>
      ))}

      <aside className="flow-chat-dedicated-note">
        <Eye aria-hidden="true" />
        <div>
          <strong>{t("components.flowChat.dedicatedTitle")}</strong>
          <p>{t("components.flowChat.dedicatedDescription")}</p>
        </div>
        <code>{productOwnedToolExamples.join(" · ")}</code>
      </aside>
    </section>
  );
}
