# Translation glossary and rules

For translators of `src/i18n/en.ts` into `src/i18n/langs/<code>.ts`. English is the reference. The translations are machine-drafted and not proofread by native speakers; consistency and caution matter more than style.

## Rules for every language

1. Translate the values only. Keys, `{placeholders}`, `**bold**` markers and `` `mono` `` markers stay exactly as in English, in the same number. A string that starts or ends with a space keeps that space (for example `common.and`).
2. **Never say more than the English says.** A checksum match proves the bytes equal what a source published; it does not make a file "authentic", "genuine", "original", "guaranteed", "100% safe", "verified safe" or "jailbreak-compatible". Avoid those words and their equivalents (the per-language patterns in `src/i18n/banned.ts` fail the tests). "Official" is fine only where English says "official" (Sony as the publisher).
3. Warnings (data loss, the internet update, the exploit ending, community-reported risk) keep the same strength: do not soften or sharpen them. "Community-reported" stays attributed to the community.
4. Product and source names stay as they are: VoidRecoverPS5, PS5, PlayStation, Sony, MewX, Darthsternie, Y2JB, PS5Dev, PSDevWiki, GitHub, Windows, WebView2, SmartScreen, MEGA, 1fichier. Technical tokens stay: PUP, SHA-256, MD5, FAT32, exFAT, USB, GiB/MiB/KiB (formatted by code, not in strings), HTTP, URL, `PS5\\UPDATE\\PS5UPDATE.PUP`, file names, version numbers, `sys_ex`, `.bak`, `.tmp`.
5. **Console menu names** (Safe Mode, Update System Software, Update from USB Storage Device, Reset PS5 (Reinstall System Software), Clear Cache and Rebuild Database): the console's menu wording differs by region and has not been checked. Give the name in your language, and put the exact English name in brackets after it where it first appears in a string, for example German "Abgesicherter Modus (Safe Mode)". Do not invent a menu name that sounds official.
6. Plain, neutral wording for a technical tool. No jokes, no marketing, no exclamation marks that English doesn't have.
7. Keep UI labels short: buttons, menu items and badges should be at most about 1.4 times the English length. Long explanatory text may be as long as it needs.
8. Prefer wording without plurals that depend on a number ("Files: 3") exactly as English does. Do not add number agreement the English doesn't have.
9. Keep the order of ideas in a sentence, but reorder words freely for natural grammar. Placeholders may move.
10. Do not translate the keys' comments or add comments.

## Form of address

| Language | Use |
|---|---|
| zh-CN | 您 only where "you" is needed; prefer no pronoun |
| es | tú (informal, as Windows does) |
| fr | vous |
| de | Sie |
| pt-BR | você |
| tr | siz (polite) |
| pl | impersonal or "Ty" in capital-free form; prefer impersonal ("Wybierz plik") |
| ja | です・ます form |
| ko | polite 합니다 / 하세요 form |
| it | tu (informal, as Windows does) |

## Core terms (use consistently)

| English | zh-CN | es | fr | de | pt-BR |
|---|---|---|---|---|---|
| system software / firmware | 系统软件 / 固件 | software del sistema / firmware | logiciel système / firmware | Systemsoftware / Firmware | software do sistema / firmware |
| update package | 更新包 | paquete de actualización | paquet de mise à jour | Update-Paket | pacote de atualização |
| recovery (reinstall) package | 恢复（重装）包 | paquete de recuperación (reinstalación) | paquet de récupération (réinstallation) | Wiederherstellungs-(Neuinstallations-)Paket | pacote de recuperação (reinstalação) |
| checksum | 校验和 | suma de comprobación | somme de contrôle | Prüfsumme | soma de verificação |
| catalog | 目录 | catálogo | catalogue | Katalog | catálogo |
| source (of data) | 来源 | fuente | source | Quelle | fonte |
| drive (USB) | 驱动器 / U 盘 | unidad | lecteur | Laufwerk | unidade |
| write (to the drive) | 写入 | escribir | écrire | schreiben | gravar |
| eject | 弹出 | expulsar | éjecter | auswerfen | ejetar |
| snapshot | 快照 | instantánea | instantané | Schnappschuss (Snapshot) | instantâneo |
| notebook | 笔记本 | cuaderno | carnet | Notizbuch | caderno |
| wizard | 向导 | asistente | assistant | Assistent | assistente |
| mirror (download mirror) | 镜像 | espejo | miroir | Spiegelserver | espelho |
| stale (data) | 已过期 | desactualizado | périmé | veraltet | desatualizado |
| backup (file kept) | 备份 | copia de seguridad | sauvegarde | Sicherung | backup |
| console | 主机 | consola | console | Konsole | console |
| exploit | 漏洞利用 | exploit | exploit | Exploit | exploit |
| reinstall | 重新安装 | reinstalar | réinstaller | neu installieren | reinstalar |

| English | tr | pl | ja | ko | it |
|---|---|---|---|---|---|
| system software / firmware | sistem yazılımı / üretici yazılımı | oprogramowanie systemowe / firmware | システムソフトウェア / ファームウェア | 시스템 소프트웨어 / 펌웨어 | software di sistema / firmware |
| update package | güncelleme paketi | pakiet aktualizacji | アップデートパッケージ | 업데이트 패키지 | pacchetto di aggiornamento |
| recovery (reinstall) package | kurtarma (yeniden yükleme) paketi | pakiet odzyskiwania (ponownej instalacji) | リカバリー（再インストール）パッケージ | 복구(재설치) 패키지 | pacchetto di ripristino (reinstallazione) |
| checksum | sağlama toplamı | suma kontrolna | チェックサム | 체크섬 | checksum |
| catalog | katalog | katalog | カタログ | 카탈로그 | catalogo |
| source (of data) | kaynak | źródło | ソース | 출처 | fonte |
| drive (USB) | sürücü | dysk | ドライブ | 드라이브 | unità |
| write (to the drive) | yazmak | zapisać | 書き込む | 쓰기 | scrivere |
| eject | çıkar | wysuń | 取り出す | 꺼내기 | espelli |
| snapshot | anlık görüntü | migawka | スナップショット | 스냅샷 | snapshot |
| notebook | not defteri | notatnik | ノート | 노트 | taccuino |
| wizard | sihirbaz | kreator | ウィザード | 마법사 | procedura guidata |
| mirror (download mirror) | ayna | serwer lustrzany | ミラー | 미러 | mirror |
| stale (data) | eski | nieaktualne | 古い | 오래됨 | non aggiornato |
| backup (file kept) | yedek | kopia zapasowa | バックアップ | 백업 | copia di backup |
| console | konsol | konsola | 本体 | 본체 | console |
| exploit | exploit | exploit | エクスプロイト | 익스플로잇 | exploit |
| reinstall | yeniden yükle | zainstaluj ponownie | 再インストール | 재설치 | reinstallare |

Russian (already shipped) is the model for tone: see `src/i18n/langs/ru.ts`.

## Checks

`npm run i18n:check` after writing a file: hard errors (missing or extra keys, placeholders, markers, banned wording) must be zero; warnings are reviewed by hand. `npm test` and `npx tsc --noEmit` must pass.
