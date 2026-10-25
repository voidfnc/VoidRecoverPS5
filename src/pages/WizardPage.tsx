import { useMemo, useState } from "react";
import { useApp } from "../App";
import { useI18n, type Key } from "../i18n";
import { formatBuild } from "../lib/format";
import { buildPlan, candidateEntries, type Answers, type Priority, type Symptom } from "../wizard/logic";

const REVIEWED = "2026-10-25";

const SYMPTOMS: Symptom[] = ["boots", "storage", "safe_mode", "loop", "asks_file"];
const PRIORITIES: Priority[] = ["keep_data", "working", "preserve"];

const SOURCES: { key: Key; url: string }[] = [
  { key: "wiz.src.sony", url: "https://www.playstation.com/en-us/support/hardware/safe-mode-playstation/" },
  { key: "wiz.src.psdevwiki", url: "https://www.psdevwiki.com/ps5/" },
  { key: "wiz.src.y2jb", url: "https://github.com/Gezine/Y2JB" },
  {
    key: "wiz.src.guide",
    url: "https://gaminglikeaboss.com/guides/unbricking-your-ps5-how-to-recover-from-a-failed-13-60-jailbreak-crash/",
  },
];

const initial: Answers = { symptom: "safe_mode", priority: "working", knownVersion: "", minVersion: "" };

export function WizardPage() {
  const { view, navigate, buildFor, requestOpen } = useApp();
  const { t, tx } = useI18n();
  const [answers, setAnswers] = useState<Answers>(initial);
  const [shown, setShown] = useState(false);

  const plan = useMemo(() => buildPlan(answers), [answers]);
  const candidates = useMemo(() => candidateEntries(view?.entries ?? [], plan.proposal), [view, plan]);
  const set = (patch: Partial<Answers>) => {
    setAnswers((a) => ({ ...a, ...patch }));
    setShown(false);
  };

  return (
    <div className="page narrow">
      <header className="page-head">
        <h1>{t("wiz.title")}</h1>
        <p className="lede">{t("wiz.lede")}</p>
      </header>
      <div className="banner warn">{t("wiz.cannot")}</div>

      <section className="panel">
        <fieldset className="field">
          <legend className="field-label">{t("wiz.q.symptom")}</legend>
          {SYMPTOMS.map((s) => (
            <label key={s} className="row gap">
              <input
                type="radio"
                name="symptom"
                checked={answers.symptom === s}
                onChange={() => set({ symptom: s })}
              />
              <span>{t(`wiz.sym.${s}` as Key)}</span>
            </label>
          ))}
        </fieldset>

        <fieldset className="field">
          <legend className="field-label">{t("wiz.q.priority")}</legend>
          {PRIORITIES.map((p) => (
            <label key={p} className="row gap">
              <input
                type="radio"
                name="priority"
                checked={answers.priority === p}
                onChange={() => set({ priority: p })}
              />
              <span>{t(`wiz.pri.${p}` as Key)}</span>
            </label>
          ))}
          {answers.priority === "preserve" && <p className="muted small">{t("wiz.pri.preserveHelp")}</p>}
        </fieldset>

        <label className="field">
          <span className="field-label">{t("wiz.q.version")}</span>
          <input
            className="mono"
            value={answers.knownVersion}
            placeholder="13.60"
            onChange={(e) => set({ knownVersion: e.target.value })}
          />
          <span className="muted small">{t("wiz.q.versionHelp")}</span>
        </label>
        <label className="field">
          <span className="field-label">{t("wiz.q.min")}</span>
          <input
            className="mono"
            value={answers.minVersion}
            placeholder="13.60"
            onChange={(e) => set({ minVersion: e.target.value })}
          />
          <span className="muted small">{t("wiz.q.minHelp")}</span>
        </label>

        <div className="row gap">
          <button className="btn primary" onClick={() => setShown(true)}>
            {t("wiz.show")}
          </button>
          <button
            className="btn"
            onClick={() => {
              setAnswers(initial);
              setShown(false);
            }}
          >
            {t("wiz.startOver")}
          </button>
        </div>
      </section>

      {shown && (
        <section className="panel" aria-live="polite">
          {plan.blocked ? (
            <div className="banner danger">{t(`wiz.blocked.${plan.blocked}` as Key)}</div>
          ) : (
            <>
              <h2>{t("wiz.plan")}</h2>
              {answers.knownVersion.trim() && (
                <p className="muted small">{t("wiz.reported", { version: answers.knownVersion.trim() })}</p>
              )}
              {plan.warnings.map((w) => (
                <div key={w} className={w === "no_internet_update" || w === "data_loss" ? "banner danger" : "banner warn"}>
                  {t(`wiz.warn.${w}` as Key)}
                </div>
              ))}
              <ol className="plain">
                {plan.steps.map((s, i) => (
                  <li key={s} className="check">
                    <div className="check-head">
                      <span className="mono">{t("wiz.stepN", { n: i + 1 })}</span>
                      <strong>{t(`wiz.step.${s}.title` as Key)}</strong>
                    </div>
                    <p className="small">{t(`wiz.step.${s}.text` as Key)}</p>
                  </li>
                ))}
              </ol>

              <h3>{t("wiz.pkg.title")}</h3>
              {!plan.proposal && <p>{t("wiz.pkg.noneNeeded")}</p>}
              {plan.proposal?.kind === "need_version" && <p>{t("wiz.pkg.needVersion")}</p>}
              {plan.proposal?.kind === "exact" && <p>{tx("wiz.pkg.exact", { version: plan.proposal.version })}</p>}
              {plan.proposal?.kind === "floor" && <p>{tx("wiz.pkg.floor", { version: plan.proposal.version })}</p>}
              {plan.proposal && plan.proposal.kind !== "need_version" && candidates.length === 0 && (
                <div className="banner warn">{t("wiz.pkg.none")}</div>
              )}
              {candidates.slice(0, 8).map((e) => (
                <div key={e.key} className="row gap between">
                  <span>
                    {t("wiz.pkg.buildLabel", { version: e.firmware_version, build: formatBuild(e.build_date) || "—" })}
                  </span>
                  <button
                    className="btn primary"
                    onClick={() => {
                      buildFor(e.key);
                      navigate("build");
                    }}
                  >
                    {t("wiz.pkg.build")}
                  </button>
                </div>
              ))}
            </>
          )}
        </section>
      )}

      <section className="panel">
        <h2>{t("wiz.sources.title", { date: REVIEWED })}</h2>
        <ul className="plain">
          {SOURCES.map((s) => (
            <li key={s.url}>
              <button className="link-btn" onClick={() => requestOpen(s.url, t(s.key))}>
                {t(s.key)}
              </button>
            </li>
          ))}
        </ul>
        <h3>{t("wiz.unconfirmed.title")}</h3>
        <ul className="plain small">
          <li>{t("wiz.unconfirmed.1")}</li>
          <li>{t("wiz.unconfirmed.2")}</li>
          <li>{t("wiz.unconfirmed.3")}</li>
        </ul>
      </section>
    </div>
  );
}
