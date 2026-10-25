import { describe, expect, it } from "vitest";
import type { CatalogEntry, PackageType } from "../api/types";
import { applyFilter, defaultFilter, matchesQuery } from "./filter";

function entry(version: string, type: PackageType, opts: Partial<CatalogEntry["evidence"]> = {}, label?: string): CatalogEntry {
  return {
    key: `${version}|${type}`,
    firmware_version: version,
    package_type: type,
    variant_label: type === "special_variant" ? "sys_ex" : null,
    records: [
      {
        package_id: `x:${version}:${type}`,
        source_id: "darthsternie-ps5",
        firmware_version: version,
        long_version: null,
        build_date: null,
        source_section: null,
        raw_package_label: label ?? `Firmware ${version}`,
        package_type: type,
        variant_label: null,
        published_size_bytes: null,
        size_is_approximate: false,
        raw_size_text: null,
        hashes: [{ algorithm: "md5", value: "f7853a0fa97a9a7828ac548798012782", source_id: "darthsternie-ps5", derived_from: null, note: null }],
        mirrors: [],
        last_checked: "t",
      },
    ],
    evidence: {
      hash_claims: [],
      sha256_conflict: false,
      md5_conflict: false,
      sizes: [],
      size_conflict: false,
      cross_source: [],
      best_size_bytes: null,
      best_size_is_approximate: false,
      sony_availability: null,
      ...opts,
    },
  };
}

const sample = [
  entry("09.60.00", "update"),
  entry("10.01.00", "reinstall", { sony_availability: "unavailable" }),
  entry("14.20.00", "update", { md5_conflict: true }),
  entry("08.20.02", "special_variant", {}, "Firmware 08.20.02 sys_ex"),
];

describe("matchesQuery", () => {
  it("matches versions with or without dots", () => {
    expect(matchesQuery(sample[2], "14.20")).toBe(true);
    expect(matchesQuery(sample[2], "1420")).toBe(true);
    expect(matchesQuery(sample[2], "13")).toBe(false);
  });
  it("matches raw labels and checksum prefixes", () => {
    expect(matchesQuery(sample[3], "sys_ex")).toBe(true);
    expect(matchesQuery(sample[0], "f7853a0f")).toBe(true);
  });
});

describe("applyFilter", () => {
  it("sorts numerically, newest first, by default", () => {
    expect(applyFilter(sample, defaultFilter).map((e) => e.firmware_version)).toEqual([
      "14.20.00",
      "10.01.00",
      "09.60.00",
      "08.20.02",
    ]);
  });
  it("filters by package type", () => {
    const f = { ...defaultFilter, types: { update: false, reinstall: true, special_variant: false, unknown: false } };
    expect(applyFilter(sample, f).map((e) => e.key)).toEqual(["10.01.00|reinstall"]);
  });
  it("filters to Sony-addressable and conflicting entries", () => {
    expect(applyFilter(sample, { ...defaultFilter, onlyWithSony: true })).toHaveLength(1);
    expect(applyFilter(sample, { ...defaultFilter, onlyConflicts: true })[0].firmware_version).toBe("14.20.00");
  });
});
