//! Everything the launcher stores in a game folder lives under one directory, so removing
//! the launcher means deleting `.genlauncher/` and nothing else.
//!
//! ```text
//! <game>/.genlauncher/
//!   mods/<Mod>/<Version>/...
//!   mods/<Mod>/patches|addons|exes/<Name>/<Version>/...
//!   state/    settings.json, launch-journal.json, catalog-<game>.json
//!   tmp/      download staging; a version only appears in mods/ once it is complete
//! ```

use std::path::{Path, PathBuf};

pub const ROOT: &str = ".genlauncher";
pub const MODS: &str = "mods";
pub const STATE: &str = "state";
pub const TMP: &str = "tmp";

pub const PATCHES: &str = "patches";
pub const ADDONS: &str = "addons";
pub const EXES: &str = "exes";

pub fn root(game_dir: &Path) -> PathBuf {
    game_dir.join(ROOT)
}
pub fn mods(game_dir: &Path) -> PathBuf {
    root(game_dir).join(MODS)
}
pub fn state(game_dir: &Path) -> PathBuf {
    root(game_dir).join(STATE)
}
pub fn tmp(game_dir: &Path) -> PathBuf {
    root(game_dir).join(TMP)
}

/// Names GenLauncher 1.x used; only [`crate::migrate`] touches them.
pub mod legacy {
    pub const MODS: &str = "GLM";
    pub const MODS_OLD: &str = "GenLauncherModifications";
    pub const DATA: &str = ".GenLauncherFolder";
    pub const PATCHES: &str = "Patches";
    pub const ADDONS: &str = "Addons";
    pub const EXES: &str = "Executables";
    /// Suffix of 1.x's temporary download folders.
    pub const TEMP_SUFFIX: &str = ".GLTC";
}
