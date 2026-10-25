import { useApp } from "../App";
import { useI18n } from "../i18n";

export function AboutPage() {
  const { view } = useApp();
  const { t } = useI18n();
  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("about.title")}</h1>
        <p className="muted">{t("about.meta", { version: view?.app_version ?? "", catalog: view?.catalog_version ?? "" })}</p>
      </header>

      <section className="panel">
        <h2>{t("about.unofficialTitle")}</h2>
        <p>{t("about.unofficial1")}</p>
        <p>{t("about.unofficial2")}</p>
      </section>

      <section className="panel">
        <h2>{t("about.langTitle")}</h2>
        <p>{t("about.langBody")}</p>
      </section>

      <section className="panel">
        <h2>{t("about.privacyTitle")}</h2>
        <ul className="plain">
          <li>{t("about.privacy1")}</li>
          <li>{t("about.privacy2")}</li>
          <li>{t("about.privacy3")}</li>
          <li>{t("about.privacy4")}</li>
        </ul>
      </section>

      <section className="panel">
        <h2>{t("about.checksumsTitle")}</h2>
        <p>{t("about.checksums1")}</p>
      </section>
    </div>
  );
}
