# Plan: twelve languages for VoidRecoverPS5

Goal: the window and the installer in the same twelve languages as the MW4 Texture Streaming Toolkit, with the same behaviour (installer choice carries over, Windows language by default, a switch in the app, English fallback, translations marked as machine-drafted). Today the app has English and Russian.

## Status

- Step 1 (infrastructure): done on `feature/i18n-infrastructure`. Language registry for all twelve, on-demand language files, installer-language handoff through the registry, Windows display language, *Automatic* setting and drop-down, source notes as keys, per-language byte units, CJK font stacks, `npm run i18n:check` / `i18n:missing`, tests. Only English and Russian have text so far; the list shows the languages that have a file.
- Step 2 (glossary and translations): done. Ten languages drafted by machine with `docs/i18n/GLOSSARY.md`, checked by script and by a second read of the warning texts; console menu names are the translators' wording with the English name in brackets and are not confirmed.
- Step 3 (installer): done. The installer offers all twelve languages (a list in each language's own name, preselected from the Windows language) and records the choice. Tauri ships custom installer strings for eleven of them; Polish has no bundled file, so `src-tauri/installer/Polish.nsh` supplies the 27 custom lines. Checked: welcome page of every language on screen, and a full install in Japanese followed by the app opening in Japanese through the installer handoff (the test install was removed again).
- Step 4 (quality check): done. Every page (home, wizard form and plan, catalog and detail, check a file, build, notebook, community, sources, about) was captured in all twelve languages at the normal window size and at the 900-pixel minimum width, with an automatic audit for clipped text, text pushed off screen and sideways page scrolling. The only findings in any language are the catalog table's source-label column (cut with an ellipsis on purpose) and the table's own horizontal scroll at 900 pixels, which are by design. Contact sheets were checked by eye; one label was improved (Chinese "Package" heading). The screenshots stay outside the repository; the README keeps English and Russian screenshots only.
- Step 5 (docs): done. README Languages section and corrected installer, header and Russian note; CONTRIBUTING (translation instructions) and CHANGELOG updated. README screenshots and videos stay English and Russian only.

All five steps are done. What is left is outside this plan: proofreading by native speakers once the repository is public.

## What MW4 does (the model)

- 12 languages: English, Russian, Chinese (Simplified), Spanish, French, German, Portuguese (Brazil), Turkish, Polish, Japanese, Korean, Italian.
- One file per language, English text as the key, 946 strings each; a missing string shows English, so nothing breaks.
- An extract/check tool (`i18n_extract.py --check`, `--missing <code>`) and a test file that fail on missing strings and placeholder mistakes.
- Language: setting `auto` = the language chosen in the installer (`language.txt`), else the Windows display language; or a fixed choice in Settings.
- Installer: language list first (starts on the Windows language), choice saved for the program.
- Logs, reports and file names stay English so bug reports compare.
- README says the translations are machine-drafted and unproofread and asks for corrections as pull requests.

## Where VoidRecoverPS5 is today

- `src/i18n/en.ts` (source of truth, 476 keys, about 3,700 words) and `ru.ts`, typed so a missing or extra key fails `npm run typecheck`; `i18n.test.ts` also checks placeholders and that every `t("…")` key used exists.
- `backend.ts` maps about 78 English messages from the Rust side to keys (regex per message).
- Source notes: English from `manifests/sources.json`, Russian from a separate table (`sourceNotes.ts`). Community notes are already keys (`res.<id>.note`).
- A banned-wording test (no "authentic", "guaranteed" and the like) covers English and Russian only.
- Language switch is an EN | RU pair of buttons; detection reads the browser language; no handoff from the installer; installer offers English and Russian.
- Dates, numbers and byte units follow the language (Russian units Б, КиБ, МиБ, ГиБ); no UI string uses plurals.

## Decisions (defaults match MW4; change any before work starts)

1. **Languages:** the same twelve as MW4 (`en ru zh-CN es fr de pt-BR tr pl ja ko it`). Traditional Chinese Windows gets Simplified, as in MW4. No right-to-left languages (Arabic, Hebrew) for now.
2. **Translation method:** machine-drafted by me in batches, with a glossary, then checked by scripts. Marked as machine-drafted in the README, the About page and CONTRIBUTING. English is the reference; the disclaimer texts are not legal advice in any language.
3. **Switch:** a drop-down with native names and *Automatic* (default). Automatic = installer language, else Windows display language, else English.
4. **Screenshots and videos** stay English and Russian. Other languages get a small comparison table in the README, like MW4's.
5. **Backend messages:** keep the current approach (English sentence from Rust, matched to a key). Moving to message codes is a larger change and not needed for this work.
6. **Docs** (README body, PHASES, plans) stay English, with the Russian note at the top.

## Work, in order

### 1. Infrastructure
- Language registry: code, native name, locale for `Intl`, Windows language id; `LANGS`, `LOCALES`, `NAMES` extended to twelve.
- One file per language, `src/i18n/<code>.ts`, loaded on demand (dynamic import) so start-up stays small; English always bundled.
- Detection: Windows UI language (not just the browser), plus the installer handoff. The NSIS installer already records its language id in the registry (`HKCU\Software\voidfnc\VoidRecoverPS5`, value *Installer Language*, e.g. 1049 for Russian); a small Rust command reads it and maps it to a code. A saved in-app choice wins.
- Move source notes into the dictionaries (`src.<id>.note`), English kept in the manifest, so every language uses one mechanism.
- Formats per language: `Intl` locale, byte units (Russian Б/КиБ…, French o/Kio…, others KiB/MiB/GiB), dates; plural helper generalised with `Intl.PluralRules` (no current string needs it; the rule stays "avoid plurals in new strings").
- Fonts: font stack with Segoe UI plus Microsoft YaHei, Yu Gothic and Malgun Gothic; CJK line-breaking rules; wider layout checks for German and Russian.
- Switch: drop-down in the sidebar footer.
- Tooling: `npm run i18n:check` lists, per language, missing keys, extra keys, placeholder and `**`/backtick marker mismatches, strings identical to English, and strings more than 1.6 times as long as English (overflow risk); `npm run i18n:missing -- de` prints what is left. Wired into `npm test` for the hard errors.
- Tests: key parity, placeholders and markers for all languages; the banned-wording guard extended with a per-language list (equivalents of "authentic", "guaranteed", "100% safe", "jailbreak-compatible", "verified safe"); Automatic resolution (installer id, Windows id, fallback).

### 2. Glossary and translations
- Glossary (`docs/i18n/GLOSSARY.md`): terms that stay as they are (PUP, SHA-256, MD5, FAT32, exFAT, USB, PS5, product and source names) and terms with a fixed translation per language (update package, recovery file, checksum, drive, snapshot, preflight).
- PS5 menu names (Safe Mode, Clear Cache and Rebuild Database, Reset PS5, Update System Software): shown in the language's usual wording with the English name in brackets, because the console's own menu text differs by region and a machine can't confirm it. Marked as not confirmed.
- Translate the 476 keys into ten languages in parallel batches (about 22,000 characters each), then run the checks and fix what they flag. Russian is re-read against the new glossary for consistency.
- Strings that explain risk (data loss, internet update, exploit warnings) get a second read for tone and for any wording stronger than the evidence.

### 3. Installer
- Add the ten languages to `bundle.windows.nsis.languages`. First step is to confirm by a build which of them the Tauri bundler ships custom strings for; any language NSIS ships only in its stock set still works, with the installer's own custom lines in English.
- Language list first (already on), preselected from the Windows language; choice carried to the app (step 1).
- Re-run the installer capture script for a few languages to check the pages.

### 4. Quality check
- Screenshot sweep: the existing capture script gets a language argument; produce every page in every language into a contact sheet and look for overflow, clipped buttons, CJK glyph problems and untranslated text. Fix layout (min widths, wrapping) as needed.
- Check the backend-message mapping by forcing each error path once in a few languages.
- Check *Automatic* from a Windows language other than English if one can be set on the test PC; otherwise by environment override for tests.

### 5. Docs and release
- README: Languages section like MW4's (list, how to switch, machine-drafted notice, a small English | Russian | one more comparison table), top-of-page note stays Russian.
- CONTRIBUTING: how to fix or add a translation (one file, the check commands, the glossary); CHANGELOG entry; PHASES updated.
- No change to the repository's privacy: everything stays on the private repo until you decide.

## Out of scope for now
- Narrated videos beyond English and Russian.
- Proofreading by native speakers (invited through pull requests once the repo is public).
- Right-to-left layout, Traditional Chinese, other languages the NSIS installer supports.

## Sizing and risks
- Size: about 5,000 new strings in total (476 × 10, plus roughly ten source notes). Small enough to bundle; loaded on demand.
- Risks: layout overflow in German, Russian and Polish; CJK fonts; machine translation errors in risk warnings (mitigated by the second read and by English being the reference); the installer's custom strings missing for some languages; PS5 menu names differing from the console's real wording (mitigated with bracketed English).

## Questions for you
1. Same twelve languages as MW4, or add/remove any?
2. OK to draft the translations by machine, labelled as such, as MW4 does?
3. Start with step 1 (infrastructure) now?
