# Mod index

The list GenLauncher shows in **Discover** and under each mod's **Patches / Addons / Executables** tabs. One file per game:

| File | Game |
|---|---|
| [`zero-hour.json`](zero-hour.json) | Command & Conquer: Generals Zero Hour |
| [`generals.json`](generals.json) | Command & Conquer: Generals |

The launcher reads these files from this repository on GitHub (`main` branch), caches the last copy it got, and ships a
copy of them inside every release.

**Pointers only.** An entry says what a mod is called, which version is the latest, and which page it is published on. It
never contains a download link or a file. When someone presses *Install*, the launcher opens `page` in its built-in
browser and the person downloads from the publisher's own page. Please keep it that way: link to the page the author
publishes on (ModDB, Nexus Mods, their site), not to a mirror somebody else uploaded.

## Format

```jsonc
{
  "format": 1,              // bumped only for incompatible changes
  "game": "zh",             // "zh" or "generals"
  "updated": "2026-10-14",  // date of the last edit
  "mods": [
    {
      "name": "Rise of the Reds",             // required; also what the library matches installed folders against
      "version": "1.87 Public Build 2.0",     // latest known version, free text
      "page": "https://www.moddb.com/mods/rise-of-the-reds",  // where to get it
      "image": "https://i.imgur.com/....png", // cover
      "discord": "...", "news": "...", "support": "...", "network": "...", // optional links
      "colors": { "active": "#ff0000", "border": "#aa0000", "backgroundImage": "https://..." },
      "deprecated": false,
      "patches": [ { "name": "Hanpatch", "version": "32.2", "page": "...", "dependsOn": "Rise of the Reds" } ],
      "addons":  [ { "name": "...", "version": "...", "page": "...", "dependsOn": "<the patch or mod it needs>" } ]
    }
  ],
  "originalGame": { "patches": [], "addons": [] },   // for the stock game
  "executables": [
    {
      "name": "modded.exe", "version": "...", "page": "...",
      "for": "Contra",                                // omit for "any mod"
      "executableFileName": "modded.exe",
      "replacesGameExe": true, "replacesWorldBuilderExe": false
    }
  ]
}
```

Everything except `name` is optional, and unknown fields are ignored. Entries with the same name are the same item, so
keep names stable: renaming a mod makes the launcher treat it as a different one.

## Adding or fixing an entry

Edit the JSON, bump `updated`, and open a pull request. The tests check that both files parse and contain no download or
storage links.

## Origin

The first version of this index was converted from the mod list of GenLauncher 1.x by p0ls3r
([p0ls3r/GenLauncherModsData](https://github.com/p0ls3r/GenLauncherModsData)). Download links and storage details from
that list were left out.
