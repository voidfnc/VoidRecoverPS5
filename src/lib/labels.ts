// All user-facing status wording lives in src/i18n so it stays precise and consistent in both languages.
// Never word a status as more than a checksum or a source listing proves; labels.test.ts enforces the
// banned phrases for English and Russian.

import type {
  Availability,
  CatalogView,
  EntryVerdict,
  HashAlgorithm,
  PackageType,
  SourceRecord,
  SourceState,
  SourceType,
} from "../api/types";
import type { T } from "../i18n/lang";

export const packageTypeShort = (t: T, type: PackageType): string => t(`type.${type}`);
export const packageTypeLabel = (t: T, type: PackageType): string => t(`typeLabel.${type}`);
export const packageTypeHelp = (t: T, type: PackageType): string => t(`typeHelp.${type}`);
export const sourceTypeLabel = (t: T, type: SourceType): string => t(`srcType.${type}`);
export const sourceStateLabel = (t: T, state: SourceState): string => t(`state.${state}`);
export const availabilityLabel = (t: T, a: Availability): string => t(`avail.${a}`);
export const verdictLabel = (t: T, v: EntryVerdict): string => t(`verdict.${v}`);

export const algorithmLabel: Record<HashAlgorithm, string> = { md5: "MD5", sha256: "SHA-256" };

export const hashMeaning = (t: T): string => t("hashMeaning");

export function sourceName(view: CatalogView | null, id: string): string {
  return view?.sources.find((s) => s.source_id === id)?.name ?? id;
}

export function sourceRecord(view: CatalogView | null, id: string): SourceRecord | undefined {
  return view?.sources.find((s) => s.source_id === id);
}

/** "MD5 matches Darthsternie's PS5 Firmware Archive" style statements. */
export function matchStatement(t: T, view: CatalogView | null, algorithm: HashAlgorithm, sourceIds: string[]): string {
  const names = sourceIds.map((id) => sourceName(view, id));
  return t("match.statement", { alg: algorithmLabel[algorithm], names: joinNames(t, names) });
}

export function joinNames(t: T, names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")}${t("common.and")}${names[names.length - 1]}`;
}
