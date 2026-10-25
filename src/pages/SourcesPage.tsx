import { useApp } from "../App";
import { ProvenanceBadge, StateBadge } from "../components/Badges";
import { useI18n } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import { sourceNote } from "../i18n/lang";
import { formatDate, hostOf } from "../lib/format";
import { sourceName } from "../lib/labels";

export function SourcesPage() {
  const { view, refresh, refreshing, refreshError, requestOpen } = useApp();
  const { t, lang } = useI18n();
  if (!view) return null;
  const local = (msg: string) => localizeBackend(msg, t, lang);

  return (
    <div className="page narrow">
      <header className="page-head row">
        <div>
          <h1>{t("sources.title")}</h1>
          <p className="muted">{t("sources.lede", { catalog: view.catalog_version })}</p>
        </div>
        <button className="btn" onClick={refresh} disabled={refreshing}>
          {refreshing ? t("catalog.refreshing") : t("catalog.refresh")}
        </button>
      </header>
      {refreshError && <div className="banner danger">{local(refreshError)}</div>}
      {view.cache_note && <div className="banner warn">{local(view.cache_note)}</div>}

      <ul className="sources">
        {view.sources.map((s) => {
          const st = view.statuses.find((x) => x.source_id === s.source_id);
          const behind = view.sources_behind.find((b) => b.source_id === s.source_id);
          return (
            <li key={s.source_id} className="source-card">
              <div className="record-head">
                <h2>{s.name}</h2>
                <ProvenanceBadge type={s.source_type} />
              </div>
              <p>{sourceNote(lang, s.source_id, s.notes)}</p>
              {s.derived_from.length > 0 && (
                <p className="muted small">
                  {t(s.derived_from.length > 1 ? "sources.derivedMany" : "sources.derivedOne", {
                    names: s.derived_from.map((d) => sourceName(view, d)).join(t("common.and")),
                  })}
                </p>
              )}
              {st ? (
                <dl className="kv">
                  <dt>{t("sources.status")}</dt>
                  <dd>
                    <StateBadge state={st.state} />
                    {st.changed_since_last_fetch && <span className="chip">{t("sources.changed")}</span>}
                  </dd>
                  <dt>{t("sources.lastRead")}</dt>
                  <dd>{formatDate(st.fetched_at)}</dd>
                  <dt>{t("sources.records")}</dt>
                  <dd>{st.record_count}</dd>
                  {st.final_host && (
                    <>
                      <dt>{t("sources.readFrom")}</dt>
                      <dd className="mono">{st.final_host}</dd>
                    </>
                  )}
                  {st.error && (
                    <>
                      <dt>{t("sources.lastError")}</dt>
                      <dd className="danger-text">{local(st.error)}</dd>
                    </>
                  )}
                </dl>
              ) : (
                <p className="muted small">{t("sources.linkOnly")}</p>
              )}
              {behind && (
                <div className="banner warn small">
                  {t("sources.behind", { newest: behind.newest_version, latest: behind.official_latest })}
                </div>
              )}
              <button className="link-btn small" onClick={() => requestOpen(s.source_page, s.name)}>
                {t("sources.open", { host: hostOf(s.source_page) })}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
