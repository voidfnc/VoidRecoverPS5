import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { localizeBackend } from "./backend";
import { en, type Key } from "./en";
import {
  dictOf,
  hasDictionary,
  interpolate,
  LANGS,
  LANGUAGES,
  langFromLcid,
  langFromTag,
  loadLang,
  makeT,
  parseSetting,
  plural,
  pluralCategory,
  resolveLang,
  setCurrentLang,
  sourceNote,
  type Lang,
} from "./lang";
import { formatBytes } from "../lib/format";

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
const markers = (s: string) => ({ bold: (s.match(/\*\*/g) ?? []).length, mono: (s.match(/`/g) ?? []).length });
const shipped = () => LANGS.filter((l) => l !== "en" && hasDictionary(l));

beforeAll(async () => {
  await Promise.all(LANGS.map(loadLang));
});

describe("language registry", () => {
  it("has twelve unique, valid languages", () => {
    expect(LANGUAGES.length).toBe(12);
    expect(new Set(LANGUAGES.map((l) => l.code)).size).toBe(12);
    expect(new Set(LANGUAGES.map((l) => l.winId)).size).toBe(12);
    for (const l of LANGUAGES) expect(Intl.getCanonicalLocales(l.locale).length, l.code).toBe(1);
  });
});

describe("dictionaries", () => {
  it("every shipped language has exactly the keys of en, no empty strings and the same markers", () => {
    for (const lang of shipped()) {
      const d = dictOf(lang) as Record<string, string>;
      expect(Object.keys(d).sort(), lang).toEqual(Object.keys(en).sort());
      for (const k of Object.keys(en) as Key[]) {
        expect(d[k].trim(), `${lang} ${k}`).not.toBe("");
        expect(placeholders(d[k]), `${lang} ${k}`).toEqual(placeholders(en[k]));
        expect(markers(d[k]), `${lang} ${k}`).toEqual(markers(en[k]));
      }
    }
  });

  it("every t(...) key used in the UI exists", () => {
    const used = new Set<string>();
    const walk = (dir: string) => {
      for (const name of readdirSync(dir)) {
        const p = join(dir, name);
        if (statSync(p).isDirectory()) walk(p);
        else if (/\.tsx?$/.test(name) && !name.endsWith(".test.ts")) {
          for (const m of readFileSync(p, "utf8").matchAll(/\bt[x]?\(\s*"([a-zA-Z0-9_.]+)"/g)) used.add(m[1]);
        }
      }
    };
    walk(join(__dirname, ".."));
    for (const k of used) expect(en, k).toHaveProperty(k);
  });
});

describe("language choice", () => {
  it("matches tags and Windows ids to the languages we ship", () => {
    expect(langFromTag("de-AT")).toBe("de");
    expect(langFromTag("zh-TW")).toBe("zh-CN");
    expect(langFromTag("pt-PT")).toBe("pt-BR");
    expect(langFromTag("RU")).toBe("ru");
    expect(langFromTag("sv-SE")).toBeNull();
    expect(langFromTag("")).toBeNull();
    expect(langFromLcid(1049)).toBe("ru");
    expect(langFromLcid(1033)).toBe("en");
    expect(langFromLcid(2052)).toBe("zh-CN");
    expect(langFromLcid(1028)).toBe("zh-CN");
    expect(langFromLcid(1046)).toBe("pt-BR");
    expect(langFromLcid(0)).toBeNull();
    expect(langFromLcid(1053)).toBeNull();
  });

  it("parses saved values: a language, otherwise Automatic", () => {
    expect(parseSetting("ru")).toBe("ru");
    expect(parseSetting("zh-CN")).toBe("zh-CN");
    expect(parseSetting("auto")).toBe("auto");
    expect(parseSetting("klingon")).toBe("auto");
    expect(parseSetting(null)).toBe("auto");
  });

  it("a fixed choice wins over every hint", () => {
    expect(resolveLang({ setting: "en", installerLcid: 1049, windowsLcid: 1049, navigatorLangs: ["ru"] })).toBe("en");
    expect(resolveLang({ setting: "ja", installerLcid: 1033 })).toBe("ja");
  });

  it("Automatic follows the installer, then Windows, then the window's languages, then English", () => {
    const base = { setting: "auto" } as const;
    expect(resolveLang({ ...base, installerLcid: 1049, windowsLcid: 1033, navigatorLangs: ["en-US"] })).toBe("ru");
    expect(resolveLang({ ...base, installerLcid: null, windowsLcid: 1049, navigatorLangs: ["en-US"] })).toBe("ru");
    expect(resolveLang({ ...base, windowsLcid: 1033, navigatorLangs: ["ru"] })).toBe("en");
    expect(resolveLang({ ...base, navigatorLangs: ["sv-SE", "ru-RU"] })).toBe("ru");
    expect(resolveLang({ ...base })).toBe("en");
  });

  it("skips languages with no text yet, but respects an installer choice of one", () => {
    const only = (...ok: Lang[]) => (l: Lang) => ok.includes(l);
    expect(resolveLang({ setting: "auto", windowsLcid: 1031, navigatorLangs: ["de", "ru"] }, only("en", "ru"))).toBe("ru");
    expect(resolveLang({ setting: "auto", installerLcid: 1031, navigatorLangs: ["ru"] }, only("en", "ru"))).toBe("en");
  });

  it("only languages with a dictionary count as shipped", () => {
    expect(hasDictionary("en")).toBe(true);
    expect(hasDictionary("ru")).toBe(true);
  });
});

describe("helpers", () => {
  it("interpolates and leaves unknown placeholders alone", () => {
    expect(interpolate("{a} of {b} {c}", { a: 1, b: "x" })).toBe("1 of x {c}");
  });

  it("Russian plural categories", () => {
    const cat = (n: number) => pluralCategory("ru", n);
    expect([1, 21, 101].map(cat)).toEqual(["one", "one", "one"]);
    expect([2, 3, 4, 22, 104].map(cat)).toEqual(["few", "few", "few", "few", "few"]);
    expect([0, 5, 11, 12, 14, 20, 111].map(cat)).toEqual(Array(7).fill("many"));
    expect(pluralCategory("en", 1)).toBe("one");
    expect(pluralCategory("en", 2)).toBe("other");
    expect(pluralCategory("ja", 1)).toBe("other");
    expect(pluralCategory("fr", 0)).toBe("one");
    expect(pluralCategory("pl", 3)).toBe("few");
    expect(pluralCategory("pl", 5)).toBe("many");
    expect(plural("ru", 3, { one: "файл", few: "файла", many: "файлов" })).toBe("файла");
    expect(plural("en", 2, { one: "file", other: "files" })).toBe("files");
  });

  it("formats byte sizes with the active language", () => {
    setCurrentLang("en");
    expect(formatBytes(1536)).toBe("1.50 KiB");
    setCurrentLang("ru");
    expect(formatBytes(1536)).toMatch(/^1,50 КиБ$/);
    expect(formatBytes(512)).toBe("512 Б");
    setCurrentLang("fr");
    expect(formatBytes(2 * 1024 ** 3)).toMatch(/^2,00 Gio$/);
    setCurrentLang("ja");
    expect(formatBytes(1536)).toBe("1.50 KiB");
    setCurrentLang("en");
  });

  it("takes source notes from the dictionary, falling back to English", () => {
    expect(sourceNote("ru", "sony-ps5-cdn", "English")).not.toBe("English");
    expect(sourceNote("ru", "unknown-source", "English")).toBe("English");
    expect(sourceNote("en", "unknown-source", "English")).toBe("English");
  });

  it("English source notes in the dictionary are the manifest's, so the two can't drift apart", () => {
    const manifest = JSON.parse(readFileSync(join(__dirname, "..", "..", "manifests", "sources.json"), "utf8"));
    for (const s of manifest.sources) {
      expect((en as Record<string, string>)[`src.${s.source_id}.note`], s.source_id).toBe(s.notes);
    }
  });
});

describe("backend messages", () => {
  const t = makeT("ru");
  const loc = (m: string) => localizeBackend(m, t, "ru");

  // Samples are copied from the Rust message templates.
  const samples = [
    "The file is empty.",
    "The file is larger than 4 GiB, which FAT32 cannot store.",
    "Not enough free space: 1342177280 bytes needed (file plus a safety margin), 1048576 free.",
    "PS5\\UPDATE exists on the drive but is not a folder.",
    "Cancelled. Nothing was changed on the drive.",
    'PS5UPDATE.PUP already exists on the drive. Choose "Replace" to keep the old file as a backup while the new one is written.',
    "The copy on the drive did not match the source, so it was removed. Try again or use another drive.",
    "Disk error: access denied",
    "That file is no longer there.",
    "Could not delete it: busy",
    "Deleted.",
    "Only a backup can be restored. A partly written file can only be deleted.",
    "PS5UPDATE.PUP already exists, so the backup was not restored. Delete the backup instead.",
    "Could not restore it: busy",
    "Restored as PS5UPDATE.PUP.",
    "That is not a file this app created.",
    "Download cancelled.",
    "Sony's server answered HTTP 404, so the file is not available from there right now.",
    "Sony's server reports 100 bytes but the catalog expects 200. Nothing was saved.",
    "The connection ended after 3 of 10 bytes. Nothing was saved.",
    "Network error: timed out",
    "Could not save the file: disk full",
    "Only standard update and recovery files can be written to a drive by this app.",
    "Sony's server no longer offers this file.",
    "The Midnight Archive's server answered HTTP 404, so the file is not available from there right now.",
    "The Midnight Archive's server reports 100 bytes but the catalog expects 200. Nothing was saved.",
    "The Midnight Archive has no download address for this package.",
    "The sources disagree about this file's checksum or size, so the app will not download it. See the catalog entry.",
    "The downloaded file does not match the checksums published for this entry, so it was deleted.",
    "Can't read that file: not found",
    "That is not a file.",
    "This file does not match the checksums published for the selected entry, so nothing was written to the drive.",
    "A download is already running.",
    "A USB build is already running.",
    "A USB build is running; try again when it has finished.",
    "A refresh is already running.",
    "Sony has no download address for this package.",
    "That catalog entry no longer exists.",
    "The drive is no longer connected.",
    "The drive changed since you chose it. Choose it again.",
    "Building a USB drive is only available on Windows in this version.",
    "Could not list drives: error 5",
    "This is the drive Windows is running from.",
    "The file system could not be read.",
    "The PS5 reads FAT32 or exFAT drives; this one is NTFS. This app never formats drives; format it yourself with the operating system's tools (this erases the drive) and come back.",
    "This is not a USB drive.",
    "Could not confirm this is a USB drive.",
    "The drive has no drive letter.",
    "hashing was cancelled",
    "The catalog could not be saved locally (access denied).",
    "network error: https://x.example/a answered HTTP 403",
    "source layout changed: MewX line 3: bad row",
    "https://x.example/a is larger than the 3145728-byte limit",
    "Sony serves this file as 0 bytes (pulled)",
    "Sony serves 5 bytes but the index lists 9; not offered",
    "Sony did not report a size",
    "No longer served by Sony",
    "Sony answered HTTP 500",
    "Taken from Sony's download URL. MewX documents this path segment as the file's SHA-256; not yet independently confirmed.",
  ];

  it("translates every known message", () => {
    for (const s of samples) {
      const out = loc(s);
      expect(out, s).not.toBe(s);
      expect(out, s).not.toMatch(/\{\w+\}/);
    }
  });

  it("carries numbers and nested details through", () => {
    setCurrentLang("ru");
    expect(loc("Sony's server answered HTTP 404, so the file is not available from there right now.")).toContain("404");
    expect(loc("Not enough free space: 1342177280 bytes needed (file plus a safety margin), 1048576 free.")).toContain(
      "1,25 ГиБ",
    );
    expect(loc("network error: https://x.example/a answered HTTP 403")).toBe(
      "ошибка сети: https://x.example/a ответил HTTP 403",
    );
    expect(loc("The PS5 reads FAT32 or exFAT drives; this one is NTFS. This app never formats drives; x")).toContain("NTFS");
    setCurrentLang("en");
  });

  it("leaves English and unknown text unchanged", () => {
    expect(localizeBackend("The file is empty.", makeT("en"), "en")).toBe("The file is empty.");
    expect(loc("Something nobody planned for.")).toBe("Something nobody planned for.");
  });
});
