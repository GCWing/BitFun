import {
  ArrowUpRight,
  BookOpen,
  Boxes,
  FileCode2,
  FileText,
  Palette,
  ShieldCheck,
} from "lucide-react";
import { componentRegistry } from "@bitfun/ui/registry";
import { useI18n, type MessageKey } from "../i18n";
import { editableTokenCatalog } from "../token-editor/catalog";

const repositoryBase = "https://github.com/GCWing/BitFun/blob/main/design-system";

const resources: readonly {
  description: MessageKey;
  href: string;
  icon: typeof BookOpen;
  title: MessageKey;
}[] = [
  {
    description: "resources.designSystemDescription",
    href: `${repositoryBase}/README.md`,
    icon: BookOpen,
    title: "resources.designSystemTitle",
  },
  {
    description: "resources.uiDescription",
    href: `${repositoryBase}/packages/ui/README.md`,
    icon: Boxes,
    title: "resources.uiTitle",
  },
  {
    description: "resources.tokensDescription",
    href: `${repositoryBase}/packages/design-tokens/README.md`,
    icon: FileCode2,
    title: "resources.tokensTitle",
  },
  {
    description: "resources.themeDescription",
    href: `${repositoryBase}/packages/theme-bitfun/README.md`,
    icon: Palette,
    title: "resources.themeTitle",
  },
  {
    description: "resources.releaseDescription",
    href: `${repositoryBase}/docs/release-policy.md`,
    icon: ShieldCheck,
    title: "resources.releaseTitle",
  },
  {
    description: "resources.contributorDescription",
    href: `${repositoryBase}/AGENTS.md`,
    icon: FileText,
    title: "resources.contributorTitle",
  },
];

export function ResourcesPage() {
  const { t } = useI18n();

  return (
    <main className="lab-page lab-page--resources" id="resources">
      <header className="page-heading">
        <span className="page-kicker">{t("resources.kicker")}</span>
        <h1>{t("resources.title")}</h1>
        <p>{t("resources.description")}</p>
      </header>

      <section className="resource-fact-strip" aria-label={t("resources.factsLabel")}>
        <span><strong>{componentRegistry.length}</strong>{t("resources.componentsFact")}</span>
        <span><strong>{editableTokenCatalog.length}</strong>{t("resources.tokensFact")}</span>
        <span><strong>3</strong>{t("resources.packagesFact")}</span>
      </section>

      <section className="resource-grid" aria-label={t("resources.libraryLabel")}>
        {resources.map((resource) => {
          const Icon = resource.icon;
          return (
            <a href={resource.href} key={resource.title} rel="noreferrer" target="_blank">
              <span className="resource-card-icon"><Icon aria-hidden="true" size={19} /></span>
              <span>
                <strong>{t(resource.title)}</strong>
                <small>{t(resource.description)}</small>
              </span>
              <ArrowUpRight aria-hidden="true" size={16} />
            </a>
          );
        })}
      </section>

      <section className="resource-boundary-panel">
        <div>
          <span className="page-kicker">{t("resources.boundaryKicker")}</span>
          <h2>{t("resources.boundaryTitle")}</h2>
        </div>
        <p>{t("resources.boundaryDescription")}</p>
      </section>
    </main>
  );
}
