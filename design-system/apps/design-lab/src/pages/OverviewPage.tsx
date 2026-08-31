import { Icon } from "@bitfun/ui";
import designSystemHero from "../assets/design-system-hero.webp";
import { useI18n } from "../i18n";

interface OverviewPageProps {
  onNavigate: (target: "components" | "getting-started" | "resources" | "tokens") => void;
}

export function OverviewPage({ onNavigate }: OverviewPageProps) {
  const { t } = useI18n();

  return (
    <main className="lab-page lab-page--overview" id="overview">
      <section className="overview-hero">
        <div className="overview-copy">
          <span className="page-version">v0.1.0</span>
          <h1>BitFun Design System</h1>
          <p>{t("overview.intro")}</p>
          <div className="overview-actions">
            <button
              className="lab-button lab-button--primary"
              onClick={() => onNavigate("getting-started")}
              type="button"
            >
              {t("overview.getStarted")}
              <Icon name="arrow-right" size="md" aria-hidden="true" />
            </button>
            <button
              className="lab-button"
              onClick={() => onNavigate("tokens")}
              type="button"
            >
              {t("overview.viewTokens")}
            </button>
          </div>
        </div>

        <div className="overview-artwork" aria-hidden="true">
          <img alt="" src={designSystemHero} />
        </div>
      </section>

      <footer className="lab-footer">
        <span>{t("overview.footerBuiltWith")}</span>
        <span>{t("overview.footerLicense")}</span>
      </footer>
    </main>
  );
}
