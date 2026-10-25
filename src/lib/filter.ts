import type { CatalogEntry, PackageType } from "../api/types";

export type SortKey = "version" | "sources" | "size" | "hashes";

export interface CatalogFilter {
  query: string;
  types: Record<PackageType, boolean>;
  sourceId: string | "all";
  onlyWithSony: boolean;
  onlyConflicts: boolean;
  sort: SortKey;
  descending: boolean;
}

export const defaultFilter: CatalogFilter = {
  query: "",
  types: { update: true, reinstall: true, special_variant: true, unknown: true },
  sourceId: "all",
  onlyWithSony: false,
  onlyConflicts: false,
  sort: "version",
  descending: true,
};

export function hasConflict(e: CatalogEntry): boolean {
  return e.evidence.md5_conflict || e.evidence.sha256_conflict || e.evidence.size_conflict;
}

function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Matches "14.20", "1420", "14.20.00" and raw source labels such as "sys_ex". */
export function matchesQuery(e: CatalogEntry, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const compact = (s: string) => s.replace(/[^0-9a-z_]/g, "");
  if (e.firmware_version.startsWith(q) || compact(e.firmware_version).startsWith(compact(q))) return true;
  if (e.variant_label?.toLowerCase().includes(q)) return true;
  return e.records.some(
    (r) =>
      r.raw_package_label.toLowerCase().includes(q) ||
      r.long_version?.toLowerCase().includes(q) ||
      r.hashes.some((h) => h.value.startsWith(q)),
  );
}

export function applyFilter(entries: CatalogEntry[], f: CatalogFilter): CatalogEntry[] {
  const out = entries.filter(
    (e) =>
      f.types[e.package_type] &&
      matchesQuery(e, f.query) &&
      (f.sourceId === "all" || e.records.some((r) => r.source_id === f.sourceId)) &&
      (!f.onlyWithSony || e.evidence.sony_availability !== null) &&
      (!f.onlyConflicts || hasConflict(e)),
  );
  const key = (e: CatalogEntry): number => {
    switch (f.sort) {
      case "sources":
        return new Set(e.records.map((r) => r.source_id)).size;
      case "size":
        return e.evidence.best_size_bytes ?? -1;
      case "hashes":
        return e.evidence.hash_claims.length;
      default:
        return 0;
    }
  };
  out.sort((a, b) => {
    const primary = f.sort === "version" ? compareVersions(a.firmware_version, b.firmware_version) : key(a) - key(b);
    const tie = compareVersions(a.firmware_version, b.firmware_version);
    return (f.descending ? -1 : 1) * (primary !== 0 ? primary : tie);
  });
  return out;
}
