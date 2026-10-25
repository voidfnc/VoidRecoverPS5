import { currentLang, currentT, LOCALES, type Lang } from "../i18n/lang";

const BINARY = ["B", "KiB", "MiB", "GiB"] as const;
// Russian and French have their own unit letters; every other language keeps the international ones.
const UNIT_OVERRIDES: Partial<Record<Lang, readonly string[]>> = {
  ru: ["Б", "КиБ", "МиБ", "ГиБ"],
  fr: ["o", "Kio", "Mio", "Gio"],
};

export function formatBytes(bytes: number | null | undefined, approximate = false): string {
  if (bytes == null) return "—";
  const lang = currentLang();
  const units = UNIT_OVERRIDES[lang] ?? BINARY;
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = u === 0 ? 0 : v >= 100 ? 0 : 2;
  const num = v.toLocaleString(LOCALES[lang], { minimumFractionDigits: digits, maximumFractionDigits: digits });
  const text = `${num} ${units[u]}`;
  return approximate ? `≈ ${text}` : text;
}

export function formatExactBytes(bytes: number): string {
  return currentT()("common.exactBytes", { n: bytes.toLocaleString(LOCALES[currentLang()]) });
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return currentT()("common.never");
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(LOCALES[currentLang()], { dateStyle: "medium", timeStyle: "short" });
}

/** "2023_1101" -> "2023-11-01" (Sony build dates are YYYY_MMDD). */
export function formatBuild(build: string | null | undefined): string {
  if (!build) return "";
  const m = /^(\d{4})_(\d{2})(\d{2})$/.exec(build);
  return m ? `${m[1]}-${m[2]}-${m[3]}` : build;
}

export function shortHash(value: string): string {
  return value.length > 16 ? `${value.slice(0, 8)}…${value.slice(-8)}` : value;
}

export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}
