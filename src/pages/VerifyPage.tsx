import { useEffect, useMemo, useState } from "react";
import { useApp } from "../App";
import { api, errorText, onVerifyProgress, pickPupFile } from "../api/client";
import type { EntryCheck, VerifyReport } from "../api/types";
import { CopyButton, TypeBadge } from "../components/Badges";
import { useI18n } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import { formatBuild, formatBytes, formatExactBytes } from "../lib/format";
import { algorithmLabel, hashMeaning, matchStatement, packageTypeShort, sourceName, verdictLabel } from "../lib/labels";

export function VerifyPage() {
  const { view, verifyTarget, verifyAgainst } = useApp();
  const { t, lang } = useI18n();
  const [path, setPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [report, setReport] = useState<VerifyReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const entries = view?.entries ?? [];
  const target = entries.find((e) => e.key === verifyTarget) ?? null;

  useEffect(() => {
    const p = onVerifyProgress((done, total) => setProgress({ done, total }));
    return () => {
      p.then((un) => un());
    };
  }, []);

  // Changing the target re-compares the hashes already computed; no re-read.
  useEffect(() => {
    if (!report) return;
    api.compareLocal(report.file_name, report.local, verifyTarget).then(setReport, (e) => setError(errorText(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [verifyTarget]);

  const choose = async () => {
    setError(null);
    const picked = await pickPupFile(t);
    if (!picked) return;
    setPath(picked);
    setReport(null);
    setBusy(true);
    setProgress(null);
    try {
      setReport(await api.verifyLocalFile(picked, verifyTarget));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const options = useMemo(
    () =>
      entries.map((e) => ({
        key: e.key,
        label: t("verify.entryOption", {
          version: e.firmware_version,
          type: packageTypeShort(t, e.package_type),
          variant: e.variant_label ? ` (${e.variant_label})` : "",
          build: e.build_date ? ` — ${t("common.build", { date: formatBuild(e.build_date) })}` : "",
        }),
      })),
    [entries, t],
  );

  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;

  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("verify.title")}</h1>
        <p className="lede">{t("verify.lede")}</p>
      </header>

      <section className="panel">
        <label className="field">
          <span className="field-label">{t("verify.compareWith")}</span>
          <select value={verifyTarget ?? ""} onChange={(e) => verifyAgainst(e.target.value || null)}>
            <option value="">{t("verify.anyEntry")}</option>
            {options.map((o) => (
              <option key={o.key} value={o.key}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        {target?.package_type === "reinstall" && (
          <div className="banner danger small">{t("verify.reinstallWarn")}</div>
        )}
        <div className="row gap">
          <button className="btn primary" onClick={choose} disabled={busy}>
            {busy ? t("verify.reading") : report ? t("verify.chooseAnother") : t("verify.chooseFile")}
          </button>
          {busy && (
            <button className="btn" onClick={() => api.cancelVerify()}>
              {t("common.cancel")}
            </button>
          )}
        </div>
        {path && <p className="mono small break muted">{path}</p>}
        {busy && (
          <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
            <div className="progress-bar" style={{ width: `${pct}%` }} />
            <span className="progress-text">
              {progress
                ? t("common.progressOf", { done: formatBytes(progress.done), total: formatBytes(progress.total) })
                : t("verify.starting")}
            </span>
          </div>
        )}
        {error && <div className="banner danger">{localizeBackend(error, t, lang)}</div>}
      </section>

      {report && <Results report={report} />}
    </div>
  );
}

function Results({ report }: { report: VerifyReport }) {
  const { t } = useI18n();
  const { local } = report;
  return (
    <section className="panel" aria-live="polite">
      <h2>{report.file_name}</h2>
      <dl className="kv">
        <dt>{t("verify.size")}</dt>
        <dd className="mono">{formatExactBytes(local.size_bytes)}</dd>
        <dt>SHA-256</dt>
        <dd className="hash-line">
          <code className="break">{local.sha256}</code>
          <CopyButton text={local.sha256} />
        </dd>
        <dt>MD5</dt>
        <dd className="hash-line">
          <code className="break">{local.md5}</code>
          <CopyButton text={local.md5} />
        </dd>
      </dl>
      {report.exceeds_fat32_limit && <div className="banner danger">{t("verify.fat32Warn")}</div>}

      {report.selected && (
        <>
          <h3>{t("verify.selectedHeading")}</h3>
          <CheckCard check={report.selected} />
        </>
      )}

      <h3>{t("verify.matchingHeading")}</h3>
      {report.matching_entries.length === 0 ? (
        <div className="banner warn">{t("verify.noMatch")}</div>
      ) : (
        report.matching_entries.map((c) => <CheckCard key={c.key} check={c} />)
      )}

      <p className="muted small">{hashMeaning(t)}</p>
    </section>
  );
}

function CheckCard({ check }: { check: EntryCheck }) {
  const { view } = useApp();
  const { t } = useI18n();
  const tone = check.verdict === "hash_matches" ? "ok" : check.verdict === "hash_mismatch" ? "danger" : "warn";
  const sc = check.size_check;
  const sizeKey = sc
    ? sc.outcome === "match"
      ? sc.approximate
        ? "verify.sizeMatchesRounded"
        : "verify.sizeMatches"
      : sc.approximate
        ? "verify.sizeDiffersRounded"
        : "verify.sizeDiffers"
    : null;
  return (
    <div className={`check ${tone}`}>
      <div className="check-head">
        <span className="mono">{check.firmware_version}</span>
        <TypeBadge type={check.package_type} variant={check.variant_label} />
        <strong>{verdictLabel(t, check.verdict)}</strong>
      </div>
      <ul className="plain small">
        {check.hash_checks.map((h, i) => (
          <li key={i}>
            {h.outcome === "match"
              ? matchStatement(t, view, h.algorithm, h.source_ids)
              : t("verify.hashDiffers", {
                  alg: algorithmLabel[h.algorithm],
                  names: h.source_ids.map((id) => sourceName(view, id)).join(", "),
                  expected: h.expected,
                })}
          </li>
        ))}
        {sc && sizeKey && <li>{t(sizeKey, { size: formatBytes(sc.expected_bytes, sc.approximate) })}</li>}
      </ul>
      {check.verdict === "hash_mismatch" && <p className="small">{t("verify.mismatchHelp")}</p>}
    </div>
  );
}
