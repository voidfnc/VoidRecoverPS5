import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { BANNED } from "../i18n/banned";
import { en } from "../i18n/en";
import { dictOf, hasDictionary, LANGS, loadLang, makeT } from "../i18n/lang";
import * as labels from "./labels";

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    // The language files are checked string by string against their own list; comments in them may name the words.
    if (statSync(p).isDirectory()) return name === "langs" ? [] : sourceFiles(p);
    return /\.tsx?$/.test(name) && !name.endsWith(".test.ts") && name !== "banned.ts" ? [p] : [];
  });
}

beforeAll(async () => {
  await Promise.all(LANGS.map(loadLang));
});

const shipped = () => LANGS.filter(hasDictionary);

describe("status wording", () => {
  it("every shipped language has its own banned-wording list", () => {
    for (const lang of shipped()) expect(BANNED[lang], `add a list for ${lang} in i18n/banned.ts`).toBeDefined();
  });

  it("no language's strings use overclaiming words (its own list and the English one)", () => {
    for (const lang of shipped()) {
      const strings = lang === "en" ? Object.values(en) : Object.values(dictOf(lang));
      for (const s of strings) {
        for (const re of [...(BANNED[lang] ?? []), ...BANNED.en]) expect(s, `${lang}: ${s}`).not.toMatch(re);
      }
    }
  });

  it("UI source avoids overclaiming words", () => {
    for (const file of sourceFiles(join(__dirname, ".."))) {
      const text = readFileSync(file, "utf8");
      for (const re of Object.values(BANNED).flat()) expect(text, file).not.toMatch(re);
    }
  });

  it("every package type has a label and help text in both languages", () => {
    for (const lang of ["en", "ru"] as const) {
      const t = makeT(lang);
      for (const type of ["update", "reinstall", "special_variant", "unknown"] as const) {
        expect(labels.packageTypeShort(t, type)).toBeTruthy();
        expect(labels.packageTypeLabel(t, type)).toBeTruthy();
        expect(labels.packageTypeHelp(t, type)).toBeTruthy();
      }
    }
    expect(labels.packageTypeHelp(makeT("en"), "reinstall")).toMatch(/deletes all data/);
    expect(labels.packageTypeHelp(makeT("ru"), "reinstall")).toMatch(/удаляет все данные/);
  });

  it("joins source names readably", () => {
    expect(labels.joinNames(makeT("en"), ["A", "B", "C"])).toBe("A, B and C");
    expect(labels.joinNames(makeT("ru"), ["A", "B", "C"])).toBe("A, B и C");
  });
});
