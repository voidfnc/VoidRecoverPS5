# Changelog

## 0.1.1-beta
**Midnight Archive.**

- New source: the Midnight Channel Archives' PS5 update and recovery listings (SHA-256, MD5, rounded sizes), read on refresh with the five-second pause its robots.txt asks for. Layout changes fail closed like the other sources.
- Build a recovery USB can download from the archive with its own button (the archive finds files by SHA-256). It is never used automatically, the page says it is a third party that sees your IP address, and the download is checked and deleted on a mismatch like any other. The size check uses the exact size from the catalog.
- An entry whose sources disagree about its checksum or size is no longer downloaded, instead of being downloaded and then deleted.
- The catalog now shows places where sources disagree, for example four old update files whose MD5 in the MewX list and Darthsternie's page differs from the archive's (the archive's MD5 matched the bytes in a test download) and recovery files the archive labels with other version numbers.
- Error messages and source notes translated in all twelve languages.

## 0.1.0-beta
**Twelve languages.**

- The window and the installer are available in English, Russian, Chinese (Simplified), Spanish, French, German, Portuguese (Brazil), Turkish, Polish, Japanese, Korean and Italian. Machine-drafted translations, English as the reference; all warning texts were read again against the English for strength.
- Language setting: *Automatic* follows the language chosen in the installer, then the Windows display language; a language list in the sidebar replaces the EN | RU buttons. Language files load on demand.
- Installer: language list with all twelve languages; Polish installer strings added (Tauri has none for Polish). The program picks up the installer's choice.
- Source notes are translated with the rest. Byte units follow the language (Russian, French have their own letters). East Asian font stacks and wrapping rules for long German, Polish, Russian and Turkish words.
- Tooling: `npm run i18n:check` and `npm run i18n:missing`; tests for key parity, placeholders, markers and a banned-wording list per language.

**First beta.**

- **Firmware catalog** merged from Sony's support page and download server, the MewX list and Darthsternie's archive, with per-source values, conflicts, independence of agreement, build-date splits and "source is behind Sony" flags.
- **Check a file:** SHA-256 and MD5 in one pass against every published value.
- **Build a recovery USB:** Windows drive identification (volume GUID, serial, file system, USB bus), FAT32/exFAT only, size-checked download from Sony, write through a temporary name with read-back and re-hash, backup of an existing file until the new one passes, leftover recovery, eject.
- **Recovery wizard:** steps from least to most destructive, never the internet update, exact-version recovery file when staying on the current firmware.
- **Storage notebook:** read-only folder snapshots, comparison, redacted JSON/CSV export.
- **Community resources:** data-only reviewed list.
- Verified on a real FAT32 USB drive. Not yet tried: exFAT drives, pulling a drive during a write, a written file on a console.
