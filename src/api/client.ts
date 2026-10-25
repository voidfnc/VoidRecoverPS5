import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { T } from "../i18n/lang";
import type {
  BuildPhase,
  BuildView,
  CatalogView,
  DownloadView,
  LeftoverGroup,
  NotebookDiff,
  NotebookKind,
  NotebookView,
  LocalHashes,
  PreflightView,
  VerifyReport,
  VolumeInfo,
  VolumeView,
} from "./types";

export const api = {
  getCatalog: () => invoke<CatalogView>("get_catalog"),
  refreshCatalog: () => invoke<CatalogView>("refresh_catalog"),
  probeSony: (key: string) => invoke<CatalogView>("probe_sony", { key }),
  verifyLocalFile: (path: string, selectedKey: string | null) =>
    invoke<VerifyReport>("verify_local_file", { path, selectedKey }),
  compareLocal: (fileName: string, local: LocalHashes, selectedKey: string | null) =>
    invoke<VerifyReport>("compare_local", { fileName, local, selectedKey }),
  cancelVerify: () => invoke<void>("cancel_verify"),
  systemLanguages: () => invoke<{ installer: number | null; windows_ui: number | null }>("system_languages"),
  listVolumes: () => invoke<VolumeView[]>("list_volumes"),
  downloadPackage: (key: string) => invoke<DownloadView>("download_package", { key }),
  cancelDownload: () => invoke<void>("cancel_download"),
  preflightBuild: (key: string, sourcePath: string, volumeId: string) =>
    invoke<PreflightView>("preflight_build", { key, sourcePath, volumeId }),
  buildUsb: (key: string, sourcePath: string, expected: VolumeInfo, replaceExisting: boolean) =>
    invoke<BuildView>("build_usb", { key, sourcePath, expected, replaceExisting }),
  cancelBuild: () => invoke<void>("cancel_build"),
  scanLeftovers: () => invoke<LeftoverGroup[]>("scan_leftovers"),
  ejectVolume: (expected: VolumeInfo) => invoke<void>("eject_volume", { expected }),
  notebookList: () => invoke<NotebookView>("notebook_list"),
  notebookScan: (path: string, label: string, note: string, kind: NotebookKind) =>
    invoke<NotebookView>("notebook_scan", { path, label, note, kind }),
  notebookCancel: () => invoke<void>("notebook_cancel"),
  notebookDelete: (id: string) => invoke<NotebookView>("notebook_delete", { id }),
  notebookCompare: (older: string, newer: string) => invoke<NotebookDiff>("notebook_compare", { older, newer }),
  notebookExport: (id: string, path: string, format: "json" | "csv", redact: boolean) =>
    invoke<void>("notebook_export", { id, path, format, redact }),
  resolveLeftover: (volumeId: string, fileName: string, action: "delete" | "restore") =>
    invoke<string>("resolve_leftover", { volumeId, fileName, action }),
};

export function onDownloadProgress(cb: (done: number, total: number) => void): Promise<UnlistenFn> {
  return listen<{ done: number; total: number }>("download-progress", (e) => cb(e.payload.done, e.payload.total));
}

export function onBuildProgress(cb: (phase: BuildPhase, done: number, total: number) => void): Promise<UnlistenFn> {
  return listen<{ phase: BuildPhase; done: number; total: number }>("build-progress", (e) =>
    cb(e.payload.phase, e.payload.done, e.payload.total),
  );
}

export function onVerifyProgress(cb: (done: number, total: number) => void): Promise<UnlistenFn> {
  return listen<{ done: number; total: number }>("verify-progress", (e) => cb(e.payload.done, e.payload.total));
}

/** Native file picker for a PUP. Returns null when the user cancels. */
export async function pickPupFile(t: T): Promise<string | null> {
  const picked = await openDialog({
    multiple: false,
    directory: false,
    title: t("verify.pickTitle"),
    filters: [
      { name: t("verify.pickPup"), extensions: ["PUP", "pup"] },
      { name: t("verify.pickAll"), extensions: ["*"] },
    ],
  });
  return typeof picked === "string" ? picked : null;
}

/** Opens an https link in the system browser; the capability scope rejects anything else. */
export function openExternal(url: string): Promise<void> {
  return openUrl(url);
}

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}

export function onNotebookProgress(cb: (count: number) => void): Promise<UnlistenFn> {
  return listen<{ count: number }>("notebook-progress", (e) => cb(e.payload.count));
}

/** Native folder picker. Returns null when the user cancels. */
export async function pickFolder(title: string): Promise<string | null> {
  const picked = await openDialog({ multiple: false, directory: true, title });
  return typeof picked === "string" ? picked : null;
}

/** Native save dialog for an export file. Returns null when the user cancels. */
export async function pickSavePath(defaultName: string, ext: "json" | "csv"): Promise<string | null> {
  const picked = await saveDialog({ defaultPath: defaultName, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
  return typeof picked === "string" ? picked : null;
}
