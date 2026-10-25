// Community resource directory. Data only: nothing here is ever executed, and no mirror or
// download links belong in this file. Each note has a translation key res.<id>.note.

export type ResourceCategory =
  | "official"
  | "archive"
  | "technical_reference"
  | "developer_release"
  | "directory"
  | "guide"
  | "discussion";

/** How the page itself should be weighed: the maintainer's own words, a wiki, a write-up or a forum. */
export type Provenance = "first_hand" | "wiki" | "community_post" | "forum";

export interface Resource {
  id: string;
  name: string;
  maintainer: string;
  url: string;
  category: ResourceCategory;
  provenance: Provenance;
  /** Site blocks automated fetches; the app only links to it. */
  linkOnly: boolean;
  /** ISO date the link was last checked by a person. */
  lastChecked: string;
}

export const CATEGORIES: ResourceCategory[] = [
  "official",
  "archive",
  "technical_reference",
  "developer_release",
  "directory",
  "guide",
  "discussion",
];

export const REPORT_URL = "https://github.com/voidfnc/VoidRecoverPS5/issues/new";

export const RESOURCES: Resource[] = [
  {
    id: "sony-support",
    name: "PS5 system software update and reinstallation",
    maintainer: "Sony Interactive Entertainment",
    url: "https://www.playstation.com/en-us/support/hardware/ps5/system-software/",
    category: "official",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "sony-safe-mode",
    name: "Safe Mode on PS5",
    maintainer: "Sony Interactive Entertainment",
    url: "https://www.playstation.com/en-us/support/hardware/safe-mode-playstation/",
    category: "official",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "darthsternie",
    name: "PS5 Firmware Archive",
    maintainer: "Darthsternie",
    url: "https://darthsternie.net/ps5-firmwares/",
    category: "archive",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "mewx-list",
    name: "PS5 official firmware list",
    maintainer: "MewX",
    url: "https://github.com/MewX/ps5-official-firmware-list",
    category: "archive",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "psdevwiki",
    name: "PS5 Developer Wiki",
    maintainer: "PSDevWiki community",
    url: "https://www.psdevwiki.com/ps5/",
    category: "technical_reference",
    provenance: "wiki",
    linkOnly: true,
    lastChecked: "2026-10-25",
  },
  {
    id: "ps5dev",
    name: "PS5Dev on GitHub",
    maintainer: "PS5Dev organisation",
    url: "https://github.com/PS5Dev",
    category: "developer_release",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "y2jb",
    name: "Y2JB",
    maintainer: "Gezine",
    url: "https://github.com/Gezine/Y2JB",
    category: "developer_release",
    provenance: "first_hand",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "myfl",
    name: "PS5 Jailbreak & Homebrew Resources",
    maintainer: "myfl.uk",
    url: "https://myfl.uk/",
    category: "directory",
    provenance: "community_post",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "recovery-guide",
    name: "Unbricking your PS5 after a failed 13.60 jailbreak crash",
    maintainer: "Gaming Like A Boss",
    url: "https://gaminglikeaboss.com/guides/unbricking-your-ps5-how-to-recover-from-a-failed-13-60-jailbreak-crash/",
    category: "guide",
    provenance: "community_post",
    linkOnly: false,
    lastChecked: "2026-10-25",
  },
  {
    id: "reddit",
    name: "r/PS5_Jailbreak",
    maintainer: "Reddit community",
    url: "https://www.reddit.com/r/PS5_Jailbreak/",
    category: "discussion",
    provenance: "forum",
    linkOnly: true,
    lastChecked: "2026-10-25",
  },
];
