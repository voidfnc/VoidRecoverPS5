import { useCallback, useEffect, useMemo, useState } from "react";
import { useApp } from "../App";
import { api, errorText, onBuildProgress, onDownloadProgress, pickPupFile } from "../api/client";
import type { BuildPhase, BuildView, CatalogEntry, LeftoverGroup, PreflightView, VolumeInfo, VolumeView } from "../api/types";
import { TypeBadge } from "../components/Badges";
import { useI18n, type Key } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import { formatBuild, formatBytes } from "../lib/format";
import { packageTypeShort } from "../lib/labels";

const SONY_SOURCE_ID = "sony-ps5-cdn";

const PHASE_KEY: Record<BuildPhase, Key> = {
  writing: "build.phaseWriting",
  checking_temp: "build.phaseCheckingTemp",
  checking_final: "build.phaseCheckingFinal",
};

interface Source {
  path: string;
  name: string;
  /** Set when the app downloaded and checked the file itself. */
  downloaded: boolean;
}

function isBuildable(e: CatalogEntry): boolean {
  return (e.package_type === "update" || e.package_type === "reinstall") && !e.variant_label;
}

function sonyDownloadable(e: CatalogEntry): boolean {
  return e.records.some(
    (r) =>
      r.source_id === SONY_SOURCE_ID &&
      r.mirrors.some((m) => m.mode === "direct_https" && m.availability !== "unavailable"),
  );
}

export function BuildPage() {
  const { view, navigate, buildTarget } = useApp();
  const { t, tx, lang } = useI18n();
  const [key, setKey] = useState(buildTarget ?? "");
  const [source, setSource] = useState<Source | null>(null);
  const [volumes, setVolumes] = useState<VolumeView[] | null>(null);
  const [volumeId, setVolumeId] = useState<string | null>(null);
  const [pre, setPre] = useState<PreflightView | null>(null);
  const [replace, setReplace] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState<null | "download" | "build">(null);
  const [progress, setProgress] = useState<{ label: Key; done: number; total: number } | null>(null);
  const [result, setResult] = useState<BuildView | null>(null);
  const [builtOn, setBuiltOn] = useState<VolumeInfo | null>(null);
  const [eject, setEject] = useState<"idle" | "busy" | "done" | string>("idle");
  const [error, setError] = useState<string | null>(null);
  const [leftovers, setLeftovers] = useState<LeftoverGroup[]>([]);

  const local = (msg: string) => localizeBackend(msg, t, lang);
  const entryLabel = (e: CatalogEntry) =>
    t("build.entryOption", {
      version: e.firmware_version,
      type: packageTypeShort(t, e.package_type),
      build: e.build_date ? ` — ${t("common.build", { date: formatBuild(e.build_date) })}` : "",
    });

  const entries = useMemo(() => (view?.entries ?? []).filter(isBuildable), [view]);
  const entry = entries.find((e) => e.key === key) ?? null;

  const loadVolumes = useCallback(async () => {
    try {
      const list = await api.listVolumes();
      setVolumes(list);
      setVolumeId((cur) => (cur && list.some((v) => v.id === cur && v.blockers.length === 0) ? cur : null));
    } catch (e) {
      setVolumes([]);
      setError(errorText(e));
    }
  }, []);

  const loadLeftovers = useCallback(() => {
    api.scanLeftovers().then(setLeftovers, () => setLeftovers([]));
  }, []);

  useEffect(() => {
    loadVolumes();
    loadLeftovers();
  }, [loadVolumes, loadLeftovers]);

  useEffect(() => {
    const d = onDownloadProgress((done, total) => setProgress({ label: "build.phaseDownloading", done, total }));
    const b = onBuildProgress((phase, done, total) => setProgress({ label: PHASE_KEY[phase], done, total }));
    return () => {
      d.then((un) => un());
      b.then((un) => un());
    };
  }, []);

  // Facts shown before writing come from the backend, never from this page.
  useEffect(() => {
    setPre(null);
    setConfirmed(false);
    setReplace(false);
    if (!entry || !source || !volumeId) return;
    let live = true;
    api.preflightBuild(entry.key, source.path, volumeId).then(
      (p) => live && setPre(p),
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [entry, source, volumeId]);

  const chooseEntry = (k: string) => {
    setKey(k);
    setSource(null);
    setResult(null);
    setError(null);
  };

  const download = async () => {
    if (!entry) return;
    setError(null);
    setBusy("download");
    setProgress(null);
    try {
      const d = await api.downloadPackage(entry.key);
      setSource({ path: d.path, name: d.path.split(/[\\/]/).pop() ?? d.path, downloaded: true });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const pick = async () => {
    setError(null);
    const p = await pickPupFile(t);
    if (p) setSource({ path: p, name: p.split(/[\\/]/).pop() ?? p, downloaded: false });
  };

  const build = async () => {
    if (!entry || !source || !pre) return;
    setError(null);
    setBusy("build");
    setProgress(null);
    try {
      setResult(await api.buildUsb(entry.key, source.path, pre.volume, replace));
      setBuiltOn(pre.volume);
      setEject("idle");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
      setProgress(null);
      loadVolumes();
    }
  };

  const resolve = async (g: LeftoverGroup, file: string, action: "delete" | "restore") => {
    setError(null);
    try {
      await api.resolveLeftover(g.volume_id, file, action);
    } catch (e) {
      setError(errorText(e));
    }
    loadLeftovers();
  };

  const blocked = (pre?.preflight.blockers.length ?? 0) > 0;
  const needsReplace = !!pre?.preflight.target_exists;
  const ready = !!pre && !blocked && confirmed && (!needsReplace || replace) && busy === null;
  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;

  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("build.title")}</h1>
        <p className="lede">{tx("build.lede")}</p>
      </header>

      {leftovers.map((g) => (
        <div key={g.volume_id} className="banner warn" role="alert">
          <strong>
            {t("build.leftoverTitle", {
              drive: g.volume_label || t("build.leftoverDriveFallback"),
              mount: g.mount_path,
            })}
          </strong>
          {g.leftovers.map((l) => (
            <div key={l.file_name} className="row gap">
              <span className="mono small">{l.file_name}</span>
              <span className="small">
                {l.kind === "backup" ? t("build.leftoverBackup") : t("build.leftoverTemp")} · {formatBytes(l.size)}
              </span>
              {l.kind === "backup" && !l.final_exists && (
                <button className="btn tiny" onClick={() => resolve(g, l.file_name, "restore")}>
                  {t("build.restore")}
                </button>
              )}
              <button className="btn tiny" onClick={() => resolve(g, l.file_name, "delete")}>
                {t("build.delete")}
              </button>
            </div>
          ))}
        </div>
      ))}

      {!view || entries.length === 0 ? (
        <div className="banner warn">
          {t("build.emptyCatalog")}{" "}
          <button className="btn tiny" onClick={() => navigate("home")}>
            {t("build.loadFromHome")}
          </button>
        </div>
      ) : (
        <>
          <section className="panel">
            <h2>{t("build.step1")}</h2>
            <label className="field">
              <span className="field-label">{t("build.whichFile")}</span>
              <select value={key} onChange={(e) => chooseEntry(e.target.value)} disabled={busy !== null}>
                <option value="">{t("build.choosePackage")}</option>
                {entries.map((e) => (
                  <option key={e.key} value={e.key}>
                    {entryLabel(e)}
                  </option>
                ))}
              </select>
            </label>
            {entry && (
              <div className="row gap">
                <TypeBadge type={entry.package_type} variant={entry.variant_label} />
                {entry.package_type === "reinstall" && <span className="small">{t("build.reinstallWarn")}</span>}
              </div>
            )}
            <p className="muted small">{t("build.packageNote")}</p>
          </section>

          {entry && (
            <section className="panel">
              <h2>{t("build.step2")}</h2>
              <div className="row gap">
                {sonyDownloadable(entry) && (
                  <button className="btn primary" onClick={download} disabled={busy !== null}>
                    {busy === "download" ? t("build.downloading") : t("build.download")}
                  </button>
                )}
                {busy === "download" && (
                  <button className="btn" onClick={() => api.cancelDownload()}>
                    {t("common.cancel")}
                  </button>
                )}
                <button className="btn" onClick={pick} disabled={busy !== null}>
                  {t("build.haveFile")}
                </button>
              </div>
              {!sonyDownloadable(entry) && <p className="muted small">{t("build.noSonyDownload")}</p>}
              {source && (
                <p className="small">
                  <span className="mono break">{source.name}</span>
                  {source.downloaded ? t("build.fileDownloaded") : t("build.filePicked")}
                </p>
              )}
            </section>
          )}

          {entry && source && (
            <section className="panel">
              <div className="row gap between">
                <h2>{t("build.step3")}</h2>
                <button className="btn tiny" onClick={loadVolumes} disabled={busy !== null}>
                  {t("build.refreshDrives")}
                </button>
              </div>
              {volumes === null ? (
                <p className="muted">{t("build.lookingDrives")}</p>
              ) : volumes.length === 0 ? (
                <div className="banner warn">{t("build.noDrives")}</div>
              ) : (
                <ul className="plain drives">
                  {volumes.map((v) => (
                    <li key={v.id} className={`drive ${v.blockers.length ? "blocked" : ""}`}>
                      <label className="row gap">
                        <input
                          type="radio"
                          name="drive"
                          checked={volumeId === v.id}
                          disabled={v.blockers.length > 0 || busy !== null}
                          onChange={() => setVolumeId(v.id)}
                        />
                        <span>
                          <strong>{v.label || t("build.noLabel")}</strong> <span className="mono">{v.mount_path}</span> ·{" "}
                          {v.filesystem || t("build.unknownFs")} ·{" "}
                          {t("build.driveSpace", { total: formatBytes(v.total_bytes), free: formatBytes(v.free_bytes) })}
                        </span>
                      </label>
                      {v.blockers.map((b) => (
                        <div key={b} className="small muted">
                          {t("build.notUsable", { reason: local(b) })}
                        </div>
                      ))}
                    </li>
                  ))}
                </ul>
              )}
              <p className="muted small">{t("build.driveNote")}</p>
            </section>
          )}

          {pre && (
            <section className="panel">
              <h2>{t("build.step4")}</h2>
              <dl className="kv">
                <dt>{t("build.rowPackage")}</dt>
                <dd>{entry && entryLabel(entry)}</dd>
                <dt>{t("build.rowFile")}</dt>
                <dd className="mono break">
                  {pre.source_name} ({formatBytes(pre.source_size)})
                </dd>
                <dt>{t("build.rowDrive")}</dt>
                <dd>
                  {pre.volume.label || t("build.noLabel")} <span className="mono">{pre.volume.mount_path}</span> ·{" "}
                  {pre.volume.filesystem} · {t("build.driveSpace", { total: formatBytes(pre.volume.total_bytes), free: formatBytes(pre.volume.free_bytes) })}
                </dd>
                <dt>{t("build.rowTarget")}</dt>
                <dd className="mono">
                  {pre.volume.mount_path}
                  {pre.preflight.target_path}
                </dd>
                <dt>{t("build.rowSpace")}</dt>
                <dd>{t("build.spaceValue", { size: formatBytes(pre.preflight.bytes_needed) })}</dd>
              </dl>

              {pre.preflight.blockers.map((b) => (
                <div key={b} className="banner danger">
                  {local(b)}
                </div>
              ))}
              {pre.preflight.target_exists && (
                <div className="banner warn">
                  {t("build.existsWarn", { size: formatBytes(pre.preflight.target_size) })}
                  <label className="row gap">
                    <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} />{" "}
                    {t("build.replaceCheck")}
                  </label>
                </div>
              )}
              {pre.preflight.other_pups.length > 0 && (
                <div className="banner warn">{t("build.otherPups", { names: pre.preflight.other_pups.join(", ") })}</div>
              )}

              {!blocked && (
                <>
                  <label className="row gap">
                    <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} />
                    <span>{t("build.confirm", { mount: pre.volume.mount_path })}</span>
                  </label>
                  <div className="row gap">
                    <button className="btn primary" onClick={build} disabled={!ready}>
                      {busy === "build" ? t("build.writing") : t("build.write")}
                    </button>
                    {busy === "build" && (
                      <button className="btn" onClick={() => api.cancelBuild()}>
                        {t("common.cancel")}
                      </button>
                    )}
                  </div>
                </>
              )}
            </section>
          )}

          {busy && progress && (
            <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
              <div className="progress-bar" style={{ width: `${pct}%` }} />
              <span className="progress-text">
                {t(progress.label)}:{" "}
                {t("common.progressOf", { done: formatBytes(progress.done), total: formatBytes(progress.total) })}
              </span>
            </div>
          )}
          {error && <div className="banner danger">{local(error)}</div>}

          {result && (
            <section className="panel" aria-live="polite">
              <h2>{t("build.doneTitle")}</h2>
              <div className="check ok">
                {tx("build.doneBody", { target: result.target })}
                {result.replaced_existing && t("build.doneReplaced")}
              </div>
              <ol>
                <li>{t("build.doneStep1b")}</li>
                <li>{t("build.doneStep2")}</li>
                <li>{t("build.doneStep3")}</li>
              </ol>
              {builtOn && (
                <div className="row gap">
                  <button
                    className="btn"
                    disabled={eject === "busy" || eject === "done"}
                    onClick={async () => {
                      setEject("busy");
                      try {
                        await api.ejectVolume(builtOn);
                        setEject("done");
                      } catch (e) {
                        setEject(errorText(e));
                      }
                    }}
                  >
                    {eject === "busy" ? t("build.ejecting") : t("build.eject")}
                  </button>
                  {eject === "done" && <span className="small">{t("build.ejected")}</span>}
                  {eject !== "idle" && eject !== "busy" && eject !== "done" && (
                    <span className="small">
                      {local(eject)} {t("build.ejectFallback")}
                    </span>
                  )}
                </div>
              )}
            </section>
          )}
        </>
      )}
    </div>
  );
}
