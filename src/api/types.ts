// Mirrors the serde types in src-tauri/src (snake_case on the wire).

export type SourceType =
  | "official_publisher"
  | "developer_release"
  | "community_archive"
  | "community_reference"
  | "community_directory"
  | "community_discussion"
  | "user_added"
  | "local_file_unknown";

export type PackageType = "update" | "reinstall" | "special_variant" | "unknown";
export type HashAlgorithm = "md5" | "sha256";
export type MirrorMode = "direct_https" | "external_browser" | "user_supplied_local_file";
export type MirrorScope = "file" | "folder";
export type Availability = "available" | "unavailable" | "browser_only" | "unknown";
export type SourceState = "fresh" | "stale" | "failed" | "never_fetched";

export interface PublishedHash {
  algorithm: HashAlgorithm;
  value: string;
  source_id: string;
  derived_from: string | null;
  note: string | null;
}

export interface Mirror {
  host: string;
  url: string;
  mode: MirrorMode;
  scope: MirrorScope;
  availability: Availability;
  probed_at: string | null;
  availability_note: string | null;
}

export interface PackageRecord {
  package_id: string;
  source_id: string;
  firmware_version: string;
  long_version: string | null;
  build_date: string | null;
  source_section: string | null;
  raw_package_label: string;
  package_type: PackageType;
  variant_label: string | null;
  published_size_bytes: number | null;
  size_is_approximate: boolean;
  raw_size_text: string | null;
  hashes: PublishedHash[];
  mirrors: Mirror[];
  last_checked: string;
}

export interface HashClaim {
  algorithm: HashAlgorithm;
  value: string;
  source_ids: string[];
}

export interface CrossSourceMatch {
  algorithm: HashAlgorithm;
  value: string;
  source_a: string;
  source_b: string;
  independent: boolean;
}

export interface SizeClaim {
  bytes: number;
  approximate: boolean;
  source_id: string;
}

export interface Evidence {
  hash_claims: HashClaim[];
  sha256_conflict: boolean;
  md5_conflict: boolean;
  sizes: SizeClaim[];
  size_conflict: boolean;
  cross_source: CrossSourceMatch[];
  best_size_bytes: number | null;
  best_size_is_approximate: boolean;
  sony_availability: Availability | null;
}

export interface EntryRef {
  key: string;
  firmware_version: string;
  package_type: PackageType;
  variant_label: string | null;
  build_date: string | null;
}

export interface CatalogEntry {
  key: string;
  firmware_version: string;
  package_type: PackageType;
  variant_label: string | null;
  /** Sony build date ("2023_1101"); some versions shipped in more than one build. */
  build_date: string | null;
  records: PackageRecord[];
  evidence: Evidence;
  also_listed_as: EntryRef[];
}

export interface OfficialLatest {
  build_date: string;
  update_sha256: string;
  reinstall_sha256: string;
  update_url: string;
  reinstall_url: string;
  version: string | null;
  page_url: string;
}

export interface SourceBehind {
  source_id: string;
  newest_version: string;
  official_latest: string;
}

export interface SourceRecord {
  source_id: string;
  name: string;
  source_type: SourceType;
  source_page: string;
  maintainer: string | null;
  last_checked: string;
  adapter: string | null;
  derived_from: string[];
  notes: string;
}

export interface SourceStatus {
  source_id: string;
  state: SourceState;
  fetched_at: string | null;
  attempted_at: string | null;
  record_count: number;
  error: string | null;
  content_sha256: string | null;
  final_host: string | null;
  changed_since_last_fetch: boolean;
}

export interface CatalogView {
  catalog_version: string;
  app_version: string;
  sources: SourceRecord[];
  statuses: SourceStatus[];
  entries: CatalogEntry[];
  official_latest: OfficialLatest | null;
  sources_behind: SourceBehind[];
  saved_at: string | null;
  cache_note: string | null;
}

export interface LocalHashes {
  size_bytes: number;
  sha256: string;
  md5: string;
}

export type CheckOutcome = "match" | "mismatch";
export type EntryVerdict = "hash_matches" | "hash_mismatch" | "no_published_hash";

export interface HashCheck {
  algorithm: HashAlgorithm;
  expected: string;
  source_ids: string[];
  outcome: CheckOutcome;
}

export interface SizeCheck {
  expected_bytes: number;
  approximate: boolean;
  outcome: CheckOutcome;
}

export interface EntryCheck {
  key: string;
  firmware_version: string;
  package_type: PackageType;
  variant_label: string | null;
  hash_checks: HashCheck[];
  size_check: SizeCheck | null;
  verdict: EntryVerdict;
}

export interface VerifyReport {
  file_name: string;
  local: LocalHashes;
  exceeds_fat32_limit: boolean;
  matching_entries: EntryCheck[];
  selected: EntryCheck | null;
}

export type BusKind = "usb" | "other" | "unknown";

export interface VolumeInfo {
  id: string;
  mount_path: string;
  label: string;
  filesystem: string;
  serial: number;
  total_bytes: number;
  free_bytes: number;
  removable: boolean;
  bus: BusKind;
  is_system: boolean;
}

export interface VolumeView extends VolumeInfo {
  blockers: string[];
}

export interface Preflight {
  target_path: string;
  target_exists: boolean;
  target_size: number | null;
  other_pups: string[];
  bytes_needed: number;
  blockers: string[];
}

export interface PreflightView {
  preflight: Preflight;
  volume: VolumeInfo;
  source_name: string;
  source_size: number;
}

export interface DownloadView {
  path: string;
  check: EntryCheck;
}

export interface BuildView {
  target: string;
  replaced_existing: boolean;
  check: EntryCheck;
  volume_label: string;
  mount_path: string;
}

export type BuildPhase = "writing" | "checking_temp" | "checking_final";

export interface Leftover {
  file_name: string;
  kind: "temp" | "backup";
  size: number;
  final_exists: boolean;
}

export interface LeftoverGroup {
  volume_id: string;
  volume_label: string;
  mount_path: string;
  leftovers: Leftover[];
}

export type NotebookKind = "pc_folder" | "console_usb";

export interface SnapshotSummary {
  id: string;
  taken_at: string;
  label: string;
  note: string;
  kind: NotebookKind;
  root: string;
  file_count: number;
  dir_count: number;
  total_bytes: number;
  truncated: boolean;
  skipped: number;
}

export interface NotebookView {
  snapshots: SnapshotSummary[];
  note: string | null;
}

export interface NotebookEntry {
  path: string;
  dir: boolean;
  size: number;
  modified: number | null;
}

export interface NotebookChange {
  path: string;
  old_size: number;
  new_size: number;
  old_modified: number | null;
  new_modified: number | null;
}

export interface NotebookMove {
  from: string;
  to: string;
  size: number;
}

export interface NotebookDiff {
  added: NotebookEntry[];
  removed: NotebookEntry[];
  changed: NotebookChange[];
  moved: NotebookMove[];
  unchanged: number;
  added_total: number;
  removed_total: number;
  changed_total: number;
  moved_total: number;
  cut: boolean;
}
