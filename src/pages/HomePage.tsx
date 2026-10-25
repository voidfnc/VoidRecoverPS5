import { useApp } from "../App";
import { useI18n } from "../i18n";
import { formatDate } from "../lib/format";
import { sourceName } from "../lib/labels";

export function HomePage() {
  const { view, refresh, refreshing, refreshError, navigate } = useApp();
  const { t, tx } = useI18n();
  const loaded = (view?.entries.length ?? 0) > 0;
  const latest = view?.official_latest;

  return (
    <div className="page">
      <header className="page-head">
        <h1>{t("home.title")}</h1>
        <p className="lede">{t("home.lede")}</p>
      </header>

      {!loaded && (
        <section className="callout" aria-live="polite">
          <h2>{t("home.loadTitle")}</h2>
          <p>{t("home.loadBody")}</p>
          <button className="btn primary" onClick={refresh} disabled={refreshing}>
            {refreshing ? t("home.loading") : t("home.loadBtn")}
          </button>
          {refreshError && <div className="banner danger">{refreshError}</div>}
        </section>
      )}

      {loaded && (
        <section className="stats" aria-label={t("home.statsAria")}>
          <div className="stat">
            <div className="stat-label">{t("home.statLatest")}</div>
            <div className="stat-value">
              {latest?.version ?? (latest ? t("common.build", { date: latest.build_date }) : "—")}
            </div>
          </div>
          <div className="stat">
            <div className="stat-label">{t("home.statEntries")}</div>
            <div className="stat-value">{view!.entries.length}</div>
          </div>
          <div className="stat">
            <div className="stat-label">{t("home.statRefreshed")}</div>
            <div className="stat-value small">{formatDate(view!.saved_at)}</div>
          </div>
          <div className="stat">
            <div className="stat-label">{t("home.statFresh")}</div>
            <div className="stat-value">
              {view!.statuses.filter((s) => s.state === "fresh").length} / {view!.statuses.length}
            </div>
          </div>
        </section>
      )}

      {loaded && view!.sources_behind.length > 0 && (
        <div className="banner warn">
          {view!.sources_behind.map((b) => (
            <div key={b.source_id}>
              {t("home.behind", {
                source: sourceName(view, b.source_id),
                newest: b.newest_version,
                latest: b.official_latest,
              })}
            </div>
          ))}
        </div>
      )}

      <section className="actions" aria-label={t("home.actionsAria")}>
        <button className="action" onClick={() => navigate("catalog")}>
          <span className="action-title">{t("home.actionCatalogTitle")}</span>
          <span className="action-text">{t("home.actionCatalogText")}</span>
        </button>
        <button className="action" onClick={() => navigate("verify")}>
          <span className="action-title">{t("home.actionVerifyTitle")}</span>
          <span className="action-text">{t("home.actionVerifyText")}</span>
        </button>
        <button className="action" onClick={() => navigate("sources")}>
          <span className="action-title">{t("home.actionSourcesTitle")}</span>
          <span className="action-text">{t("home.actionSourcesText")}</span>
        </button>
        <button className="action" onClick={() => navigate("build")}>
          <span className="action-title">{t("home.actionBuildTitle")}</span>
          <span className="action-text">{t("home.actionBuildText")}</span>
        </button>
        <button className="action" onClick={() => navigate("wizard")}>
          <span className="action-title">{t("home.actionTroubleTitle")}</span>
          <span className="action-text">{t("home.actionTroubleText")}</span>
        </button>
      </section>

      <aside className="note">{tx("home.note")}</aside>
    </div>
  );
}
