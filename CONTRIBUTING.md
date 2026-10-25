# Contributing

## Translations

The window text lives in `src/i18n/en.ts` (English, the source of truth) and one file per other language in `src/i18n/langs/<code>.ts`, which exports `dict` with the same keys. Codes: `ru zh-CN es fr de pt-BR tr pl ja ko it`. A language appears in the app's language list as soon as its file exists; a string a file lacks shows in English. Keep `{placeholders}` and the `**bold**` and `` `mono` `` markers exactly as in English.

- `npm run i18n:check` reports, per language, missing and extra keys, placeholder or marker mismatches, banned wording, text identical to English and text much longer than English (a layout risk). Hard errors fail `npm test` too.
- `npm run i18n:missing -- de` prints what a language still lacks.
- Each shipped language also needs its own list of banned overclaiming words in `src/i18n/banned.ts` (a checksum match is never "authentic" or "safe"); the tests fail without it.
- Messages the Rust side returns in English are matched in `src/i18n/backend.ts` (one rule per message, with a key in every dictionary); a message without a rule shows in English. Source notes are the `src.<id>.note` keys.
- The translations are machine-drafted and have not been proofread by native speakers; English is the reference. Corrections are welcome as pull requests. The installer languages are listed in `src-tauri/tauri.conf.json` (Tauri bundles the installer's own strings for most; Polish uses `src-tauri/installer/Polish.nsh`).

## Ground rules

- **Never attach firmware files**, mirror links, keys or captures that contain them. If a capture is needed to explain a bug, describe it or trim it to the lines that matter.
- The app stays read-only toward consoles and never formats drives. Pull requests that add formatting, run or unpack firmware, or send data anywhere will be closed.
- Wording: no claim stronger than the evidence. A checksum match is not "authentic" or "safe"; `npm test` fails on a list of such phrases in English and Russian.
- Network requests are https only, including redirects. The source registry (`manifests/sources.json`) is data only and compiled into the app.
- Run the checks before sending a change: `npm test`, `npm run typecheck`, and in `src-tauri`: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. There is no automated build service.
- Windows paths and behaviour first. A macOS port is planned; Linux isn't.

## Useful reports

Another source whose layout changed, a file Sony stopped serving, a drive that is wrongly allowed or blocked (with its file system and how it connects), and, above all, results from a real console: what the prompt said, which file it accepted and which error it gave for a wrong one.

## Making a release (maintainer)

Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, add a section to `CHANGELOG.md`, regenerate the screenshots in `docs/images`, run the checks above, then `npm run tauri build` (use a short `CARGO_TARGET_DIR`). Attach the installer, its SHA-256, and the two walkthrough videos to a draft release and read it before publishing.
