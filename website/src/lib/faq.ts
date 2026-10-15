export const faq: { q: string; a: string }[] = [
  {
    q: "Will it break my game?",
    a: "GenLauncher never edits your original files permanently. Before the game starts it writes down every change it is about to make (hiding or swapping files, linking the mod's files in), and when the game closes it undoes them. If the launcher or the PC dies mid-game, the next start finishes the cleanup first.",
  },
  {
    q: "Where do the mods come from?",
    a: "From the people who made them. GenLauncher hosts nothing and mirrors nothing: pressing Install on a mod opens its own page (ModDB, Nexus Mods, an author's site) in a built-in browser, you pick the version and click the site's own download button, and the launcher takes that file and installs it. You can also add a zip, 7z or rar you already have, or an unpacked folder.",
  },
  {
    q: "Do I need administrator rights?",
    a: "No. Mods are linked into the game folder with hard links on Windows, which need no special permission.",
  },
  {
    q: "Where are my mods stored?",
    a: "In a folder called .genlauncher inside your game folder, one folder per mod and version, with the page each version came from saved next to it. Removing a version deletes only that folder.",
  },
  {
    q: "I already use the old GenLauncher. Do I have to start over?",
    a: "No. On first start GenLauncher offers to move your existing mods into the new layout and to remove the old launcher's files. Nothing is deleted until the mods are safely moved.",
  },
  {
    q: "Can I have several versions of the same mod?",
    a: "Yes. Every version is kept separately and you pick which one to play from a list. Updating adds a new version; removing the old one is up to you.",
  },
  {
    q: "What does signing in with Google do?",
    a: "It is optional. It keeps copies of your launcher settings in a hidden folder of your own Google Drive that only GenLauncher can see, so you can bring your setup to a new PC. No mod files are ever uploaded. See the privacy policy for the details.",
  },
  {
    q: "Windows says \"Windows protected your PC\". Is it safe?",
    a: "That screen (Microsoft Defender SmartScreen) appears for any program that is not code-signed yet or that few people have downloaded. GenLauncher is open source, so you can read exactly what it does, and the installer on the releases page is built by GitHub from that source. Click More info, then Run anyway. Code signing is planned; once it is in place the warning fades as more people install it.",
  },
  {
    q: "Does it work on Linux?",
    a: "Not yet. Windows comes first. Running the game through Wine or Proton on Linux is planned.",
  },
  {
    q: "Is it free?",
    a: "Yes. GenLauncher is a free fan project and the source is on GitHub.",
  },
];
