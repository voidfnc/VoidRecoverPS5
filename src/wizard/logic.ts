// Decision logic for the recovery wizard. Pure and language-free: it returns ids,
// and the page turns them into text. Rules:
// least destructive step first, never the internet update, preservation means the
// recovery file for the exact current version and nothing newer.

import type { CatalogEntry } from "../api/types";

export type Symptom = "boots" | "storage" | "safe_mode" | "asks_file" | "loop";
export type Priority = "keep_data" | "working" | "preserve";

export interface Answers {
  symptom: Symptom;
  priority: Priority;
  /** What the user says the console is on, e.g. "13.60" (user-reported, not verified). */
  knownVersion: string;
  /** The "version X or later" the console shows on its reinstall prompt, if any. */
  minVersion: string;
}

export type StepId = "restart" | "repair_storage" | "rebuild_db" | "update_usb" | "reinstall";
export type WarningId =
  | "no_internet_update"
  | "data_loss"
  | "backup_first"
  | "newer_ends_exploit"
  | "fpkg_risk";

export type Blocked = "bad_version" | "bad_min_version" | "floor_above_reported" | null;

export type PackageProposal =
  /** The recovery file for exactly this version. */
  | { kind: "exact"; version: string }
  /** Any recovery file at or above this version; the user picks. */
  | { kind: "floor"; version: string }
  /** Needs the user's version before a file can be proposed. */
  | { kind: "need_version" };

export interface Plan {
  blocked: Blocked;
  steps: StepId[];
  warnings: WarningId[];
  proposal: PackageProposal | null;
}

/** "14.2" / "14.20" / "14.20.00" -> "14.20.00"; anything else -> null. */
export function normalizeVersion(text: string): string | null {
  const m = /^\s*(\d{1,2})\.(\d{1,2})(?:\.(\d{1,2}))?\s*$/.exec(text);
  if (!m) return null;
  const pad = (s: string | undefined) => (s ?? "0").padStart(2, "0");
  return `${pad(m[1])}.${pad(m[2])}.${pad(m[3])}`;
}

export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

const STEPS: Record<Symptom, StepId[]> = {
  boots: ["restart"],
  storage: ["restart", "repair_storage", "rebuild_db"],
  safe_mode: ["restart", "rebuild_db", "update_usb", "reinstall"],
  loop: ["restart", "rebuild_db", "update_usb", "reinstall"],
  asks_file: ["reinstall"],
};

export function buildPlan(a: Answers): Plan {
  const known = a.knownVersion.trim() ? normalizeVersion(a.knownVersion) : null;
  const min = a.minVersion.trim() ? normalizeVersion(a.minVersion) : null;
  const empty = (blocked: Blocked): Plan => ({ blocked, steps: [], warnings: ["no_internet_update"], proposal: null });
  if (a.knownVersion.trim() && !known) return empty("bad_version");
  if (a.minVersion.trim() && !min) return empty("bad_min_version");
  // The console's own floor beats the user's recollection: stop instead of picking a file.
  if (a.priority === "preserve" && known && min && compareVersions(known, min) < 0) {
    return empty("floor_above_reported");
  }

  let steps = [...STEPS[a.symptom]];
  // The USB update step installs whatever file is on the stick; when preserving, only the
  // same-version recovery file is acceptable, which is the reinstall route.
  if (a.priority === "preserve") steps = steps.filter((s) => s !== "update_usb");

  const warnings: WarningId[] = ["no_internet_update"];
  if (steps.includes("reinstall")) warnings.push("data_loss");
  if (steps.length > 1 || steps.includes("reinstall")) warnings.push("backup_first");
  if (a.priority === "preserve") {
    warnings.push("newer_ends_exploit");
    if (steps.includes("rebuild_db")) warnings.push("fpkg_risk");
  }

  let proposal: PackageProposal | null = null;
  if (steps.includes("reinstall")) {
    if (a.priority === "preserve") {
      proposal = known ? { kind: "exact", version: known } : { kind: "need_version" };
    } else if (min) {
      proposal = { kind: "floor", version: min };
    }
  }
  return { blocked: null, steps, warnings, proposal };
}

/** Recovery (reinstall) entries a proposal can point at, oldest first. */
export function candidateEntries(entries: CatalogEntry[], proposal: PackageProposal | null): CatalogEntry[] {
  if (!proposal || proposal.kind === "need_version") return [];
  const ok = (e: CatalogEntry) => e.package_type === "reinstall" && !e.variant_label;
  const matches = (e: CatalogEntry) =>
    proposal.kind === "exact"
      ? compareVersions(e.firmware_version, proposal.version) === 0
      : compareVersions(e.firmware_version, proposal.version) >= 0;
  return entries
    .filter((e) => ok(e) && matches(e))
    .sort(
      (a, b) =>
        compareVersions(a.firmware_version, b.firmware_version) ||
        (a.build_date ?? "").localeCompare(b.build_date ?? ""),
    );
}
