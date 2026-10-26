# Phases

This file tracks what is built. Windows x64 first; macOS and code signing come after the Windows beta.

## Phase 1 — catalog and file check (done)

- Typed domain model: package types (update / reinstall / special variant / unknown), raw source labels kept next to normalized fields, nullable and approximate sizes, per-source hash claims, mirror scope and availability.
- Source registry (`manifests/sources.json`) compiled into the app and validated: no executable-looking fields, no unknown fields, https only, known adapters only.
- Adapters, each failing closed on layout changes and tested against fixtures:
  - MewX README tables (SHA-256, MD5, size, build date, sys_ex rows),
  - Sony support page (current update and recovery links) plus Sony CDN addresses built from the index, with HEAD availability checks (404 or 0 bytes = unavailable),
  - Darthsternie archive via the WordPress REST API (Official / Recovery tables, sys_ex suffixes, blank sizes, file vs folder links),
  - Midnight Channel Archives PS5 listings (update and recovery tables with SHA-256, MD5 and rounded sizes; download addresses are built from the validated SHA-256, never from page text; the two pages are fetched five seconds apart).
- Refresh keeps a failed source's previous records as stale, detects content changes by hash, and caches locally.
- Catalog merge per version/type with conflicts, cross-source matches and `derived_from` independence.
- Streaming SHA-256 + MD5 of a chosen file with progress and cancel; precise result wording.
- UI: home, catalog with filters and detail panel, file check, sources, about. External links open only after a prompt naming the domain.
- Tests: Rust unit tests, an opt-in live-source test, frontend unit tests (filtering and a banned-wording guard).

## Phase 2 — recovery USB builder (code complete, needs real-drive testing)

- Windows volume adapter (`volumes/`): drives are listed by Volume GUID path with volume serial, file system, drive type and USB bus type (`IOCTL_STORAGE_QUERY_PROPERTY`, opened with zero access). Anything unknown blocks the drive; the Windows system drive is never offered. The drive is re-read and compared (id, serial, file system, bus) right before writing.
- Builder (`builder.rs`): FAT32/exFAT only, free space from the real file size plus a 64 MiB margin, 4 GiB limit on FAT32, `PS5/UPDATE/` created if missing, `.tmp` write while hashing, flush, re-read and re-hash, existing file kept as `.bak` until the final file re-hashes correctly, undo on any failure, warning for other `.PUP` files. Only update and recovery entries without a variant label can enter. The file is accepted only if its hashes match every checksum published for the selected entry; otherwise the temp file is removed and nothing is moved into place.
- Leftover `.tmp`/`.bak` files (name pattern `PS5UPDATE.PUP.<8 hex>.tmp|bak`) are found on allowed drives when the Build page opens; the user can delete them or restore a backup.
- Direct download (`download.rs`): the address comes from the catalog, https on every hop, no whole-request timeout but a 30 s stall limit, size checked against the published size, `.part` file removed on failure or cancel, downloaded file deleted if its checksums don't match.
- UI: "Build a recovery USB" page (package, file, drive, preflight, confirm, progress, result).
- Verified on a real FAT32 USB stick (download, write, read-back, replace, cleanup). Limits: re-reads may be served from the OS cache rather than the device; no exFAT stick test yet.

## Phase 3 — guidance (done)

- Recovery wizard (`src/wizard/logic.ts`, `src/pages/WizardPage.tsx`), English and Russian. Pure decision logic with tests: steps ordered least to most destructive, the internet update is never offered, "stay on my firmware" proposes only the recovery file for the exact reported version and stops when the console's stated minimum is higher than the reported version, other paths show "that version or later". Plans list warnings, source links with a review date, and what is unconfirmed. A proposed file hands off to the Build page.
- Storage notebook (`notebook.rs`, `NotebookPage.tsx`): read-only snapshots of folders the user picks (names, sizes, modified times; links are not followed; 200k entry and 32 level limits), comparison (added, removed, changed, possibly moved), JSON/CSV export with the folder path and account name redacted by default and spreadsheet formulas defused in CSV. PC folders and console USB drives are labelled differently, and the page says the app cannot see the console's internal database.
- Community resources (`src/data/resources.ts`): data-only list with category, provenance, maintainer, last-checked date and a "report" link. No mirror or download links; link-only sites are marked.
- Safe eject: flush, lock, dismount and eject through the Windows storage API, with a plain fallback message. Tested on a real USB drive as a standard user.
- English/Russian UI and installer language choice, extended to twelve languages (see [PLAN-i18n.md](PLAN-i18n.md)).

## Phase 4 — release and walkthrough (next)

- Release build and a launch check of the installer in both languages; README and screenshots section. No CI: builds and tests are run locally (`npm test`, `cargo test`, `cargo clippy`).
- Tutorial assets in English and Russian, produced the same way as the MW4 Texture Streaming walkthrough: scripted UI capture of every page, narration (Piper for Russian), and a rendered video. Done once the app is feature-complete.
- Still open from earlier phases: exFAT stick test, pulling a drive mid-write on real hardware, a console-side test of the written file.

## Later

Windows ARM64, macOS port, code signing, user-added catalogs, optional explicit formatting after a separate review.
