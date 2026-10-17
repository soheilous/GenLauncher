// The words for the findings of the game health check (src-tauri/src/health.rs sends ids and values, never sentences).

import { N, t } from "$lib/i18n/index.svelte";

export type Level = "ok" | "info" | "warn" | "bad";

export interface Finding {
  id: string;
  level: Level;
  args: Record<string, string>;
  fix?: string | null;
}

interface Words { title: string; detail: string }

// The English text is the translation key (see src/lib/i18n). {values} come from the finding's args.
const WORDS: Record<string, Words> = {
  "all-good": { title: N("Everything looks fine"), detail: N("No problems were found with your game folder.") },
  "archives-missing": { title: N("Some game files are missing"), detail: N("The game needs {files} ({count} missing). In Steam, right-click the game, open Properties, Installed Files and choose Verify integrity of game files, or repair the install.") },
  "folder-read-only": { title: N("GenLauncher cannot write to the game folder"), detail: N("Move the game out of a protected folder such as Program Files, or give your user account write access to it. Mods cannot be installed or switched on until then.") },
  "dbghelp": { title: N("An old dbghelp.dll sits in the game folder"), detail: N("A copy of dbghelp.dll in the game folder is a known cause of crashes at start on Windows 10 and 11. Disabling it only renames the file, and you can undo that.") },
  "dbghelp-disabled": { title: N("dbghelp.dll is disabled"), detail: N("You disabled it earlier; the game uses the one that comes with Windows. You can put it back.") },
  "foreign-archives": { title: N("Extra .big files are in the game folder"), detail: N("{count} archives that are not part of the game ({files}) can turn textures pink or change how the game looks. Moving them aside keeps them safe, and you can move them back.") },
  "foreign-parked": { title: N("Some files were moved aside"), detail: N("GenLauncher moved some .big files out of your game folder. You can move them back.") },
  "session-unfinished": { title: N("A previous session was not closed properly"), detail: N("GenLauncher finishes restoring your game folder the next time it starts.") },
  "never-started": { title: N("The game has not been started yet"), detail: N("Start Generals or Zero Hour once without a mod so that it creates its settings, then check again. Some fixes and tools expect that.") },
  "registry-missing": { title: N("Windows does not know where the game is"), detail: N("The game's registry entry is missing. Many Steam installs lack it; some tools and mods need it. The Windows 10/11 fix in GenPatcher adds it.") },
  "registry-other-folder": { title: N("Windows points to another game folder"), detail: N("The registry says the game is at {path}, but you are using a different folder. Tools that read the registry look at the other one.") },
  "gentool": { title: N("GenTool is installed"), detail: N("GenTool (d3d8.dll) was found. It adds widescreen and fixes. Turn it off for LAN play if you see desyncs.") },
};

export const FIX_LABELS: Record<string, string> = {
  "disable-dbghelp": N("Disable it"),
  "restore-dbghelp": N("Restore it"),
  "park-foreign": N("Move them aside"),
  "restore-foreign": N("Move them back"),
};

export function describe(f: Finding): Words {
  const w = WORDS[f.id] ?? { title: f.id, detail: "" };
  return { title: t(w.title), detail: t(w.detail, f.args) };
}

/** Findings that deserve the user's attention (not just good to know). */
export const needsAttention = (list: Finding[]) => list.filter((f) => f.level === "warn" || f.level === "bad");
