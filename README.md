# VoidRecoverPS5

Beta 0.1.0 · Windows 10/11 x64 · 12 languages · installer (nothing else to install) · from source: Rust and Node.js

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/E4E029CR36) [![Discord](https://img.shields.io/badge/Discord-Join%20the%20server-5865F2?logo=discord&logoColor=white)](https://discord.gg/rcNfF5AvMn)

[Video](#video-walkthrough) · [Видео на русском](#видео-на-русском) · [Screenshots](#what-it-looks-like) · [Download](#download) · [Releases](https://github.com/voidfnc/VoidRecoverPS5/releases)

> **Русский язык.** Окно программы и установщик есть на русском (и ещё на десяти языках, см. [Languages](#languages)): язык выбирается в установщике и в списке «Язык» в боковой панели. Переводы сделаны машинным способом и не проверялись носителями языка, поэтому возможны ошибки; исправления приветствуются (см. [CONTRIBUTING.md](CONTRIBUTING.md)). Описание на этой странице пока только на английском. [Видео-обзор на русском](#видео-на-русском).

When a PS5 stops starting normally, the fix usually depends on one specific file: the right system software version, in the right form (update or recovery), on a USB drive laid out the way the console expects. The checksums for those files are scattered across Sony's page and a couple of community archives, they don't always agree, and a wrong file can cost you your data or your firmware version. VoidRecoverPS5 puts the sources side by side, checks the file you downloaded, builds the USB drive without touching anything else on it, and walks you through the gentlest recovery steps first.

> **Please read this first.** This is an **unofficial community tool**. It is not affiliated with, endorsed by or supported by Sony Interactive Entertainment. "PlayStation" and "PS5" are trademarks of their owners and appear here only to say which console the tool is for.
> The repository contains **source code and written notes only**: no Sony firmware, no keys, no download mirror links (the app reads those from the source pages when you ask it to, and never stores them in the code). Please don't attach firmware files to issues or pull requests.
> The app prepares and checks files on your computer. It **never installs anything on a console** and can't promise that a console will recover or stay on a particular firmware. It never formats, erases or repartitions a drive. A matching checksum means the file's bytes equal what a source published; it does not prove who made the file, that it is safe, or that your console will accept it.
> Using it is your decision and your responsibility. Modifying a console can go against its terms of service or the law where you live. No warranty (see [LICENSE](LICENSE) and [DISCLAIMER.md](DISCLAIMER.md)).

## Video walkthrough

A short video (about two and a half minutes) goes through the whole app: the catalog, checking a file, the recovery wizard, building the USB drive (download, drive choice, the preflight summary, the write and the read-back check), ejecting the drive, the storage notebook and the community list. It plays here when the video is attached to this page; if it doesn't load for you, [download the file from the 0.1.0-beta release](https://github.com/voidfnc/VoidRecoverPS5/releases/download/v0.1.0-beta/VoidRecoverPS5-Walkthrough.mp4) (about 38 MB).

<video src="https://github.com/voidfnc/VoidRecoverPS5/releases/download/v0.1.0-beta/VoidRecoverPS5-Walkthrough.mp4" controls width="100%"></video>

The program window in it is the real thing (version 0.1.0-beta, a real download from Sony's server and a real FAT32 USB drive, a throw-away folder for the notebook), with captions on top. The voice, music and sound effects are computer-generated, so don't expect a polished production.

## Видео на русском

Тот же обзор на русском языке (около трёх с половиной минут): окно программы, подписи и закадровый голос. Файл можно [скачать из релиза 0.1.0-beta](https://github.com/voidfnc/VoidRecoverPS5/releases/download/v0.1.0-beta/VoidRecoverPS5-Walkthrough-ru.mp4) (около 56 МБ); встроенный плеер на этой странице появится позже.

В видео показано настоящее окно программы (версия 0.1.0-beta, настоящая загрузка с сервера Sony и настоящий USB-накопитель FAT32, временная папка для блокнота). Голос (Piper, модель ru_RU-irina-medium), музыка и звуки созданы компьютером, так что это не профессиональная постановка.

## What it looks like

**The installer** starts with a list of the twelve languages (preselected from your Windows language), installs for the current user only (no administrator rights), and can add a desktop shortcut. Cancelling at any point installs nothing.

![Setup language list](docs/images/installer-language.png)
![Welcome page](docs/images/installer-1-welcome.png)
![Setup complete](docs/images/installer-2-complete.png)
![Finish: run the program, desktop shortcut](docs/images/installer-3-finish.png)

**The program.** Screenshots of version **0.1.0-beta** on Windows 11, taken from the running app. The USB drive in them is a real FAT32 stick; the folder in the notebook is a throw-away demo folder. They are regenerated when the windows change.

**Home.** The sources it reads, Sony's current release, how many catalog entries there are, and what you can do.

![Home](docs/images/home.png)

**Firmware catalog.** Every known update and recovery file, with every source's label, size and checksum side by side. Pick a row for the details: where each value comes from, which sources agree, whether Sony still serves the file.

![The catalog](docs/images/catalog.png)
![Details of one entry](docs/images/catalog-detail.png)

**Check a file.** Pick a PUP; the app reads it once and compares its SHA-256 and MD5 with every published value. The file is only read, never changed, run or sent anywhere.

![A checked file: the checksums match the catalog entry](docs/images/verify-result.png)

**Recovery wizard.** It asks what the console does and what matters most, then lists steps from the gentlest to the most destructive. It never offers the internet update, and "stay on my firmware" proposes only the recovery file for exactly the version you report (and stops if the console's stated minimum is higher).

![Wizard questions](docs/images/wizard-form.png)
![The plan: gentlest step first, with warnings](docs/images/wizard-plan.png)

**Build a recovery USB.** Pick the package, download it from Sony (with a size check) or choose a file you already have, then pick the drive. Only USB drives with FAT32 or exFAT can be chosen; the Windows drive and anything else is listed but greyed out with the reason.

![Downloading from Sony](docs/images/build-downloading.png)
![Choosing the drive](docs/images/build-drive.png)

Before anything is written the app shows exactly what it will do. It writes one file, `PS5\UPDATE\PS5UPDATE.PUP`, through a temporary name, reads it back from the drive and compares the checksums again. A file that doesn't match the catalog entry is never put in place, and an existing file is kept as a backup until the new one has passed.

![The summary before writing](docs/images/build-preflight.png)
![Writing to the drive](docs/images/build-writing.png)
![Done, with the next steps and an Eject button](docs/images/build-done.png)

**Storage notebook.** Read-only snapshots of folders you choose, a comparison between two snapshots (added, removed, changed, possibly moved) and JSON/CSV export with your user name and the folder's full path hidden by default. It only sees what a PC sees, never the PS5's internal database.

![Snapshots and a comparison](docs/images/notebook-compare.png)

**Community resources** is a data-only list of references, archives and guides, with who maintains each one and when the link was last checked. Nothing on it is ever run. **Sources** shows where the catalog comes from and when each source was last read.

![Community resources](docs/images/community.png)
![Sources](docs/images/sources.png)

**In Russian.** The same screens with the Russian text (the program follows your Windows language the first time, and the language list in the sidebar changes it any time; see [Languages](#languages)).

<details>
<summary>Show the screenshots in Russian (Русский)</summary>

![Установщик: приветствие](docs/images/ru/installer-1-welcome.png)
![Установщик: установка завершена](docs/images/ru/installer-2-complete.png)
![Главная](docs/images/ru/home.png)
![Каталог прошивок](docs/images/ru/catalog.png)
![Сведения о записи каталога](docs/images/ru/catalog-detail.png)
![Проверка файла](docs/images/ru/verify-result.png)
![Мастер восстановления: вопросы](docs/images/ru/wizard-form.png)
![Мастер восстановления: план](docs/images/ru/wizard-plan.png)
![Загрузка с сервера Sony](docs/images/ru/build-downloading.png)
![Выбор накопителя](docs/images/ru/build-drive.png)
![Сводка перед записью](docs/images/ru/build-preflight.png)
![Запись на накопитель](docs/images/ru/build-writing.png)
![Готово: извлечение накопителя](docs/images/ru/build-done.png)
![Блокнот накопителей](docs/images/ru/notebook-compare.png)
![Ресурсы сообщества](docs/images/ru/community.png)
![Источники](docs/images/ru/sources.png)

</details>

## What the app does

- **Firmware catalog.** Every known PS5 update and recovery file, merged from four sources: Sony's support page (the current release) and Sony's download server (older files it still serves), the [MewX PS5 official firmware list](https://github.com/MewX/ps5-official-firmware-list) (SHA-256, MD5, sizes, build dates), [Darthsternie's PS5 firmware archive](https://darthsternie.net/ps5-firmwares/) (MD5s and mirror links) and the [Midnight Channel Archives](https://archive.midnightchannel.net/SonyPS/Firmware/index.php?cat=PS5SYS) (SHA-256 and MD5 for update and recovery files, and a download by SHA-256). Entries where Sony shipped more than one build under a version number are split by build date. The catalog says when one source is built from another, so their agreement isn't counted as independent, and flags a source that is behind Sony's latest release.
- **Check a file.** SHA-256 and MD5 in one streaming pass with progress and cancel. Precise wording: "checksum matches the published value", "mismatch" (any disagreeing value counts) or "no published checksum". A file over FAT32's 4 GiB limit is flagged.
- **Sony availability.** One HEAD request tells you whether Sony still serves an older file. A file served as 0 bytes or with a different size is shown as unavailable.
- **Direct download.** From Sony's server, or from the Midnight Archive when you choose its own button (for files Sony no longer serves). https on every redirect hop, a size check against the published size, progress, cancel, and removal of partial files. The address always comes from the catalog, never from the window. A download that doesn't match the published checksums is deleted, and an entry whose sources disagree about its checksum or size is not downloaded at all.
- **USB builder.** The target drive is identified by its volume GUID, serial number, file system and USB bus type, and is re-read right before writing; anything unknown blocks it. FAT32 and exFAT only, free space checked from the real file size plus a margin, FAT32's 4 GiB limit enforced. The file is written to a temporary name, flushed, re-read and re-hashed, any existing file is kept as a backup until the final file passes the same check, and a failure undoes exactly what the run created. Leftover temporary or backup files from an interrupted run are offered for deletion or restore. Only standard update and recovery files can enter; special variants and unknown packages can't. It can eject the drive afterwards.
- **Recovery wizard.** Steps ordered from least to most destructive; "stay on my firmware" proposes only the recovery file for the exact version you report, and stops when the console's stated minimum is higher; warnings about data loss, the internet update, and the community-reported risk of rebuilding the database on a modified console; source links with a review date; and a list of what is still unconfirmed.
- **Storage notebook.** Up to 30 snapshots of folders you choose (names, sizes, modified times; links are not followed). Compare, and export JSON/CSV with the user name and path redacted by default and spreadsheet formulas defused in CSV.
- **Community resources.** A reviewed list of references, with category, maintainer, the kind of page it is (the maintainer's own documentation, a wiki, a write-up, a forum) and a "report a stale link" link. Sites that block automated reading are marked link-only.
- **Twelve languages.** English, Russian, Chinese (Simplified), Spanish, French, German, Portuguese (Brazil), Turkish, Polish, Japanese, Korean and Italian, for the window and the installer. The window follows the language chosen in the installer, then your Windows language, or the one you pick in the sidebar. See [Languages](#languages).

## Download

Pick one. The files are on the [Releases page](https://github.com/voidfnc/VoidRecoverPS5/releases) (while the versions are betas they are marked *Pre-release*).

| | What you get | Who it is for |
|---|---|---|
| **Installer** (recommended) | `VoidRecoverPS5_<version>_x64-setup.exe`. Double-click, Next, Install. Installs for the current user (no administrator rights), offers its twelve languages first, an optional desktop shortcut, and uninstalls from *Apps & features*. | Most people. |
| **From source** | Clone this repository and build it (see [Build from source](#build-from-source-windows)). | Developers and advanced users. |

Windows may say "Windows protected your PC" (SmartScreen) because the files are not code-signed yet: choose *More info*, then *Run anyway*. You can check a download against the checksum on the release page (`certutil -hashfile <file> SHA256`). The app needs no administrator rights; writing to a USB drive uses the normal rights of your account.

## Build from source (Windows)

Requirements: [Rust](https://rustup.rs) (stable, MSVC toolchain), Node.js 20+, Visual Studio Build Tools with the C++ workload, and WebView2 (preinstalled on Windows 10 1803+ and Windows 11).

```
git clone https://github.com/voidfnc/VoidRecoverPS5
cd VoidRecoverPS5
npm install
npm run tauri dev      # run in development
npm run tauri build    # NSIS installer in src-tauri/target/release/bundle/nsis
```

If the build fails with a linker error about a missing file (LNK1104) in a deeply nested folder, set a shorter `CARGO_TARGET_DIR`, for example `C:\vr5t` (Windows' 260-character path limit).

## Tests

There is no automated build service; tests run on your machine.

```
cd src-tauri
cargo test                                             # Rust unit tests (fixtures only, no network)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test --test live_sources -- --ignored --nocapture  # optional: parse today's live source pages
cd ..
npm test                                               # frontend unit tests
npm run typecheck
```

Source adapters are tested against saved fixtures. Mirror links in fixtures are synthetic; real mirror links are fetched at runtime and are never committed. An optional, ignored test (`cargo test --test live_usb -- --ignored`, with `VR5_LIVE_USB_MOUNT` set to a disposable USB drive) downloads Sony's current update and writes it to that drive; read its header before running it.

## If something doesn't work

- **A USB drive is greyed out.** The reason is shown under it: not USB, a file system other than FAT32 or exFAT, no drive letter, or it is the drive Windows runs from. The app never formats a drive; format it yourself with Windows' tools (this erases it) and come back.
- **"Another program is still using the drive" when ejecting.** Close File Explorer windows and anything that has files open on the drive, then press *Eject the drive* again, or use Windows' *Safely Remove Hardware*.
- **A source shows as stale or failed.** Its earlier data is kept and marked; press *Refresh sources* again. If a source changed its page layout, the adapter stops instead of guessing, and the Sources page says so.
- **Sony shows a file as unavailable.** Sony pulls older files over time (some are served as 0 bytes). Pick another source from the entry's details and check the file you got with *Check a file*.
- **The write stops with "does not match the checksums".** Nothing was written to the drive. Download the file again (or from another source) and check it first.

## What I know and what I don't

What I tested: the full flow on a real FAT32 USB stick (download from Sony, write, read-back, replace an existing file, eject), the checksum comparison against Sony's current update (which matches the catalog, and shows that the 64-hex segment in Sony's download address is the file's SHA-256), the notebook against a real folder, the installer in both languages, and the full test suite.

What is not confirmed yet:

- I have not tested a written file on a console. The exact wording of the PS5's "version X or later" prompt, the error codes for a wrong or old file, and whether *Reset PS5* rejects an update-type file are not confirmed.
- Whether the console accepts FAT16, GPT or multi-partition drives. Community advice is a single MBR partition.
- exFAT drives and pulling a drive in the middle of a write have not been tried on real hardware yet.
- The read-back check may be served from the operating system's cache rather than the drive itself.
- Everything was developed and tested on one Windows 11 PC. All translations are machine-drafted and not proofread by native speakers. The console menu names in them (Safe Mode and the rest) are my wording with the English name in brackets, and have not been checked against a real console in any language.

## Languages

The program and its installer speak English, Russian, Chinese (Simplified), Spanish, French, German, Portuguese (Brazil), Turkish, Polish, Japanese, Korean and Italian. The installer shows a language list first (it starts on your Windows display language), and the program follows that choice. To change it later, use the language list at the bottom of the sidebar; *Automatic* follows the installer's choice, then Windows. Windows set to Traditional Chinese gets the Simplified Chinese text, and European Portuguese gets Brazilian. A string a language doesn't have shows in English, and messages from the program's own engine that have no translation yet show in English too.

The translations were drafted by machine and have not been proofread by native speakers; English is the reference text. The wording that says a checksum proves nothing about safety is checked for every language by a list of words each language must not use. The PS5's own menu names (Safe Mode, Reset PS5 and so on) vary by region and are shown in the translator's wording with the exact English name in brackets. Corrections are welcome as pull requests (see [CONTRIBUTING.md](CONTRIBUTING.md); each language is one file, and `npm run i18n:check` lists what is missing or wrong). The screenshots and videos on this page are in English and Russian only.

| English | Русский |
|---|---|
| ![Check a file, in English](docs/images/verify-result.png) | ![Проверка файла, по-русски](docs/images/ru/verify-result.png) |
| ![Build: choosing the drive, in English](docs/images/build-drive.png) | ![Сборка: выбор накопителя, по-русски](docs/images/ru/build-drive.png) |
| ![Recovery wizard plan, in English](docs/images/wizard-plan.png) | ![План мастера восстановления, по-русски](docs/images/ru/wizard-plan.png) |

## Privacy

No telemetry, accounts or uploads. See [PRIVACY.md](PRIVACY.md) for exactly when the app contacts the network.

## Contributing and issues

Bug reports and ideas are welcome ([CONTRIBUTING.md](CONTRIBUTING.md)). Please never attach firmware files. Security-related reports: see [SECURITY.md](SECURITY.md).

## Project layout

```
src/                 React + TypeScript UI (pages, i18n, wizard logic, community data)
src-tauri/src/
  domain/            types, versions, source registry validation, catalog merge
  sources/           https client and the MewX, Sony, Darthsternie and Midnight Archive adapters
  volumes/           drive discovery behind a trait (Windows adapter) and eject
  builder.rs         the safe write of PS5UPDATE.PUP and leftover recovery
  download.rs        size-checked https download
  integrity.rs       streaming SHA-256/MD5 and comparison
  notebook.rs        folder snapshots, comparison, redacted export
  usb.rs, commands.rs, notebook_commands.rs   the only actions the UI can ask for
manifests/           source registry, compiled into the app
tests/fixtures/      saved source pages for adapter tests
docs/                phase plan, translation notes, screenshots
```

## License

MIT for the code in this repository. See [LICENSE](LICENSE). It covers my code and notes only and gives no rights to any firmware, asset or trademark.
