import { useEffect, useState } from "react";
import { api, errorText, onNotebookProgress, pickFolder, pickSavePath } from "../api/client";
import type { NotebookDiff, NotebookKind, NotebookView, SnapshotSummary } from "../api/types";
import { useI18n, type Key } from "../i18n";
import { localizeBackend } from "../i18n/backend";
import { formatBytes, formatDate } from "../lib/format";

const KINDS: NotebookKind[] = ["pc_folder", "console_usb"];

export function NotebookPage() {
  const { t, lang } = useI18n();
  const local = (msg: string) => localizeBackend(msg, t, lang);
  const [nb, setNb] = useState<NotebookView>({ snapshots: [], note: null });
  const [kind, setKind] = useState<NotebookKind>("pc_folder");
  const [label, setLabel] = useState("");
  const [note, setNote] = useState("");
  const [scanning, setScanning] = useState(false);
  const [count, setCount] = useState(0);
  const [redact, setRedact] = useState(true);
  const [older, setOlder] = useState("");
  const [newer, setNewer] = useState("");
  const [diff, setDiff] = useState<NotebookDiff | null>(null);
  const [message, setMessage] = useState<{ tone: "ok" | "danger"; text: string } | null>(null);

  useEffect(() => {
    api.notebookList().then(setNb, (e) => setMessage({ tone: "danger", text: local(errorText(e)) }));
    const p = onNotebookProgress(setCount);
    return () => {
      p.then((un) => un());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fail = (e: unknown) => setMessage({ tone: "danger", text: local(errorText(e)) });

  const scan = async () => {
    setMessage(null);
    const path = await pickFolder(t("notebook.choose"));
    if (!path) return;
    setScanning(true);
    setCount(0);
    try {
      setNb(await api.notebookScan(path, label, note, kind));
      setLabel("");
      setNote("");
    } catch (e) {
      fail(e);
    } finally {
      setScanning(false);
    }
  };

  const remove = async (id: string) => {
    try {
      setNb(await api.notebookDelete(id));
      setDiff(null);
      if (older === id) setOlder("");
      if (newer === id) setNewer("");
    } catch (e) {
      fail(e);
    }
  };

  const exportAs = async (s: SnapshotSummary, format: "json" | "csv") => {
    setMessage(null);
    const safe = s.label.replace(/[^\p{L}\p{N}_-]+/gu, "_").slice(0, 40) || "snapshot";
    const path = await pickSavePath(`${safe}.${format}`, format);
    if (!path) return;
    try {
      await api.notebookExport(s.id, path, format, redact);
      setMessage({ tone: "ok", text: t("notebook.exported") });
    } catch (e) {
      fail(e);
    }
  };

  const compare = async () => {
    setMessage(null);
    try {
      setDiff(await api.notebookCompare(older, newer));
    } catch (e) {
      fail(e);
    }
  };

  const a = nb.snapshots.find((s) => s.id === older);
  const b = nb.snapshots.find((s) => s.id === newer);
  const option = (s: SnapshotSummary) => `${s.label} — ${formatDate(s.taken_at)}`;

  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("notebook.title")}</h1>
        <p className="lede">{t("notebook.lede")}</p>
      </header>
      <div className="banner warn">{t("notebook.limits")}</div>
      {nb.note && <div className="banner warn">{local(nb.note)}</div>}

      <section className="panel">
        <h2>{t("notebook.newTitle")}</h2>
        <label className="field">
          <span className="field-label">{t("notebook.kind")}</span>
          <select value={kind} onChange={(e) => setKind(e.target.value as NotebookKind)} disabled={scanning}>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`notebook.kind.${k}` as Key)}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">{t("notebook.label")}</span>
          <input value={label} maxLength={80} onChange={(e) => setLabel(e.target.value)} disabled={scanning} />
        </label>
        <label className="field">
          <span className="field-label">{t("notebook.noteField")}</span>
          <input value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} disabled={scanning} />
        </label>
        <div className="row gap">
          <button className="btn primary" onClick={scan} disabled={scanning}>
            {scanning ? t("notebook.scanning", { count }) : t("notebook.choose")}
          </button>
          {scanning && (
            <button className="btn" onClick={() => api.notebookCancel()}>
              {t("notebook.cancel")}
            </button>
          )}
        </div>
        {message && <div className={`banner ${message.tone}`}>{message.text}</div>}
      </section>

      <section className="panel">
        <h2>{t("notebook.listTitle")}</h2>
        <label className="row gap">
          <input type="checkbox" checked={redact} onChange={(e) => setRedact(e.target.checked)} />
          <span>{t("notebook.redact")}</span>
        </label>
        {nb.snapshots.length === 0 && <p className="muted">{t("notebook.none")}</p>}
        {nb.snapshots.map((s) => (
          <div key={s.id} className="check">
            <div className="check-head">
              <strong>{s.label}</strong>
              <span className="badge">{t(`notebook.kind.${s.kind}` as Key)}</span>
              <span className="muted small">{formatDate(s.taken_at)}</span>
            </div>
            <p className="small">
              {t("notebook.summary", { files: s.file_count, folders: s.dir_count, size: formatBytes(s.total_bytes) })}
            </p>
            {s.note && <p className="small muted">{s.note}</p>}
            {s.truncated && <p className="small">{t("notebook.truncated")}</p>}
            {s.skipped > 0 && <p className="small muted">{t("notebook.skipped", { n: s.skipped })}</p>}
            <div className="row gap">
              <button className="btn tiny" onClick={() => exportAs(s, "json")}>
                {t("notebook.exportJson")}
              </button>
              <button className="btn tiny" onClick={() => exportAs(s, "csv")}>
                {t("notebook.exportCsv")}
              </button>
              <button className="btn tiny" onClick={() => remove(s.id)}>
                {t("notebook.delete")}
              </button>
            </div>
          </div>
        ))}
      </section>

      {nb.snapshots.length >= 2 && (
        <section className="panel" aria-live="polite">
          <h2>{t("notebook.compareTitle")}</h2>
          <div className="row gap">
            <label className="field grow">
              <span className="field-label">{t("notebook.older")}</span>
              <select value={older} onChange={(e) => setOlder(e.target.value)}>
                <option value="">{t("notebook.pick")}</option>
                {nb.snapshots.map((s) => (
                  <option key={s.id} value={s.id}>
                    {option(s)}
                  </option>
                ))}
              </select>
            </label>
            <label className="field grow">
              <span className="field-label">{t("notebook.newer")}</span>
              <select value={newer} onChange={(e) => setNewer(e.target.value)}>
                <option value="">{t("notebook.pick")}</option>
                {nb.snapshots.map((s) => (
                  <option key={s.id} value={s.id}>
                    {option(s)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {a && b && a.kind !== b.kind && <div className="banner warn small">{t("notebook.sameKind")}</div>}
          <button className="btn primary" onClick={compare} disabled={!older || !newer || older === newer}>
            {t("notebook.compare")}
          </button>
          {diff && <DiffView diff={diff} />}
        </section>
      )}
    </div>
  );
}

function DiffView({ diff }: { diff: NotebookDiff }) {
  const { t } = useI18n();
  const none =
    diff.added_total + diff.removed_total + diff.changed_total + diff.moved_total === 0;
  return (
    <div>
      <p>
        {t("notebook.diffSummary", {
          added: diff.added_total,
          removed: diff.removed_total,
          changed: diff.changed_total,
          moved: diff.moved_total,
          unchanged: diff.unchanged,
        })}
      </p>
      {none && <p className="muted">{t("notebook.nothingChanged")}</p>}
      {diff.added.length > 0 && (
        <List title={t("notebook.added")} rows={diff.added.map((e) => `${e.path}${e.dir ? "/" : ""}`)} />
      )}
      {diff.removed.length > 0 && (
        <List title={t("notebook.removed")} rows={diff.removed.map((e) => `${e.path}${e.dir ? "/" : ""}`)} />
      )}
      {diff.changed.length > 0 && (
        <List
          title={t("notebook.changed")}
          rows={diff.changed.map(
            (c) =>
              `${c.path} (${t("notebook.sizeChange", { old: formatBytes(c.old_size), new: formatBytes(c.new_size) })})`,
          )}
        />
      )}
      {diff.moved.length > 0 && (
        <List title={t("notebook.moved")} rows={diff.moved.map((m) => `${m.from} → ${m.to}`)} />
      )}
      {diff.cut && <p className="muted small">{t("notebook.cut", { n: 2000 })}</p>}
    </div>
  );
}

function List({ title, rows }: { title: string; rows: string[] }) {
  return (
    <details open>
      <summary>
        {title} ({rows.length})
      </summary>
      <ul className="plain small mono">
        {rows.map((r, i) => (
          <li key={i} className="break">
            {r}
          </li>
        ))}
      </ul>
    </details>
  );
}
