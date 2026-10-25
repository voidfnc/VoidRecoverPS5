import { useMemo, useState } from "react";
import { useApp } from "../App";
import { useI18n } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import type { CatalogEntry, PackageType } from "../api/types";
import { AvailabilityBadge, TypeBadge } from "../components/Badges";
import { EntryDetail } from "../components/EntryDetail";
import { applyFilter, defaultFilter, hasConflict, type CatalogFilter, type SortKey } from "../lib/filter";
import { formatBuild, formatBytes } from "../lib/format";
import { packageTypeShort } from "../lib/labels";

const TYPES: PackageType[] = ["update", "reinstall", "special_variant", "unknown"];

export function CatalogPage() {
  const { view, refresh, refreshing, refreshError } = useApp();
  const { t, lang } = useI18n();
  const [filter, setFilter] = useState<CatalogFilter>(defaultFilter);
  const [selected, setSelected] = useState<string | null>(null);

  const entries = view?.entries ?? [];
  const shown = useMemo(() => applyFilter(entries, filter), [entries, filter]);
  const selectedEntry = entries.find((e) => e.key === selected) ?? null;
  const set = (patch: Partial<CatalogFilter>) => setFilter((f) => ({ ...f, ...patch }));
  const adapterSources = view?.sources.filter((s) => s.adapter) ?? [];

  return (
    <div className={selectedEntry ? "page with-detail" : "page"}>
      <div className="catalog-main">
        <header className="page-head row">
          <div>
            <h1>{t("catalog.title")}</h1>
            <p className="muted">
              {t("catalog.count", { shown: shown.length, total: entries.length })}
            </p>
          </div>
          <button className="btn" onClick={refresh} disabled={refreshing}>
            {refreshing ? t("catalog.refreshing") : t("catalog.refresh")}
          </button>
        </header>
        {refreshError && <div className="banner danger">{localizeBackend(refreshError, t, lang)}</div>}

        <div className="toolbar" role="search">
          <label className="field grow">
            <span className="field-label">{t("catalog.search")}</span>
            <input
              type="search"
              placeholder={t("catalog.searchPh")}
              value={filter.query}
              onChange={(e) => set({ query: e.target.value })}
            />
          </label>
          <fieldset className="field toggles">
            <legend className="field-label">{t("catalog.pkgType")}</legend>
            {TYPES.map((pt) => (
              <label key={pt} className="toggle">
                <input
                  type="checkbox"
                  checked={filter.types[pt]}
                  onChange={(e) => set({ types: { ...filter.types, [pt]: e.target.checked } })}
                />
                {packageTypeShort(t, pt)}
              </label>
            ))}
          </fieldset>
          <label className="field">
            <span className="field-label">{t("catalog.source")}</span>
            <select value={filter.sourceId} onChange={(e) => set({ sourceId: e.target.value })}>
              <option value="all">{t("catalog.allSources")}</option>
              {adapterSources.map((s) => (
                <option key={s.source_id} value={s.source_id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">{t("catalog.sortBy")}</span>
            <select
              value={`${filter.sort}:${filter.descending ? "d" : "a"}`}
              onChange={(e) => {
                const [sort, dir] = e.target.value.split(":");
                set({ sort: sort as SortKey, descending: dir === "d" });
              }}
            >
              <option value="version:d">{t("catalog.sortVersionDesc")}</option>
              <option value="version:a">{t("catalog.sortVersionAsc")}</option>
              <option value="sources:d">{t("catalog.sortSources")}</option>
              <option value="hashes:d">{t("catalog.sortHashes")}</option>
              <option value="size:d">{t("catalog.sortSize")}</option>
            </select>
          </label>
          <div className="field toggles">
            <span className="field-label">{t("catalog.showOnly")}</span>
            <label className="toggle">
              <input type="checkbox" checked={filter.onlyWithSony} onChange={(e) => set({ onlyWithSony: e.target.checked })} />
              {t("catalog.onlySony")}
            </label>
            <label className="toggle">
              <input type="checkbox" checked={filter.onlyConflicts} onChange={(e) => set({ onlyConflicts: e.target.checked })} />
              {t("catalog.onlyConflicts")}
            </label>
          </div>
        </div>

        {entries.length === 0 ? (
          <div className="empty">
            <p>{t("catalog.empty")}</p>
            <button className="btn primary" onClick={refresh} disabled={refreshing}>
              {refreshing ? t("catalog.loading") : t("catalog.load")}
            </button>
          </div>
        ) : shown.length === 0 ? (
          <div className="empty">
            <p>{t("catalog.noMatch")}</p>
            <button className="btn" onClick={() => setFilter(defaultFilter)}>
              {t("catalog.clear")}
            </button>
          </div>
        ) : (
          <div className="table-wrap">
            <table className="catalog">
              <thead>
                <tr>
                  <th scope="col">{t("catalog.colVersion")}</th>
                  <th scope="col">{t("catalog.colType")}</th>
                  <th scope="col" className="col-labels">{t("catalog.colLabels")}</th>
                  <th scope="col">{t("catalog.colSize")}</th>
                  <th scope="col">{t("catalog.colChecksums")}</th>
                  <th scope="col">{t("catalog.colSony")}</th>
                </tr>
              </thead>
              <tbody>
                {shown.map((e) => (
                  <Row key={e.key} entry={e} selected={e.key === selected} onSelect={() => setSelected(e.key)} />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {selectedEntry && <EntryDetail entry={selectedEntry} onClose={() => setSelected(null)} />}
    </div>
  );
}

function Row({ entry, selected, onSelect }: { entry: CatalogEntry; selected: boolean; onSelect: () => void }) {
  const { t } = useI18n();
  const ev = entry.evidence;
  const algs = new Set(ev.hash_claims.map((c) => c.algorithm));
  const labels = [...new Set(entry.records.filter((r) => r.source_id !== "sony-ps5-cdn").map((r) => r.raw_package_label))];
  return (
    <tr className={selected ? "selected" : undefined}>
      <td>
        <button className="link-btn mono" onClick={onSelect} aria-label={t("catalog.detailsFor", { version: entry.firmware_version, type: packageTypeShort(t, entry.package_type) })}>
          {entry.firmware_version}
        </button>
        {entry.build_date && <div className="muted small mono">{t("common.build", { date: formatBuild(entry.build_date) })}</div>}
      </td>
      <td>
        <TypeBadge type={entry.package_type} variant={entry.variant_label} />
      </td>
      <td className="labels col-labels" title={labels.join("\n")}>
        {labels[0] ?? "—"}
        {labels.length > 1 && <span className="muted"> +{labels.length - 1}</span>}
      </td>
      <td className="mono">{formatBytes(ev.best_size_bytes, ev.best_size_is_approximate)}</td>
      <td>
        {algs.has("sha256") && <span className="chip">SHA-256</span>}
        {algs.has("md5") && <span className="chip">MD5</span>}
        {algs.size === 0 && <span className="muted">{t("catalog.noHashes")}</span>}
        {hasConflict(entry) && <span className="chip warn">{t("catalog.conflictChip")}</span>}
      </td>
      <td>{ev.sony_availability ? <AvailabilityBadge value={ev.sony_availability} /> : <span className="muted">—</span>}</td>
    </tr>
  );
}
