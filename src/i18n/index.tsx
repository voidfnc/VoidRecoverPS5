import { createContext, Fragment, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api } from "../api/client";
import { en, type Key } from "./en";
import {
  AUTO,
  hasDictionary,
  interpolate,
  isLang,
  LANGS,
  loadLang,
  makeT,
  parseSetting,
  resolveLang,
  setCurrentLang,
  STORAGE_KEY,
  translate,
  type Lang,
  type LangSetting,
  type T,
  type Vars,
} from "./lang";

export type { Key } from "./en";
export type { Lang, LangSetting, T, Vars } from "./lang";

/** `**bold**` and `` `mono` `` markers in a string become elements. */
export function renderRich(text: string): ReactNode {
  const parts = text.split(/(\*\*[^*]+\*\*|`[^`]+`)/g);
  return parts.map((p, i) => {
    if (p.startsWith("**") && p.endsWith("**") && p.length > 4) return <strong key={i}>{p.slice(2, -2)}</strong>;
    if (p.startsWith("`") && p.endsWith("`") && p.length > 2) {
      return (
        <span key={i} className="mono">
          {p.slice(1, -1)}
        </span>
      );
    }
    return <Fragment key={i}>{p}</Fragment>;
  });
}

interface I18n {
  /** The language being shown. */
  lang: Lang;
  /** What the user chose: a language or Automatic. */
  setting: LangSetting;
  setSetting: (s: LangSetting) => void;
  /** Languages that have text to show, for the language list. */
  languages: readonly Lang[];
  t: T;
  /** Like t, but renders `**bold**` and `` `mono` `` markers. */
  tx: (key: Key, vars?: Vars) => ReactNode;
}

const I18nContext = createContext<I18n | null>(null);

function readSaved(): string | null {
  try {
    return window.localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

/** Installer and Windows language hints; absent outside the desktop app (plain browser), where the window's own list is used. */
async function systemHints(): Promise<{ installer: number | null; windows_ui: number | null }> {
  try {
    return await api.systemLanguages();
  } catch {
    return { installer: null, windows_ui: null };
  }
}

export function LangProvider({ children, initial }: { children: ReactNode; initial?: Lang }) {
  const [setting, setSettingState] = useState<LangSetting>(() => (initial ? initial : parseSetting(readSaved())));
  const [lang, setLang] = useState<Lang | null>(null);

  useEffect(() => {
    let live = true;
    (async () => {
      const hints = isLang(setting) ? null : await systemHints();
      const next = resolveLang(
        {
          setting,
          installerLcid: hints?.installer,
          windowsLcid: hints?.windows_ui,
          navigatorLangs: navigator.languages?.length ? navigator.languages : [navigator.language],
        },
        hasDictionary,
      );
      await loadLang(next);
      if (live) setLang(next);
    })();
    return () => {
      live = false;
    };
  }, [setting]);

  const shown: Lang = lang ?? "en";
  // Set during render so format helpers used by children already see the new language.
  setCurrentLang(shown);

  useEffect(() => {
    document.documentElement.lang = shown;
  }, [shown]);

  const value = useMemo<I18n>(
    () => ({
      lang: shown,
      setting,
      languages: LANGS.filter(hasDictionary),
      t: makeT(shown),
      tx: (key, vars) => renderRich(interpolate(translate(shown, key) ?? en[key], vars)),
      setSetting: (s) => {
        setSettingState(s);
        try {
          window.localStorage.setItem(STORAGE_KEY, s === AUTO ? AUTO : s);
        } catch {
          // Storage can be unavailable; the choice just won't be remembered.
        }
      },
    }),
    [shown, setting],
  );

  // The first language is chosen before anything is drawn, so the window never flashes in the wrong one.
  if (lang === null) return null;
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n outside LangProvider");
  return ctx;
}

export function useT(): T {
  return useI18n().t;
}
