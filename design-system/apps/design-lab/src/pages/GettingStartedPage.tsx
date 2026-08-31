import { Icon as CatalogIcon, type IconName } from "@bitfun/ui";
import { Blocks, FileCode2, Layers3 } from "lucide-react";
import { useI18n, type MessageKey } from "../i18n";

interface GettingStartedPageProps {
  onNavigate: (target: "components" | "resources" | "tokens") => void;
}

const steps: readonly {
  description: MessageKey;
  icon: typeof Layers3 | IconName;
  title: MessageKey;
}[] = [
  {
    description: "gettingStarted.stepContractDescription",
    icon: Layers3,
    title: "gettingStarted.stepContractTitle",
  },
  {
    description: "gettingStarted.stepThemeDescription",
    icon: "palette",
    title: "gettingStarted.stepThemeTitle",
  },
  {
    description: "gettingStarted.stepComponentDescription",
    icon: Blocks,
    title: "gettingStarted.stepComponentTitle",
  },
];

const contractKeys: readonly MessageKey[] = [
  "gettingStarted.contract1",
  "gettingStarted.contract2",
  "gettingStarted.contract3",
];

export function GettingStartedPage({ onNavigate }: GettingStartedPageProps) {
  const { t } = useI18n();

  return (
    <main className="lab-page lab-page--guide" id="getting-started">
      <header className="guide-hero">
        <span className="page-kicker">{t("gettingStarted.kicker")}</span>
        <h1>{t("gettingStarted.title")}</h1>
        <p>{t("gettingStarted.description")}</p>
        <div className="overview-actions">
          <button
            className="lab-button lab-button--primary"
            onClick={() => onNavigate("components")}
            type="button"
          >
            {t("gettingStarted.browseComponents")}
            <CatalogIcon name="arrow-right" size="md" aria-hidden="true" />
          </button>
          <button className="lab-button" onClick={() => onNavigate("tokens")} type="button">
            {t("gettingStarted.openTokens")}
          </button>
        </div>
      </header>

      <section className="guide-step-grid" aria-label={t("gettingStarted.stepsLabel")}>
        {steps.map((step, index) => {
          const Icon = step.icon;
          return (
            <article key={step.title}>
              <span className="guide-step-number">0{index + 1}</span>
              <span className="guide-step-icon">{typeof Icon === "string" ? <CatalogIcon name={Icon} size="lg" style={{ width: 19, height: 19 }} /> : <Icon aria-hidden="true" size={19} />}</span>
              <h2>{t(step.title)}</h2>
              <p>{t(step.description)}</p>
            </article>
          );
        })}
      </section>

      <section className="guide-contract-panel">
        <div className="guide-contract-copy">
          <span className="page-kicker">{t("gettingStarted.workspaceKicker")}</span>
          <h2>{t("gettingStarted.workspaceTitle")}</h2>
          <p>{t("gettingStarted.workspaceDescription")}</p>
          <ul>
            {contractKeys.map((key) => (
              <li key={key}><CatalogIcon name="check-line" size="lg" aria-hidden="true" style={{ width: 15, height: 15 }} />{t(key)}</li>
            ))}
          </ul>
        </div>
        <div className="guide-code-card">
          <div><FileCode2 aria-hidden="true" size={15} /><span>App.tsx</span></div>
          <pre><code>{`import { Button, ThemeRoot } from "@bitfun/ui";

export function App() {
  return (
    <ThemeRoot colorScheme="light">
      <Button variant="fill">Continue</Button>
    </ThemeRoot>
  );
}`}</code></pre>
        </div>
      </section>

      <section className="guide-next-panel">
        <div>
          <span className="page-kicker">{t("gettingStarted.nextKicker")}</span>
          <h2>{t("gettingStarted.nextTitle")}</h2>
          <p>{t("gettingStarted.nextDescription")}</p>
        </div>
        <button className="lab-button" onClick={() => onNavigate("resources")} type="button">
          {t("gettingStarted.viewResources")}
          <CatalogIcon name="arrow-right" size="lg" aria-hidden="true" style={{ width: 15, height: 15 }} />
        </button>
      </section>
    </main>
  );
}
