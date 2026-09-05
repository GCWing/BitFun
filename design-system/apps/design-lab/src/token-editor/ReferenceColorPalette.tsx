import {
  referenceColorCatalog,
  referenceColorScales,
  type ReferenceColorEntry,
  type ReferenceColorScaleName,
} from "@openbitfun/theme-openbitfun/authoring";
import { useI18n } from "../i18n";
import { humanizeTokenSegment } from "./catalog";

const colorScales = Object.entries(referenceColorScales) as readonly [
  ReferenceColorScaleName,
  readonly ReferenceColorEntry[],
][];

export function ReferenceColorPalette() {
  const { t } = useI18n();

  return (
    <section
      aria-labelledby="reference-color-palette-title"
      className="reference-color-palette"
    >
      <header className="reference-color-palette__heading">
        <div>
          <span className="page-kicker">{t("tokens.palette.kicker")}</span>
          <h2 id="reference-color-palette-title">{t("tokens.palette.title")}</h2>
          <p>{t("tokens.palette.description")}</p>
        </div>
        <div className="reference-color-palette__summary">
          <span>{t("tokens.palette.scaleCount", { count: colorScales.length })}</span>
          <span>{t("tokens.palette.stepCount", { count: referenceColorCatalog.length })}</span>
        </div>
      </header>

      <div className="reference-color-palette__scales">
        {colorScales.map(([scale, entries]) => (
          <article className="reference-color-scale" key={scale}>
            <header>
              <h3>{humanizeTokenSegment(scale)}</h3>
              <span>{t("tokens.palette.scaleSteps", { count: entries.length })}</span>
            </header>
            <div
              aria-label={t("tokens.palette.scaleLabel", {
                name: humanizeTokenSegment(scale),
              })}
              className="reference-color-scale__steps"
              role="list"
            >
              {entries.map((entry) => (
                <div
                  aria-label={t("tokens.palette.stepLabel", {
                    name: entry.name,
                    value: entry.value,
                  })}
                  className="reference-color-step"
                  key={entry.name}
                  role="listitem"
                  title={`${entry.name} · ${entry.value}`}
                >
                  <span
                    aria-hidden="true"
                    className="reference-color-step__swatch"
                    style={{ backgroundColor: entry.value }}
                  />
                  <strong>{entry.step}</strong>
                  <code>{entry.value}</code>
                </div>
              ))}
            </div>
          </article>
        ))}
      </div>

      <p className="reference-color-palette__contract">
        {t("tokens.palette.contract")}
      </p>
    </section>
  );
}
