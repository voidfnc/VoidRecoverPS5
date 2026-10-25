// The Rust side returns English sentences. This maps each known template to a translation key so the
// UI can show it in the active language; anything unrecognised is shown as received.

import { formatBytes } from "../lib/format";
import type { Key } from "./en";
import type { Lang, T, Vars } from "./lang";

interface Rule {
  re: RegExp;
  key: Key;
  /** Names the capture groups; "bytes" groups are shown as sizes, "inner" groups are translated too. */
  groups?: [string, "text" | "bytes" | "inner"][];
}

const R = (re: RegExp, key: Key, groups?: Rule["groups"]): Rule => ({ re, key, groups });

const RULES: Rule[] = [
  R(/^The drive has no plain drive letter\.$/, "bk.noPlainLetter"),
  R(/^Windows would not let this app open the drive for ejecting\.$/, "bk.ejectOpen"),
  R(/^Windows could not flush the drive\.$/, "bk.ejectFlush"),
  R(/^Another program is still using the drive, so it can't be ejected yet\.$/, "bk.ejectBusy"),
  R(/^Windows could not dismount the drive\.$/, "bk.ejectDismount"),
  R(/^Windows could not unlock the drive for removal\.$/, "bk.ejectUnlock"),
  R(/^Windows could not eject the drive\.$/, "bk.ejectFail"),
  R(/^Ejecting from this app isn't available on this system\.$/, "bk.ejectUnavailable"),
  R(/^Scan cancelled\.$/, "bk.scanCancelled"),
  R(/^Can't read that folder: ([\s\S]*)$/, "bk.cantReadFolder", [["detail", "text"]]),
  R(/^Choose a folder \(not a link or a file\)\.$/, "bk.chooseFolder"),
  R(/^A scan is already running\.$/, "bk.scanRunning"),
  R(/^The notebook holds at most (\d+) snapshots\. Delete one first\.$/, "bk.notebookFull", [["n", "text"]]),
  R(/^That snapshot no longer exists\.$/, "bk.snapshotGone"),
  R(/^The first snapshot no longer exists\.$/, "bk.snapshotGone1"),
  R(/^The second snapshot no longer exists\.$/, "bk.snapshotGone2"),
  R(/^The file name must end in \.json or \.csv/, "bk.exportExt"),
  R(/^Could not write the file: ([\s\S]*)$/, "bk.exportWrite", [["detail", "text"]]),
  R(/^The notebook could not be saved: ([\s\S]*)$/, "bk.notebookSave", [["detail", "text"]]),
  R(/^The notebook file was too large and was ignored\.$/, "bk.notebookTooLarge"),
  R(/^The notebook is from another app version and was ignored\.$/, "bk.notebookOtherVersion"),
  R(/^The notebook could not be read and was ignored \(([\s\S]*)\)\.$/, "bk.notebookUnreadable", [["detail", "text"]]),
  R(/^The file is empty\.$/, "bk.fileEmpty"),
  R(/^The file is larger than 4 GiB, which FAT32 cannot store\.$/, "bk.fileOver4g"),
  R(/^Not enough free space: (\d+) bytes needed \(file plus a safety margin\), (\d+) free\.$/, "bk.notEnoughSpace", [
    ["needed", "bytes"],
    ["free", "bytes"],
  ]),
  R(/^PS5\\UPDATE exists on the drive but is not a folder\.$/, "bk.updateNotFolder"),
  R(/^Cancelled\. Nothing was changed on the drive\.$/, "bk.cancelledNothing"),
  R(/^PS5UPDATE\.PUP already exists on the drive\. Choose "Replace"/, "bk.targetExists"),
  R(/^The copy on the drive did not match the source/, "bk.copyMismatch"),
  R(/^Disk error: ([\s\S]*)$/, "bk.diskError", [["detail", "text"]]),
  R(/^That file is no longer there\.$/, "bk.leftoverGone"),
  R(/^Could not delete it: ([\s\S]*)$/, "bk.couldNotDelete", [["detail", "text"]]),
  R(/^Deleted\.$/, "bk.deleted"),
  R(/^Only a backup can be restored\./, "bk.onlyBackupRestore"),
  R(/^PS5UPDATE\.PUP already exists, so the backup was not restored\./, "bk.restoreBlocked"),
  R(/^Could not restore it: ([\s\S]*)$/, "bk.couldNotRestore", [["detail", "text"]]),
  R(/^Restored as PS5UPDATE\.PUP\.$/, "bk.restored"),
  R(/^That is not a file this app created\.$/, "bk.notOurs"),
  R(/^Download cancelled\.$/, "bk.downloadCancelled"),
  R(/^Sony's server answered HTTP (\d+), so the file is not available/, "bk.sonyHttp", [["status", "text"]]),
  R(/^Sony's server reports (\d+) bytes but the catalog expects (\d+)\./, "bk.sizeMismatch", [
    ["reported", "text"],
    ["expected", "text"],
  ]),
  R(/^The connection ended after (\d+) of (\d+) bytes\./, "bk.truncated", [
    ["got", "text"],
    ["expected", "text"],
  ]),
  R(/^Network error: ([\s\S]*)$/, "bk.networkError", [["detail", "inner"]]),
  R(/^Could not save the file: ([\s\S]*)$/, "bk.couldNotSave", [["detail", "text"]]),
  R(/^Only standard update and recovery files can be written/, "bk.onlyStandard"),
  R(/^Sony's server no longer offers this file\.$/, "bk.sonyNoLonger"),
  R(/^The downloaded file does not match the checksums published/, "bk.downloadedNoMatch"),
  R(/^Can't read that file: ([\s\S]*)$/, "bk.cantRead", [["detail", "text"]]),
  R(/^That is not a file\.$/, "bk.notAFile"),
  R(/^This file does not match the checksums published for the selected entry/, "bk.fileNoMatch"),
  R(/^A download is already running\.$/, "bk.downloadRunning"),
  R(/^A USB build is (?:already|still) running\.$/, "bk.buildRunning"),
  R(/^A USB build is running; try again when it has finished\.$/, "bk.buildRunningRetry"),
  R(/^A refresh is already running\.$/, "bk.refreshRunning"),
  R(/^Sony has no download address for this package\.$/, "bk.noSonyAddress"),
  R(/^That catalog entry no longer exists\.$/, "bk.entryGone"),
  R(/^The drive is no longer connected\.$/, "bk.driveGone"),
  R(/^The drive changed since you chose it\./, "bk.driveChanged"),
  R(/^Building a USB drive is only available on Windows/, "bk.windowsOnly"),
  R(/^Could not list drives: ([\s\S]*)$/, "bk.couldNotList", [["detail", "text"]]),
  R(/^This is the drive Windows is running from\.$/, "bk.driveSystem"),
  R(/^The file system could not be read\.$/, "bk.fsUnreadable"),
  R(/^The PS5 reads FAT32 or exFAT drives; this one is (\S+?)\. This app never formats/, "bk.fsWrong", [
    ["fs", "text"],
  ]),
  R(/^This is not a USB drive\.$/, "bk.notUsb"),
  R(/^Could not confirm this is a USB drive\.$/, "bk.usbUnconfirmed"),
  R(/^The drive has no drive letter\.$/, "bk.noDriveLetter"),
  R(/^hashing was cancelled$/, "bk.hashCancelled"),
  R(/^The catalog could not be saved locally \(([\s\S]*)\)\.$/, "bk.catalogNotSaved", [["detail", "text"]]),
  R(/^network error: ([\s\S]*)$/, "bk.adapterNetwork", [["detail", "inner"]]),
  R(/^source layout changed: ([\s\S]*)$/, "bk.adapterLayout", [["detail", "text"]]),
  R(/^(\S+) is larger than the (\d+)-byte limit$/, "bk.adapterTooLarge", [
    ["url", "text"],
    ["limit", "text"],
  ]),
  R(/^(\S+) answered HTTP (\d+)$/, "bk.answeredHttp", [
    ["url", "text"],
    ["status", "text"],
  ]),
  R(/^Sony serves this file as 0 bytes \(pulled\)$/, "bk.sony0Bytes"),
  R(/^Sony serves (\d+) bytes but the index lists (\d+); not offered$/, "bk.sonySizeDiffers", [
    ["served", "text"],
    ["listed", "text"],
  ]),
  R(/^Sony did not report a size$/, "bk.sonyNoSize"),
  R(/^No longer served by Sony$/, "bk.sonyNoLongerServed"),
  R(/^Sony answered HTTP (\d+)$/, "bk.sonyAnswered", [["status", "text"]]),
  R(/^Taken from Sony's download URL\. MewX documents this path segment/, "bk.urlHashNote"),
];

/** Translate one backend message; English and unknown messages come back unchanged. */
export function localizeBackend(msg: string, t: T, lang: Lang): string {
  if (lang === "en") return msg;
  for (const rule of RULES) {
    const m = rule.re.exec(msg);
    if (!m) continue;
    const vars: Vars = {};
    (rule.groups ?? []).forEach(([name, kind], i) => {
      const raw = m[i + 1] ?? "";
      vars[name] = kind === "bytes" ? formatBytes(Number(raw)) : kind === "inner" ? localizeBackend(raw, t, lang) : raw;
    });
    return t(rule.key, vars);
  }
  return msg;
}
