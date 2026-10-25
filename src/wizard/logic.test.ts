import { describe, expect, it } from "vitest";
import type { CatalogEntry } from "../api/types";
import { buildPlan, candidateEntries, compareVersions, normalizeVersion, type Answers } from "./logic";

const base: Answers = { symptom: "safe_mode", priority: "working", knownVersion: "", minVersion: "" };

describe("versions", () => {
  it("normalises and compares", () => {
    expect(normalizeVersion("14.2")).toBe("14.02.00");
    expect(normalizeVersion("9.60")).toBe("09.60.00");
    expect(normalizeVersion(" 14.20.00 ")).toBe("14.20.00");
    expect(normalizeVersion("v14")).toBeNull();
    expect(normalizeVersion("14.20.00.1")).toBeNull();
    expect(compareVersions("13.60.00", "14.20.00")).toBeLessThan(0);
    expect(compareVersions("14.20.00", "14.20.00")).toBe(0);
  });
});

describe("plan", () => {
  it("never proposes the internet update and orders steps least destructive first", () => {
    const p = buildPlan(base);
    expect(p.steps).toEqual(["restart", "rebuild_db", "update_usb", "reinstall"]);
    expect(p.warnings).toContain("no_internet_update");
    expect(p.warnings).toContain("data_loss");
  });

  it("does not jump to reinstall when the console boots", () => {
    const p = buildPlan({ ...base, symptom: "boots" });
    expect(p.steps).toEqual(["restart"]);
    expect(p.warnings).not.toContain("data_loss");
  });

  it("preserve: drops the USB update and proposes only the exact reported version", () => {
    const p = buildPlan({ ...base, priority: "preserve", knownVersion: "13.60" });
    expect(p.steps).not.toContain("update_usb");
    expect(p.proposal).toEqual({ kind: "exact", version: "13.60.00" });
    expect(p.warnings).toContain("newer_ends_exploit");
    expect(p.warnings).toContain("fpkg_risk");
  });

  it("preserve without a version asks for it instead of guessing", () => {
    expect(buildPlan({ ...base, priority: "preserve" }).proposal).toEqual({ kind: "need_version" });
  });

  it("the console's floor above the reported version stops the wizard", () => {
    const p = buildPlan({ ...base, priority: "preserve", knownVersion: "13.60", minVersion: "14.00" });
    expect(p.blocked).toBe("floor_above_reported");
    expect(p.proposal).toBeNull();
    expect(p.steps).toEqual([]);
  });

  it("a floor equal to the reported version is fine", () => {
    const p = buildPlan({ ...base, priority: "preserve", knownVersion: "13.60", minVersion: "13.60" });
    expect(p.blocked).toBeNull();
    expect(p.proposal).toEqual({ kind: "exact", version: "13.60.00" });
  });

  it("non-preserve with a floor proposes that version or later", () => {
    const p = buildPlan({ ...base, symptom: "asks_file", minVersion: "12.00" });
    expect(p.steps).toEqual(["reinstall"]);
    expect(p.proposal).toEqual({ kind: "floor", version: "12.00.00" });
  });

  it("rejects malformed versions", () => {
    expect(buildPlan({ ...base, knownVersion: "abc" }).blocked).toBe("bad_version");
    expect(buildPlan({ ...base, minVersion: "1" }).blocked).toBe("bad_min_version");
  });
});

function entry(version: string, type: CatalogEntry["package_type"], build: string, variant: string | null = null) {
  return {
    key: `${version}-${type}-${build}`,
    firmware_version: version,
    package_type: type,
    variant_label: variant,
    build_date: build,
  } as CatalogEntry;
}

describe("candidates", () => {
  const entries = [
    entry("13.60.00", "reinstall", "2025_0101"),
    entry("13.60.00", "update", "2025_0101"),
    entry("14.20.00", "reinstall", "2026_1014"),
    entry("13.60.00", "reinstall", "2025_0202", "sys_ex"),
  ];
  it("exact: only recovery files of that version without a variant", () => {
    const c = candidateEntries(entries, { kind: "exact", version: "13.60.00" });
    expect(c.map((e) => e.key)).toEqual(["13.60.00-reinstall-2025_0101"]);
  });
  it("floor: that version or newer", () => {
    expect(candidateEntries(entries, { kind: "floor", version: "13.60.00" }).length).toBe(2);
  });
  it("nothing without a proposal", () => {
    expect(candidateEntries(entries, null)).toEqual([]);
    expect(candidateEntries(entries, { kind: "need_version" })).toEqual([]);
  });
});
