// The languages the app knows about, and how a language is chosen. Pure functions, no dictionaries,
// so the choice can be tested without loading any text.

export interface LanguageInfo {
  code: string;
  /** Name in the language itself, shown in the language list. */
  name: string;
  /** BCP 47 tag for Intl (dates, numbers, plurals). */
  locale: string;
  /** Windows primary language id (the low 10 bits of a language id). */
  winId: number;
}

// Same twelve as the MW4 Texture Streaming Toolkit.
export const LANGUAGES = [
  { code: "en", name: "English", locale: "en-US", winId: 0x09 },
  { code: "ru", name: "Русский", locale: "ru-RU", winId: 0x19 },
  { code: "zh-CN", name: "简体中文", locale: "zh-CN", winId: 0x04 },
  { code: "es", name: "Español", locale: "es-ES", winId: 0x0a },
  { code: "fr", name: "Français", locale: "fr-FR", winId: 0x0c },
  { code: "de", name: "Deutsch", locale: "de-DE", winId: 0x07 },
  { code: "pt-BR", name: "Português (Brasil)", locale: "pt-BR", winId: 0x16 },
  { code: "tr", name: "Türkçe", locale: "tr-TR", winId: 0x1f },
  { code: "pl", name: "Polski", locale: "pl-PL", winId: 0x15 },
  { code: "ja", name: "日本語", locale: "ja-JP", winId: 0x11 },
  { code: "ko", name: "한국어", locale: "ko-KR", winId: 0x12 },
  { code: "it", name: "Italiano", locale: "it-IT", winId: 0x10 },
] as const satisfies readonly LanguageInfo[];

export type Lang = (typeof LANGUAGES)[number]["code"];
export const LANGS: readonly Lang[] = LANGUAGES.map((l) => l.code);
export const DEFAULT_LANG: Lang = "en";
export const LOCALES = Object.fromEntries(LANGUAGES.map((l) => [l.code, l.locale])) as Record<Lang, string>;
export const NAMES = Object.fromEntries(LANGUAGES.map((l) => [l.code, l.name])) as Record<Lang, string>;

/** What is stored: a language code, or "auto" (follow the installer, then Windows). */
export type LangSetting = Lang | "auto";
export const AUTO = "auto";
export const STORAGE_KEY = "voidrecoverps5.lang";

export function isLang(value: unknown): value is Lang {
  return typeof value === "string" && (LANGS as readonly string[]).includes(value);
}

/** A saved value from storage -> a setting; anything unknown means Automatic. */
export function parseSetting(value: string | null | undefined): LangSetting {
  return isLang(value) ? value : AUTO;
}

/** Match a BCP 47 tag ("de-AT", "zh-TW", "pt-PT") to a language we have. Chinese and Portuguese map to the one variant we ship. */
export function langFromTag(tag: string | null | undefined): Lang | null {
  const primary = tag?.toLowerCase().split(/[-_]/)[0];
  if (!primary) return null;
  if (primary === "zh") return "zh-CN";
  if (primary === "pt") return "pt-BR";
  return LANGS.find((c) => c.toLowerCase() === primary) ?? null;
}

/** Match a Windows language id (such as 1049 for Russian) to a language we have. */
export function langFromLcid(lcid: number | null | undefined): Lang | null {
  if (!lcid) return null;
  const primary = lcid & 0x3ff;
  return LANGUAGES.find((l) => l.winId === primary)?.code ?? null;
}

export interface LangInputs {
  setting: LangSetting;
  /** Language id recorded by the installer, if any. */
  installerLcid?: number | null;
  /** Windows display language id. */
  windowsLcid?: number | null;
  /** navigator.languages of the window, most preferred first. */
  navigatorLangs?: readonly string[];
}

/**
 * The language to show. A fixed choice wins. Automatic follows the installer's choice, then the Windows
 * display language, then the window's own language list, then English. A language that has no text yet
 * (`available` says no) is skipped, so it never leaves the window in English by accident when a later hint
 * would work.
 */
export function resolveLang(inputs: LangInputs, available: (lang: Lang) => boolean = () => true): Lang {
  if (isLang(inputs.setting)) return inputs.setting;
  const candidates: (Lang | null)[] = [
    langFromLcid(inputs.installerLcid),
    langFromLcid(inputs.windowsLcid),
    ...(inputs.navigatorLangs ?? []).map(langFromTag),
  ];
  // The installer's choice is a decision, not a hint: if it names a language we don't ship yet, English is right.
  const installer = candidates[0];
  if (installer) return available(installer) ? installer : DEFAULT_LANG;
  return candidates.find((l): l is Lang => l !== null && available(l)) ?? DEFAULT_LANG;
}
