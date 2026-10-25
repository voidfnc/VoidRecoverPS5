// Checks the language files against English.
//   npm run i18n:check                 report for every language file; exit code 1 on a hard error
//   npm run i18n:missing -- de         print the keys a language still lacks as JSON {key: english}
//
// Hard errors: missing or extra keys, empty strings, {placeholder} or **bold**/`mono` marker mismatches,
// words from the banned-wording list. Warnings (shown, not fatal): text identical to English, and text much
// longer than English (a layout risk).
import { existsSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "i18n");
const load = async (file) => import(pathToFileURL(join(root, file)).href);

const { en } = await load("en.ts");
const { LANGUAGES } = await load("registry.ts");
const { BANNED } = await load("banned.ts");
const langDir = join(root, "langs");
const files = existsSync(langDir) ? readdirSync(langDir).filter((f) => f.endsWith(".ts")) : [];
const have = new Map();
for (const f of files) have.set(f.replace(/\.ts$/, ""), (await load(`langs/${f}`)).dict);

const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const markers = (s) => `${(s.match(/\*\*/g) ?? []).length}/${(s.match(/`/g) ?? []).length}`;
// Strings that are the same in every language (names, units, symbols).
const SAME_OK = /^(app\.name|lang\.|common\.sizeWord|type\.|srcType\.|build\.entryOption|app\.footer)/;
const CJK = new Set(["zh-CN", "ja", "ko"]);

const arg = process.argv.slice(2);
if (arg[0] === "--missing") {
  const code = arg[1];
  const d = have.get(code) ?? {};
  console.log(JSON.stringify(Object.fromEntries(Object.keys(en).filter((k) => !d[k]).map((k) => [k, en[k]])), null, 1));
  process.exit(0);
}

let errors = 0;
const keys = Object.keys(en);
console.log(`English: ${keys.length} keys\n`);
for (const { code, name } of LANGUAGES) {
  if (code === "en") continue;
  const d = have.get(code);
  if (!d) {
    console.log(`${code.padEnd(6)} ${name}: no file yet`);
    continue;
  }
  const errs = [];
  const warns = [];
  const banned = [...(BANNED[code] ?? []), ...BANNED.en];
  if (!BANNED[code]) errs.push(`no banned-wording list for ${code} in src/i18n/banned.ts`);
  for (const k of keys) {
    const v = d[k];
    if (v === undefined) { errs.push(`missing: ${k}`); continue; }
    if (!v.trim()) { errs.push(`empty: ${k}`); continue; }
    if (placeholders(v) !== placeholders(en[k])) errs.push(`placeholders differ: ${k} (${placeholders(en[k])} vs ${placeholders(v)})`);
    if (markers(v) !== markers(en[k])) errs.push(`**/\` markers differ: ${k}`);
    for (const re of banned) if (re.test(v)) errs.push(`banned wording ${re}: ${k}`);
    if (v === en[k] && en[k].length > 12 && /[A-Za-z]{4}/.test(en[k]) && !SAME_OK.test(k)) warns.push(`same as English: ${k}`);
    if (!CJK.has(code) && en[k].length >= 25 && v.length > en[k].length * 1.6) warns.push(`much longer than English (${(v.length / en[k].length).toFixed(1)}x): ${k}`);
  }
  for (const k of Object.keys(d)) if (!(k in en)) errs.push(`extra key: ${k}`);
  errors += errs.length;
  console.log(`${code.padEnd(6)} ${name}: ${keys.length - errs.filter((e) => e.startsWith("missing")).length}/${keys.length} keys, ${errs.length} errors, ${warns.length} warnings`);
  for (const e of errs.slice(0, 20)) console.log(`   ERROR ${e}`);
  if (errs.length > 20) console.log(`   ... ${errs.length - 20} more errors`);
  for (const w of warns.slice(0, 8)) console.log(`   warn  ${w}`);
  if (warns.length > 8) console.log(`   ... ${warns.length - 8} more warnings`);
}
console.log(errors ? `\n${errors} hard errors` : "\nno hard errors");
process.exit(errors ? 1 : 0);
