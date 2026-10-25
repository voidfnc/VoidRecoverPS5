import { useState } from "react";
import { useApp } from "../App";
import { api, errorText } from "../api/client";
import type { CatalogEntry, Mirror, PackageRecord } from "../api/types";
import { useI18n } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import { formatBuild, formatBytes, formatDate, formatExactBytes, hostOf } from "../lib/format";
import { algorithmLabel, hashMeaning, packageTypeHelp, packageTypeLabel, sourceName, sourceRecord } from "../lib/labels";
import { AvailabilityBadge, CopyButton, ProvenanceBadge, TypeBadge } from "./Badges";

export function EntryDetail({ entry, onClose }: { entry: CatalogEntry; onClose: () => void }) {
  const { view, setView, requestOpen, verifyAgainst } = useApp();
  const { t, tx, lang } = useI18n();
  const [probing, setProbing] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);
  const ev = entry.evidence;
  const hasSony = entry.records.some((r) => r.source_id === "sony-ps5-cdn");
  const conflict = ev.md5_conflict || ev.sha256_conflict || ev.size_conflict;

  const probe = async () => {
    setProbing(true);
    setProbeError(null);
    try {
      setView(await api.probeSony(entry.key));
    } catch (e) {
      setProbeError(errorText(e));
    } finally {
      setProbing(false);
    }
  };

  const alsoItem = (r: CatalogEntry["also_listed_as"][number]) =>
    `${r.firmware_version} ${packageTypeLabel(t, r.package_type).toLowerCase()}${r.variant_label ? ` (${r.variant_label})` : ""}${r.build_date ? `, ${t("common.build", { date: formatBuild(r.build_date) })}` : ""}`;

  return (
    <aside className="detail" aria-label={t("detail.aria", { version: entry.firmware_version })}>
      <div className="detail-head">
        <div>
          <h2 className="mono">{entry.firmware_version}</h2>
          <TypeBadge type={entry.package_type} variant={entry.variant_label} />
          {entry.build_date && (
            <span className="muted small mono">{t("common.build", { date: formatBuild(entry.build_date) })}</span>
          )}
        </div>
        <button className="btn tiny" onClick={onClose} aria-label={t("detail.closeAria")}>
          {t("common.close")}
        </button>
      </div>

      <p className="muted">
        {tx("detail.typeLine", {
          label: packageTypeLabel(t, entry.package_type),
          help: packageTypeHelp(t, entry.package_type),
        })}
      </p>

      {entry.package_type === "reinstall" && <div className="banner danger">{t("detail.reinstallWarn")}</div>}
      {(entry.package_type === "special_variant" || entry.package_type === "unknown") && (
        <div className="banner warn">{t("detail.specialWarn")}</div>
      )}
      {view &&
        view.entries.some(
          (e) =>
            e.key !== entry.key &&
            e.firmware_version === entry.firmware_version &&
            e.package_type === entry.package_type &&
            e.variant_label === entry.variant_label,
        ) && <div className="banner warn">{t("detail.multiBuild", { version: entry.firmware_version })}</div>}
      {entry.also_listed_as.length > 0 && (
        <p className="small">{t("detail.alsoListed", { list: entry.also_listed_as.map(alsoItem).join("; ") })}</p>
      )}
      {conflict && (
        <div className="banner warn">
          {tx("detail.conflict", {
            what: [ev.sha256_conflict && "SHA-256", ev.md5_conflict && "MD5", ev.size_conflict && t("common.sizeWord")]
              .filter(Boolean)
              .join(", "),
          })}
        </div>
      )}

      <section>
        <h3>{t("detail.sizeHeading")}</h3>
        {ev.sizes.length === 0 ? (
          <p className="muted">{t("detail.noSize")}</p>
        ) : (
          <ul className="plain">
            {ev.sizes.map((s, i) => (
              <li key={i}>
                <span className="mono">{s.approximate ? formatBytes(s.bytes, true) : formatExactBytes(s.bytes)}</span>{" "}
                <span className="muted">
                  — {sourceName(view, s.source_id)}
                  {s.approximate ? ` (${t("common.rounded")})` : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h3>{t("detail.checksumsHeading")}</h3>
        {ev.hash_claims.length === 0 ? (
          <p>{t("detail.noHash")}</p>
        ) : (
          <ul className="hashes">
            {ev.hash_claims.map((c) => (
              <li key={c.algorithm + c.value}>
                <div className="hash-line">
                  <span className="chip">{algorithmLabel[c.algorithm]}</span>
                  <code className="break">{c.value}</code>
                  <CopyButton text={c.value} />
                </div>
                <div className="muted small">
                  {t("detail.publishedBy", { names: c.source_ids.map((id) => sourceName(view, id)).join(", ") })}
                </div>
              </li>
            ))}
          </ul>
        )}
        {ev.cross_source.length > 0 && (
          <ul className="plain small">
            {ev.cross_source.map((x, i) => (
              <li key={i}>
                {t("detail.listedByBoth", {
                  alg: algorithmLabel[x.algorithm],
                  a: sourceName(view, x.source_a),
                  b: sourceName(view, x.source_b),
                })}
                {x.independent ? "." : t("detail.notIndependent")}
              </li>
            ))}
          </ul>
        )}
        <p className="muted small">{hashMeaning(t)}</p>
      </section>

      <section>
        <h3>{t("detail.whereHeading")}</h3>
        {entry.records.map((r) => (
          <RecordCard key={r.package_id} record={r} onOpen={requestOpen} />
        ))}
        {hasSony && (
          <div className="row gap">
            <button className="btn" onClick={probe} disabled={probing}>
              {probing ? t("detail.probing") : t("detail.probe")}
            </button>
            <span className="muted small">{t("detail.probeNote")}</span>
          </div>
        )}
        {probeError && <div className="banner danger">{localizeBackend(probeError, t, lang)}</div>}
      </section>

      <div className="detail-actions">
        <button className="btn primary" onClick={() => verifyAgainst(entry.key)}>
          {t("detail.verifyBtn")}
        </button>
      </div>
    </aside>
  );
}

function RecordCard({ record, onOpen }: { record: PackageRecord; onOpen: (url: string, what: string) => void }) {
  const { view } = useApp();
  const { t, lang } = useI18n();
  const src = sourceRecord(view, record.source_id);
  const hashNote = record.hashes.find((h) => h.note)?.note;
  return (
    <div className="record">
      <div className="record-head">
        <strong>{src?.name ?? record.source_id}</strong>
        {src && <ProvenanceBadge type={src.source_type} />}
      </div>
      <dl className="kv">
        <dt>{t("rec.label")}</dt>
        <dd className="mono">{record.raw_package_label}</dd>
        {record.source_section && (
          <>
            <dt>{t("rec.section")}</dt>
            <dd>
              {record.source_section}
              {record.source_section === "Official Firmwares" && (
                <span className="muted small">{t("rec.officialNote")}</span>
              )}
            </dd>
          </>
        )}
        {record.long_version && (
          <>
            <dt>{t("rec.build")}</dt>
            <dd className="mono">{record.long_version}</dd>
          </>
        )}
        <dt>{t("rec.lastRead")}</dt>
        <dd>{formatDate(record.last_checked)}</dd>
      </dl>
      {hashNote && <p className="muted small">{localizeBackend(hashNote, t, lang)}</p>}
      {record.mirrors.length > 0 && (
        <ul className="mirrors">
          {record.mirrors.map((m) => (
            <MirrorRow key={m.url} mirror={m} onOpen={onOpen} />
          ))}
        </ul>
      )}
      {src && (
        <button
          className="link-btn small"
          onClick={() => onOpen(src.source_page, t("open.pageOf", { name: src.name }))}
        >
          {t("rec.openPage", { host: hostOf(src.source_page) })}
        </button>
      )}
    </div>
  );
}

function MirrorRow({ mirror, onOpen }: { mirror: Mirror; onOpen: (url: string, what: string) => void }) {
  const { t, lang } = useI18n();
  const direct = mirror.mode === "direct_https";
  return (
    <li className="mirror">
      <div>
        <strong>{mirror.host}</strong>{" "}
        <span className="muted small">
          {direct ? t("mirror.direct") : t("mirror.browser")}
          {mirror.scope === "folder" ? t("mirror.folder") : ""}
        </span>
        <div>
          <AvailabilityBadge value={mirror.availability} />
          {mirror.availability_note && (
            <span className="muted small"> {localizeBackend(mirror.availability_note, t, lang)}</span>
          )}
          {mirror.probed_at && (
            <span className="muted small">{t("mirror.checked", { date: formatDate(mirror.probed_at) })}</span>
          )}
        </div>
      </div>
      <button
        className="btn tiny"
        disabled={direct && mirror.availability === "unavailable"}
        onClick={() =>
          onOpen(mirror.url, direct ? t("open.fileOn", { host: mirror.host }) : t("open.pageOn", { host: mirror.host }))
        }
      >
        {t("mirror.open")}
      </button>
    </li>
  );
}
