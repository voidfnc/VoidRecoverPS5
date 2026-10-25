import { useState } from "react";
import { useApp } from "../App";
import { CATEGORIES, REPORT_URL, RESOURCES, type ResourceCategory } from "../data/resources";
import { useI18n, type Key } from "../i18n";
import { formatDate } from "../lib/format";

export function CommunityPage() {
  const { requestOpen } = useApp();
  const { t } = useI18n();
  const [cat, setCat] = useState<ResourceCategory | "all">("all");
  const shown = RESOURCES.filter((r) => cat === "all" || r.category === cat);

  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("community.title")}</h1>
        <p className="lede">{t("community.lede")}</p>
      </header>

      <div className="row gap" role="group" aria-label={t("community.title")}>
        <button className={cat === "all" ? "chip active" : "chip"} onClick={() => setCat("all")}>
          {t("community.all")}
        </button>
        {CATEGORIES.map((c) => (
          <button key={c} className={cat === c ? "chip active" : "chip"} onClick={() => setCat(c)}>
            {t(`community.cat.${c}` as Key)}
          </button>
        ))}
      </div>

      {shown.length === 0 && <p className="muted">{t("community.empty")}</p>}
      {shown.map((r) => (
        <section key={r.id} className="panel">
          <div className="row gap between">
            <h2>{r.name}</h2>
            <button className="btn" onClick={() => requestOpen(r.url, r.name)}>
              {t("community.open")}
            </button>
          </div>
          <div className="row gap">
            <span className="badge">{t(`community.cat.${r.category}` as Key)}</span>
            <span className="badge">{t(`community.prov.${r.provenance}` as Key)}</span>
          </div>
          <p>{t(`res.${r.id}.note` as Key)}</p>
          <p className="muted small">
            {t("community.maintainer")}: {r.maintainer} · {t("community.checked", { date: formatDate(r.lastChecked) })}
          </p>
          {r.linkOnly && <p className="muted small">{t("community.linkOnly")}</p>}
        </section>
      ))}

      <button className="link-btn" onClick={() => requestOpen(REPORT_URL, t("community.report"))}>
        {t("community.report")}
      </button>
    </div>
  );
}
