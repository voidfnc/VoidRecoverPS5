// Language state and pure translation helpers (no React), shared by the UI and by lib/format.ts.
// English is always bundled; every other language is its own file in ./langs/ and is loaded on demand.
// A string a language file doesn't have shows in English, so a gap never breaks the window.

import { en, type Key } from "./en";
import { DEFAULT_LANG, LOCALES, type Lang } from "./registry";

export * from "./registry";

export type Dict = Partial<Record<Key, string>>;
export type Vars = Record<string, string | number>;
export type T = (key: Key, vars?: Vars) => string;

// import.meta.glob picks up whatever langs/<code>.ts files exist, so a language ships by adding its file.
const loaders = import.meta.glob<{ dict: Dict }>("./langs/*.ts");
const codeOf = (path: string) => path.replace(/^\.\/langs\//, "").replace(/\.ts$/, "");
const shipped = new Set<string>([DEFAULT_LANG, ...Object.keys(loaders).map(codeOf)]);

/** Languages that have text to show (English plus every file in ./langs/). */
export function hasDictionary(lang: Lang): boolean {
  return shipped.has(lang);
}

const cache: Partial<Record<Lang, Dict>> = { en };

/** Make a language's text available to translate() and makeT(). Safe to call again. */
export async function loadLang(lang: Lang): Promise<void> {
  if (cache[lang]) return;
  const loader = loaders[`./langs/${lang}.ts`];
  cache[lang] = loader ? (await loader()).dict : {};
}

export function dictOf(lang: Lang): Dict {
  return cache[lang] ?? {};
}

export function interpolate(template: string, vars?: Vars): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}

export function translate(lang: Lang, key: Key, vars?: Vars): string {
  return interpolate(dictOf(lang)[key] ?? en[key], vars);
}

export function makeT(lang: Lang): T {
  return (key, vars) => translate(lang, key, vars);
}

export type PluralCategory = "one" | "few" | "many" | "other";

/** CLDR plural category for a whole number in the language (English one/other, Russian one/few/many, Japanese other only...). */
export function pluralCategory(lang: Lang, n: number): PluralCategory {
  const cat = new Intl.PluralRules(LOCALES[lang]).select(Math.abs(Math.trunc(n)));
  return cat === "one" || cat === "few" || cat === "many" ? cat : "other";
}

/** Pick a form: `plural("ru", 3, { one: "файл", few: "файла", many: "файлов" })`. New strings should avoid plurals instead. */
export function plural(lang: Lang, n: number, forms: Partial<Record<PluralCategory, string>>): string {
  return forms[pluralCategory(lang, n)] ?? forms.other ?? forms.many ?? forms.one ?? "";
}

// The active language, kept here so plain functions (byte and date formatting) can follow it.
let current: Lang = DEFAULT_LANG;

export function setCurrentLang(lang: Lang): void {
  current = lang;
}

export function currentLang(): Lang {
  return current;
}

export function currentT(): T {
  return makeT(current);
}

/** The text of a source's note in the active language: its dictionary entry if there is one, else the manifest's English. */
export function sourceNote(lang: Lang, sourceId: string, english: string): string {
  const key = `src.${sourceId}.note`;
  return dictOf(lang)[key as Key] ?? (en as Record<string, string>)[key] ?? english;
}
