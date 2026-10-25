import { beforeAll, describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { dictOf, loadLang } from "../i18n/lang";
import { CATEGORIES, REPORT_URL, RESOURCES } from "./resources";

beforeAll(async () => {
  await loadLang("ru");
});

describe("community resources", () => {
  it("ids are unique and every entry has a note in both languages", () => {
    const ids = RESOURCES.map((r) => r.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const r of RESOURCES) {
      const key = `res.${r.id}.note`;
      expect((en as Record<string, string>)[key], key).toBeTruthy();
      expect((dictOf("ru") as Record<string, string>)[key], key).toBeTruthy();
    }
  });

  it("links are https and never mirror or download hosts", () => {
    for (const r of [...RESOURCES.map((x) => x.url), REPORT_URL]) {
      expect(r.startsWith("https://"), r).toBe(true);
      expect(r, r).not.toMatch(/mega\.(nz|io)|1fichier|\.pup(\?|$)/i);
    }
  });

  it("categories are known, dates are ISO, and data has only the expected fields", () => {
    const fields = ["category", "id", "lastChecked", "linkOnly", "maintainer", "name", "provenance", "url"];
    for (const r of RESOURCES) {
      expect(CATEGORIES).toContain(r.category);
      expect(r.lastChecked).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(Object.keys(r).sort()).toEqual(fields);
    }
  });
});
